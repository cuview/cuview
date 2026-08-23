#![allow(unused)]

use std::{
	borrow::{Borrow, Cow},
	collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher},
	convert::TryInto,
	f32::consts::TAU,
	ffi::OsStr,
	hash::{Hash, Hasher},
	io::{Read, Write},
	mem::size_of,
	path::{Component, Path, PathBuf},
	process::exit,
	time::Duration,
};

use anyhow::{Context, bail};
use blockstate::BlockStates;
use clap::Parser;
use cuview::{
	AResult,
	default,
	jarfs::JarFS,
	loader::{
		self,
		common::AnvilRegion,
		model::{Element, Face as JsonFace, JsonBlockState, JsonModel},
		*,
	},
	renderer::{
		Framebuffer,
		RenderContext,
		model::{Cube, Model, ModelCache, Texture, models_for_states},
		texture::{Cartographer, Image, TextureId},
	},
	types::{
		BlockPos,
		ChunkPos,
		IString,
		RegionPos,
		ResourceLocation,
		blockstate::{BlockState, BlockStateBuilder, BlockStateCache},
		resource_location::ResourceKind,
	},
	world::Palette,
};
use glam::{Mat4, UVec2, Vec2, Vec3, uvec2, vec2, vec3};
use loader::model::{BlockStateModel, MultipartCase, OneOrMany};
use model::MultipartWhen;
use wgpu::{
	Extent3d,
	util::{DeviceExt, DrawIndirectArgs},
};

#[derive(Parser, Debug)]
struct Args {
	#[arg(short, long)]
	blockstates: PathBuf,

	#[arg(short, long)]
	jars: Vec<PathBuf>,

	#[arg(long)]
	jarlist: Option<PathBuf>,

	#[arg(short, long)]
	world_root: PathBuf,

	#[arg(short, long)]
	target_chunk: ChunkPos,

	#[arg(short, long)]
	chunk_radius: usize,

	#[arg(long, default_value_t = UVec2Arg(uvec2(1920, 1080)))]
	image_size: UVec2Arg,

	#[arg(long, default_value_t = Vec3Arg(vec3(-5.0, 4.0, -5.0)))]
	camera_origin: Vec3Arg,

	#[arg(long, default_value_t = Vec2Arg(Vec2::splat(0.0)))]
	camera_angles: Vec2Arg,
}

macro_rules! replace {
	($_:tt $e:expr) => {
		$e
	};
}

macro_rules! count {
	($($xs:tt)*) => { 0usize $(+ replace!($xs 1usize))* };
}

macro_rules! VecArg {
	($name:ident $type:ty [ $($field:ident)+ ]) => {
		#[derive(Clone, Copy, Debug)]
		struct $name($type);

		impl std::ops::Deref for $name {
			type Target = $type;

			fn deref(&self) -> &Self::Target {
				&self.0
			}
		}

		impl std::fmt::Display for $name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				use std::fmt::Write;

				for (i, &v) in self.0.as_ref().into_iter().enumerate() {
					if i > 0 {
						f.write_char(',')?;
					}
					write!(f, "{v}")?;
				}
				Ok(())
			}
		}

		impl std::str::FromStr for $name {
			type Err = <<$type as std::ops::Index<usize>>::Output as std::str::FromStr>::Err;

			fn from_str(str: &str) -> Result<Self, Self::Err> {
				let mut res = <$type>::ZERO;
				let mut split = str.splitn(count!($($field)*), ",");
				$(res.$field = split.next().unwrap_or("").parse()?;)*
				Ok(Self(res))
			}
		}
	};
}

VecArg!(Vec2Arg Vec2 [x y]);
VecArg!(UVec2Arg UVec2 [x y]);
VecArg!(Vec3Arg Vec3 [x y z]);

enum CameraArgs {
	Perspective {},
}

fn main() -> AResult {
	let mut args = Args::parse();
	#[cfg(debug_assertions)]
	dbg!(&args);

	let world_root = args.world_root;
	if !world_root.is_dir() {
		let world_dir = world_root.display();
		bail!("{world_dir} is not a directory");
	}

	let version = identify_version(&world_root);
	if version.is_none() {
		bail!("Couldn't determine Minecraft version of the given world");
	}
	let version = version.unwrap();
	println!(
		"Minecraft version: {}.{}.{}",
		version.0, version.1, version.2
	);

	let blockstates = std::fs::read_to_string(args.blockstates).unwrap();
	let blockstates: blockstate::BlockStates = serde_json::from_str(&blockstates).unwrap();
	let blockstates = BlockStateCache::from_json(blockstates);

	if let Some(jarlist) = args.jarlist {
		let contents = std::fs::read_to_string(jarlist).unwrap();
		let paths = contents.lines().map(PathBuf::from);
		args.jars.extend(paths);
	}
	let fs = JarFS::new(args.jars).unwrap();

	let models = ModelCache::from_jsons(&fs);
	let statemap = models_for_states(&fs, &blockstates);

	let wrangler = WorldWrangler::new(world_root).unwrap();

	let dim = wrangler.probe_dimension("overworld".into()).unwrap();
	let dim = wrangler.load_dimension(dim);

	let target_chunk = args.target_chunk;

	pollster::block_on(async {
		let render = RenderContext::new().await?;

		let camera_buffer = {
			let camera_buffer = render.device.create_buffer(&wgpu::BufferDescriptor {
				label: None,
				size: size_of::<[f32; 32]>() as wgpu::BufferAddress,
				usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
				mapped_at_creation: false,
			});
			let projection = glam::camera::rh::proj::directx::perspective(
				110f32.to_radians(),
				args.image_size.x as f32 / args.image_size.y as f32,
				0.01,
				1000.0,
			);
			let rot = Mat4::from_rotation_y(args.camera_angles.y.to_radians()) *
				Mat4::from_rotation_x(args.camera_angles.x.to_radians());
			let forward = rot.transform_vector3(Vec3::Z);
			let camera = glam::camera::rh::view::look_at_mat4(
				*args.camera_origin,
				*args.camera_origin + forward,
				Vec3::Y,
			);

			render
				.queue
				.write_buffer(&camera_buffer, 0, bytemuck::cast_slice(projection.as_ref()));
			render.queue.write_buffer(
				&camera_buffer,
				size_of::<[f32; 16]>() as wgpu::BufferAddress,
				bytemuck::cast_slice(camera.as_ref()),
			);

			camera_buffer
		};

		let framebuffer = Framebuffer::new(&render, *args.image_size);

		let (cartographer, block_texture_layers) =
			Cartographer::load(&fs, &models, &render.device).unwrap();
		let block_texture_size = wgpu::Extent3d {
			width: block_texture_layers[0].size.x,
			height: block_texture_layers[0].size.y,
			depth_or_array_layers: block_texture_layers.len() as u32,
		};
		let block_texture = render.device.create_texture(&wgpu::TextureDescriptor {
			label: None,
			size: block_texture_size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});
		let block_texture_view = block_texture.create_view(&wgpu::TextureViewDescriptor {
			dimension: Some(wgpu::TextureViewDimension::D2Array),
			..Default::default()
		});
		for (i, layer) in block_texture_layers.iter().enumerate() {
			let mut dest = block_texture.as_image_copy();
			dest.origin = wgpu::Origin3d {
				x: 0,
				y: 0,
				z: i as u32,
			};
			render.queue.write_texture(
				dest,
				bytemuck::cast_slice(&layer.pixels),
				wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(layer.size.x * size_of::<u32>() as u32),
					rows_per_image: None,
				},
				wgpu::Extent3d {
					depth_or_array_layers: 1,
					..block_texture_size
				},
			);
		}
		let block_texture_sampler = render.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		let atlas_diameters = render
			.device
			.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: None,
				usage: wgpu::BufferUsages::STORAGE,
				contents: bytemuck::cast_slice(cartographer.element_diameters()),
			});

		let geometry = models.geometry_buffer(&cartographer);
		let block_models_buffer =
			render
				.device
				.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: None,
					usage: wgpu::BufferUsages::VERTEX,
					contents: bytemuck::cast_slice(&geometry.vertices),
				});

		// assuming worst case every block in section is composed of 10 submodels
		const SUBMODELS_PER_BLOCK: usize = 10;
		const SUBMODELS_PER_SECTION: usize =
			ChunkPos::DIAMETER_BLOCKS.pow(3) as usize * SUBMODELS_PER_BLOCK;
		let indirect_buffers: Vec<_> = ChunkPos::SECTIONS
			.map(|_| {
				render.device.create_buffer(&wgpu::BufferDescriptor {
					label: None,
					size: (SUBMODELS_PER_SECTION * size_of::<wgpu::util::DrawIndirectArgs>())
						as wgpu::BufferAddress,
					usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
					mapped_at_creation: false,
				})
			})
			.collect();

		let shader = render
			.device
			.create_shader_module(wgpu::ShaderModuleDescriptor {
				label: None,
				source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/main.wgsl"))),
			});
		let bind_group_layout =
			render
				.device
				.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					label: None,
					entries: &[
						wgpu::BindGroupLayoutEntry {
							binding: 0,
							visibility: wgpu::ShaderStages::VERTEX,
							ty: wgpu::BindingType::Buffer {
								ty: wgpu::BufferBindingType::Uniform,
								has_dynamic_offset: false,
								min_binding_size: wgpu::BufferSize::new(
									size_of::<[f32; 32]>() as wgpu::BufferAddress
								),
							},
							count: None,
						},
						wgpu::BindGroupLayoutEntry {
							binding: 1,
							visibility: wgpu::ShaderStages::VERTEX,
							ty: wgpu::BindingType::Buffer {
								ty: wgpu::BufferBindingType::Storage { read_only: true },
								has_dynamic_offset: false,
								min_binding_size: None,
							},
							count: None,
						},
						wgpu::BindGroupLayoutEntry {
							binding: 2,
							visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Texture {
								sample_type: wgpu::TextureSampleType::Float { filterable: true },
								view_dimension: wgpu::TextureViewDimension::D2Array,
								multisampled: false,
							},
							count: None,
						},
						wgpu::BindGroupLayoutEntry {
							binding: 3,
							visibility: wgpu::ShaderStages::FRAGMENT,
							ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
							count: None,
						},
					],
				});
		let bind_group = render.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: camera_buffer.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: atlas_diameters.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&block_texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&block_texture_sampler),
				},
			],
		});
		let pipeline_layout =
			render
				.device
				.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
					label: None,
					bind_group_layouts: &[Some(&bind_group_layout)],
					immediate_size: 12,
				});
		let pipeline = render
			.device
			.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
				label: None,
				layout: Some(&pipeline_layout),
				vertex: wgpu::VertexState {
					module: &shader,
					entry_point: Some("vsMain"),
					compilation_options: default(),
					buffers: &[Some(wgpu::VertexBufferLayout {
						array_stride: size_of::<[f32; 6]>() as wgpu::BufferAddress,
						step_mode: wgpu::VertexStepMode::Vertex,
						attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32],
					})],
				},
				fragment: Some(wgpu::FragmentState {
					module: &shader,
					entry_point: Some("fsMain"),
					compilation_options: default(),
					targets: &[Some(wgpu::ColorTargetState {
						format: framebuffer.format(),
						blend: Some(wgpu::BlendState {
							color: wgpu::BlendComponent {
								src_factor: wgpu::BlendFactor::SrcAlpha,
								dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
								operation: wgpu::BlendOperation::Add,
							},
							alpha: wgpu::BlendComponent {
								src_factor: wgpu::BlendFactor::One,
								dst_factor: wgpu::BlendFactor::One,
								operation: wgpu::BlendOperation::Max,
							},
						}),
						write_mask: wgpu::ColorWrites::ALL,
					})],
				}),
				primitive: wgpu::PrimitiveState {
					cull_mode: None, // Some(wgpu::Face::Back),
					..wgpu::PrimitiveState::default()
				},
				depth_stencil: Some(wgpu::DepthStencilState {
					format: wgpu::TextureFormat::Depth24Plus,
					depth_write_enabled: Some(true),
					depth_compare: Some(wgpu::CompareFunction::Less),
					stencil: wgpu::StencilState::default(),
					bias: wgpu::DepthBiasState::default(),
				}),
				multisample: wgpu::MultisampleState {
					count: 4,
					..Default::default()
				},
				multiview_mask: None,
				cache: None,
			});

		framebuffer.clear(&render, wgpu::Color {
			r: 1.0,
			g: 0.5,
			b: 0.0,
			a: 1.0,
		})?;

		{
			let mut indirect_draws = vec![];
			let chunk_radius = args.chunk_radius as i32;
			let chunk_positions = (target_chunk.z - chunk_radius ..= target_chunk.z + chunk_radius)
				.flat_map(|z| {
					(target_chunk.x - chunk_radius ..= target_chunk.x + chunk_radius)
						.map(move |x| ChunkPos::new(x, z))
				});
			for chunk_pos in chunk_positions {
				dbg!(chunk_pos);
				let region = if let Some(region) = dim.borrow().get_region(chunk_pos.into()) {
					region
				} else {
					wrangler.load_region(&dim, chunk_pos.into())
				};
				let chunk = wrangler.load_chunk(&region, chunk_pos);
				let chunk = chunk.borrow();
				for section_y in chunk.sections() {
					indirect_draws.clear();
					let section = chunk.get_section(section_y).unwrap();
					let section = section.borrow();
					for block_pos in chunk_pos.blocks_in_section(section_y) {
						let state = section.get_block(block_pos);
						let modelsets = statemap.get(&state).unwrap();
						for set in modelsets {
							// FIXME: weighting
							let model = &set[blockpos_rng(block_pos).rem_euclid(set.len())];
							let model_id = model.model;
							if let Some((base_vertex, num_verts)) =
								geometry.model_info.get(&model_id).copied()
							{
								let block_rel = block_pos.chunk_relative();
								let block_index = block_rel.y * ChunkPos::DIAMETER_BLOCKS.pow(2) +
									block_rel.z * ChunkPos::DIAMETER_BLOCKS +
									block_rel.x;

								// pack rotations into the unused upper 20 bits of instance id
								let rot = vec2(
									model.x_rotation.unwrap_or(0.0).to_radians(),
									model.y_rotation.unwrap_or(0.0).to_radians(),
								);
								let rot_turns =
									Vec2::from((rot / TAU).as_ref().map(|v| v.rem_euclid(1.0)));
								let rot_discrete = (rot_turns * 1024.0).as_uvec2();
								let rot_packed =
									(rot_discrete.y & 1023) << 10 | rot_discrete.x & 1023;

								let instance = rot_packed << 12 | block_index as u32;
								indirect_draws.extend(
									DrawIndirectArgs {
										first_vertex: base_vertex as u32,
										vertex_count: num_verts as u32,
										first_instance: instance,
										instance_count: 1,
									}
									.as_bytes(),
								);
							}
						}
					}

					let indirect_buffer =
						&indirect_buffers[(section_y - ChunkPos::SECTIONS.start()) as usize];
					render
						.queue
						.write_buffer(indirect_buffer, 0, &indirect_draws);

					let mut encoder = render.device.create_command_encoder(&Default::default());
					let views = framebuffer.views();
					let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
						label: None,
						color_attachments: &[Some(wgpu::RenderPassColorAttachment {
							view: &views.multisample,
							depth_slice: None,
							resolve_target: Some(&views.color),
							ops: wgpu::Operations {
								load: wgpu::LoadOp::Load,
								store: wgpu::StoreOp::Store,
							},
						})],
						depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
							view: &views.depth,
							depth_ops: Some(wgpu::Operations {
								load: wgpu::LoadOp::Load,
								store: wgpu::StoreOp::Store,
							}),
							stencil_ops: None,
						}),
						timestamp_writes: None,
						occlusion_query_set: None,
						multiview_mask: None,
					});
					pass.set_pipeline(&pipeline);
					pass.set_bind_group(0, &bind_group, &[]);
					pass.set_vertex_buffer(0, block_models_buffer.slice(..));
					pass.set_immediates(8, bytemuck::bytes_of(&(section_y as i32)));
					pass.set_immediates(0, bytemuck::bytes_of(&chunk_pos.x));
					pass.set_immediates(4, bytemuck::bytes_of(&chunk_pos.z));
					pass.multi_draw_indirect(
						indirect_buffer,
						0,
						(indirect_draws.len() / size_of::<DrawIndirectArgs>()) as u32,
					);
					drop(pass);
					render.submit_and_poll(encoder);
				}
			}
		}

		let pixels = framebuffer.read(&render)?;
		let file = std::fs::OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(true)
			.open("out.png")
			.unwrap();
		let framebuffer_size = framebuffer.size();
		let mut encoder = png::Encoder::new(file, framebuffer_size.x, framebuffer_size.y);
		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);
		let mut writer = encoder.write_header().unwrap();
		writer.write_image_data(&pixels).unwrap();

		Ok(())
	})
}

fn blockpos_rng(pos: BlockPos) -> usize {
	let mut hasher = DefaultHasher::new();
	pos.hash(&mut hasher);
	hasher.finish() as usize
}
