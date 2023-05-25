#![allow(non_snake_case, non_upper_case_globals, unused)]

use std::borrow::{Borrow, Cow};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::TryInto;
use std::f32::consts::TAU;
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::mem::size_of;
use std::path::{Component, Path, PathBuf};
use std::process::exit;

use anyhow::Context;
use blockstate::BlockStates;
use clap::Parser;
use cuview::jarfs::JarFS;
use cuview::loader::common::AnvilRegion;
use cuview::loader::model::{Element, Face as JsonFace, JsonBlockState, JsonModel};
use cuview::loader::{self, *};
use cuview::renderer::model::{models_for_states, Cube, Model, ModelCache, Texture};
use cuview::renderer::texture::{Cartographer, Image, TextureId};
use cuview::types::blockstate::{BlockState, BlockStateBuilder, BlockStateCache};
use cuview::types::resource_location::ResourceKind;
use cuview::types::{BlockPos, ChunkPos, IString, RegionPos, ResourceLocation};
use cuview::world::Palette;
use glam::{uvec2, vec2, vec3, Mat4, UVec2, Vec2, Vec3};
use loader::model::{BlockStateModel, MultipartCase, OneOrMany};
use model::MultipartWhen;
use wgpu::util::{DeviceExt, DrawIndirect};
use wgpu::Extent3d;

#[cfg(none)]
fn main() {
	let fs = cuview::jarfs::JarFS::new(vec![
		Path::new("client-1.18.2.jar"),
		// Path::new("snad.jar"),
	])
	.unwrap();

	let mut blockstates: blockstate::BlockStates =
		serde_json::from_str(&std::fs::read_to_string("blockstates.json").unwrap()).unwrap();
	/* dbg!(
		&blockstates
			.0
			.get(&"redstone_wire".into())
			.unwrap()
			.properties
	); */
	// blockstates.0.retain(|&k, _| k.name.as_str() == "sandstone_wall");
	/* let k = blockstates.0.keys().copied().next().unwrap();
	blockstates.0.get_mut(&k).unwrap().states.truncate(1); */
	#[cfg(none)]
	blockstates.0.insert(
		"cuview:test".into(),
		blockstate::BlockDefinition {
			properties: None,
			states: vec![blockstate::State {
				properties: None,
				id: u32::MAX,
				default: true,
			}],
		},
	);
	let blockstates = BlockStateCache::from_json(blockstates);

	let modelsForState = models_for_states(&fs, &blockstates);
	let test1 = BlockState::stateless("stone".into());
	let test2 = BlockStateBuilder::from_variants_model("grass_block".into(), "snowy=false").build();
	let test3 = BlockStateBuilder::from_variants_model(
		"cobblestone_wall".into(),
		"north=low,east=none,south=none,west=none,up=true,waterlogged=false",
	)
	.build();
	dbg!(test1, modelsForState.get(&test1));
	dbg!(test2, modelsForState.get(&test2));
	dbg!(test3, modelsForState.get(&test3));
}

#[cfg(none)]
fn main() {
	let fs = cuview::jarfs::JarFS::new(vec![
		Path::new("client-1.18.2.jar"),
		// Path::new("snad.jar"),
	])
	.unwrap();
	let mut modelCache = ModelCache::from_jsons(&fs);

	let interestingModels = [
		"block/cactus",
		"block/fence_post",
		"block/fence_side",
		"block/template_fence_gate_wall",
		"block/template_fence_gate_open",
		"block/cross",
		"block/slab_top",
		"block/slab",
		"block/stairs",
		"block/stonecutter",
	];
	let mut xformed = Vec::with_capacity(interestingModels.len());
	for (modelIndex, modelPath) in interestingModels.iter().cloned().enumerate() {
		let loc = ResourceLocation::from(modelPath);
		let mat = Mat4::from_translation(vec3(modelIndex as f32, 0.0, 0.0));
		let model = modelCache
			.get(&loc)
			.expect(&format!("{modelPath}"))
			.transformed(&modelCache, mat);
		xformed.push((modelPath, model));
	}

	let (obj, mtl) = Model::into_wavefront(&modelCache, xformed.as_slice(), "interesting.mtl");
	std::fs::write("out/interesting.obj", obj).unwrap();
	std::fs::write("out/interesting.mtl", mtl).unwrap();
}

#[derive(Parser, Debug)]
struct Args {
	#[arg(short, long)]
	blockstates: PathBuf,

	#[arg(short, long)]
	jars: Vec<PathBuf>,

	#[arg(long)]
	jarlist: Option<PathBuf>,

	#[arg(short, long)]
	worldRoot: PathBuf,

	#[arg(short, long)]
	targetChunk: ChunkPos,

	#[arg(long, default_value_t = Vec3Arg(vec3(-5.0, 4.0, -5.0)))]
	cameraOrigin: Vec3Arg,

	#[arg(long, default_value_t = Vec2Arg(Vec2::splat(0.0)))]
	cameraAngles: Vec2Arg,
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
			type Err = std::num::ParseFloatError;

			fn from_str(str: &str) -> Result<Self, Self::Err> {
				let mut res = <$type>::splat(0.0);
				let mut split = str.splitn(count!($($field)*), ",");
				$(res.$field = split.next().unwrap_or("").parse()?;)*
				Ok(Self(res))
			}
		}
	};
}

VecArg!(Vec2Arg Vec2 [x y]);
VecArg!(Vec3Arg Vec3 [x y z]);

enum CameraArgs {
	Perspective {},
}

// #[cfg(none)]
fn main() {
	let mut args = Args::parse();

	dbg!(&args);

	let worldRoot = args.worldRoot;
	if !worldRoot.is_dir() {
		let worldDir = worldRoot.display();
		eprintln!("{worldDir} is not a directory");
		exit(1);
	}

	let version = identify_version(&worldRoot);
	if version == None {
		eprintln!("Couldn't determine Minecraft version of the given world");
		exit(1);
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

	let wrangler = WorldWrangler::new(worldRoot).unwrap();

	let dim = wrangler.probe_dimension("overworld".into()).unwrap();
	let dim = wrangler.load_dimension(dim);

	let targetChunk = args.targetChunk;
	let region = wrangler.load_region(&dim, targetChunk.into());
	let chunk = wrangler.load_chunk(&region, targetChunk);
	let chunk = chunk.borrow();
	/*let world = cuview::world::World::new(&worldRoot);
	let dim = world.borrow_mut().new_dimension("overworld".into(), &worldRoot);
	let region = dim.borrow_mut().new_region(RegionPos::new(0, 0));
	let chunk = region.borrow_mut().new_chunk(ChunkPos::new(0, 0));

	// let blocks: [ResourceLocation; 16] = [
	// 	"white_wool".into(),
	// 	"orange_wool".into(),
	// 	"magenta_wool".into(),
	// 	"light_blue_wool".into(),
	// 	"yellow_wool".into(),
	// 	"lime_wool".into(),
	// 	"pink_wool".into(),
	// 	"gray_wool".into(),
	// 	"light_gray_wool".into(),
	// 	"cyan_wool".into(),
	// 	"purple_wool".into(),
	// 	"blue_wool".into(),
	// 	"brown_wool".into(),
	// 	"green_wool".into(),
	// 	"red_wool".into(),
	// 	"black_wool".into(),
	// ];
	// let mut blocks = blocks.into_iter().cycle();
	let mut states = blockstates.blocks().map(|v| blockstates.default_state_of(v).unwrap());
	for y in ChunkPos::sections {
		// let state = BlockState::stateless(blocks.next().unwrap());
		let state = states.next().unwrap();
		let palette: Palette = [state].into_iter().collect();
		let section = chunk.borrow_mut().new_section(y, palette);
		section.borrow_mut().fill_with_block(state);
	}

	let targetChunk = ChunkPos::new(0, 0);
	let chunk = chunk.borrow();*/

	/* let section = chunk.get_section(-4).unwrap();
	let section = section.borrow();
	let mut blocks = BTreeSet::new();
	for pos in section.pos().0.blocks_in_section(-4) {
		blocks.insert(section.get_block(pos).block_name());
	}
	dbg!(blocks); */

	#[cfg(none)]
	pollster::block_on(async {
		let instance = wgpu::Instance::new(wgpu::Backends::all());
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				force_fallback_adapter: false,
				compatible_surface: None,
			})
			.await
			.unwrap();
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					features: wgpu::Features::PUSH_CONSTANTS |
						wgpu::Features::MULTI_DRAW_INDIRECT |
						wgpu::Features::INDIRECT_FIRST_INSTANCE,
					limits: wgpu::Limits {
						max_push_constant_size: 128,
						// max_texture_dimension_3d: 1024,
						..wgpu::Limits::default()
					},
				},
				None,
			)
			.await
			.unwrap();
		let (cartographer, texLayers) = Cartographer::load(&fs, &models, &device).unwrap();
		dbg!(cartographer.texture_for_id(TextureId {
			atlas: 1,
			texture: 128 * 71 + 110
		}));
		eprintln!(
			"{} layers, max diameter {}",
			texLayers.len(),
			device.limits().max_texture_dimension_3d
		);
		/* let base = PathBuf::from("./aout/");
		std::fs::remove_dir_all(&base).unwrap_or_default();
		std::fs::create_dir(&base).unwrap();
		let led = cartographer.element_diameters();
		for (id, img) in texLayers.iter().enumerate() {
			let diameter = led[id];
			let UVec2 { x: width, y: height } = img.size;
			let path = base.join(format!("layer{id}_{width}x{height}_{diameter}x.png"));
			img.save_to_file(&path).unwrap();
			eprintln!("ok wrote {path:?}");
		} */
	});

	// #[cfg(none)]
	pollster::block_on(async {
		let instance = wgpu::Instance::new(wgpu::Backends::all());
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				force_fallback_adapter: false,
				compatible_surface: None,
			})
			.await
			.unwrap();
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					features: wgpu::Features::PUSH_CONSTANTS |
						wgpu::Features::MULTI_DRAW_INDIRECT |
						wgpu::Features::INDIRECT_FIRST_INSTANCE,
					limits: wgpu::Limits {
						max_push_constant_size: 128,
						max_texture_dimension_2d: 32768,
						..wgpu::Limits::default()
					},
				},
				None,
			)
			.await
			.unwrap();

		let (cameraBuffer, imgWidth, imgHeight) = {
			let (imgWidth, imgHeight) = (1280, 720);
			let cameraBuffer = device.create_buffer(&wgpu::BufferDescriptor {
				label: None,
				size: size_of::<[f32; 32]>() as wgpu::BufferAddress,
				usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
				mapped_at_creation: false,
			});
			// #[cfg(none)]
			let projection = Mat4::perspective_rh(
				110f32.to_radians(),
				imgWidth as f32 / imgHeight as f32,
				0.01,
				1000.0,
			);
			let rot = Mat4::from_rotation_y(args.cameraAngles.0.y.to_radians()) *
				Mat4::from_rotation_x(args.cameraAngles.0.x.to_radians());
			let forward = rot.transform_vector3(Vec3::Z);
			dbg!(forward);
			let camera =
				Mat4::look_at_rh(args.cameraOrigin.0, args.cameraOrigin.0 + forward, Vec3::Y);

			/* let rot = Mat4::from_rotation_y(args.cameraAngles.0.y.to_radians()) *
				Mat4::from_rotation_x(args.cameraAngles.0.x.to_radians());
			let forward = rot.transform_vector3(Vec3::Z);
			let pos = vec3(0.0, 321.0, 0.0);
			let camera = Mat4::look_at_rh(
				/* args.cameraOrigin.0 */ pos,
				/* args.cameraOrigin.0 */ pos + forward,
				Vec3::Y,
			);
			let cube = Cube::new(vec3(0.0, -64.0, 0.0), vec3(16.0, 320.0, 16.0)).transform(camera);
			let projection = Mat4::orthographic_rh(
				cube.mins.x,
				cube.maxs.x,
				cube.mins.y,
				cube.maxs.y,
				0.0,
				1000.0,
			); */

			queue.write_buffer(&cameraBuffer, 0, bytemuck::cast_slice(projection.as_ref()));
			queue.write_buffer(
				&cameraBuffer,
				size_of::<[f32; 16]>() as wgpu::BufferAddress,
				bytemuck::cast_slice(camera.as_ref()),
			);

			// let cubeSize = cube.size();
			// let scale = 32.0;
			// (
			// 	cameraBuffer,
			// 	(cubeSize.x * scale) as u32,
			// 	(cubeSize.y * scale) as u32,
			// )
			(cameraBuffer, imgWidth, imgHeight)
		};

		let frameSize = wgpu::Extent3d {
			width: imgWidth,
			height: imgHeight,
			depth_or_array_layers: 1,
		};
		let frameFormat = wgpu::TextureFormat::Rgba8Unorm;
		let frameTexture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("frameTexture"),
			size: frameSize,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: frameFormat,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
		});
		let frameTextureMultisample = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("frameTextureMultisample"),
			size: frameSize,
			mip_level_count: 1,
			sample_count: 4,
			dimension: wgpu::TextureDimension::D2,
			format: frameFormat,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		});
		let frameDepthFormat = wgpu::TextureFormat::Depth24Plus;
		let frameDepthTexture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("frameDepthTexture"),
			size: frameSize,
			mip_level_count: 1,
			sample_count: 4,
			dimension: wgpu::TextureDimension::D2,
			format: frameDepthFormat,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		});
		let frameCopyBufferSize = ImgBufferSize::new(frameSize);
		let frameCopyBuffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: None,
			mapped_at_creation: false,
			size: (frameCopyBufferSize.bplPadded * frameCopyBufferSize.height)
				as wgpu::BufferAddress,
			usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
		});
		let surfaceConfig = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: frameFormat,
			width: frameSize.width,
			height: frameSize.height,
			present_mode: wgpu::PresentMode::Immediate,
		};

		let (cartographer, blockTextureLayers) = Cartographer::load(&fs, &models, &device).unwrap();
		#[cfg(none)]
		{
			let base = PathBuf::from("./aout/");
			std::fs::remove_dir_all(&base).unwrap_or_default();
			std::fs::create_dir(&base).unwrap();
			let diams = cartographer.element_diameters();
			for (id, img) in blockTextureLayers.iter().enumerate() {
				let diameter = diams[id];
				let UVec2 {
					x: width,
					y: height,
				} = img.size;
				let path = base.join(format!("layer{id}_{width}x{height}_{diameter}x.png"));
				img.save_to_file(&path).unwrap();
				eprintln!("ok wrote {path:?}");
			}
		}
		let blockTextureSize = wgpu::Extent3d {
			width: blockTextureLayers[0].size.x,
			height: blockTextureLayers[0].size.y,
			depth_or_array_layers: blockTextureLayers.len() as u32,
		};
		let blockTexture = device.create_texture(&wgpu::TextureDescriptor {
			label: None,
			size: blockTextureSize,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		});
		let blockTextureView = blockTexture.create_view(&wgpu::TextureViewDescriptor {
			dimension: Some(wgpu::TextureViewDimension::D2Array),
			..Default::default()
		});
		for (i, layer) in blockTextureLayers.iter().enumerate() {
			let mut dest = blockTexture.as_image_copy();
			dest.origin = wgpu::Origin3d {
				x: 0,
				y: 0,
				z: i as u32,
			};
			queue.write_texture(
				dest,
				bytemuck::cast_slice(&layer.pixels),
				wgpu::ImageDataLayout {
					offset: 0,
					bytes_per_row: Some(
						(layer.size.x * size_of::<u32>() as u32).try_into().unwrap(),
					),
					rows_per_image: None,
				},
				wgpu::Extent3d {
					depth_or_array_layers: 1,
					..blockTextureSize
				},
			);
		}
		let blockTextureSampler = device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Linear,
			..Default::default()
		});
		let atlasDiameters = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			usage: wgpu::BufferUsages::STORAGE,
			contents: bytemuck::cast_slice(cartographer.element_diameters()),
		});

		let geometry = models.geometry_buffer(&cartographer);
		let blockModelsBuffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			usage: wgpu::BufferUsages::VERTEX,
			contents: bytemuck::cast_slice(&geometry.vertices),
		});

		// assuming worst case every block in section is composed of 10 submodels
		const submodelsPerBlock: usize = 10;
		const submodelsPerSection: usize =
			ChunkPos::diameterBlocks.pow(3) as usize * submodelsPerBlock;
		let indirectBuffers: Vec<_> = ChunkPos::sections
			.map(|_| {
				device.create_buffer(&wgpu::BufferDescriptor {
					label: None,
					size: (submodelsPerSection * size_of::<wgpu::util::DrawIndirect>())
						as wgpu::BufferAddress,
					usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
					mapped_at_creation: false,
				})
			})
			.collect();

		/* let debugTris: &[f32] = &[
			0.0, 1.0, -1.0,   0.5, 1.0,
			-1.0, 0.0, 1.0,   0.0, 0.0,
			1.0, 0.0, 1.0,   1.0, 0.0,

			0.0, 0.0, 0.0,   1.0, 0.0,
			1.0, 0.0, 0.0,   1.0, 0.0,
			0.0, 0.0, 1.0,   1.0, 0.0,

			0.0, 0.0, 0.0,   0.0, 1.0,
			1.0, 0.0, 0.0,   0.0, 1.0,
			0.0, 1.0, 0.0,   0.0, 1.0,

			0.0, 0.0, 0.0,   1.0, 1.0,
			0.0, 1.0, 0.0,   1.0, 1.0,
			0.0, 0.0, 1.0,   1.0, 1.0,
		];
		let debugTris = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			usage: wgpu::BufferUsages::VERTEX,
			contents: bytemuck::cast_slice(debugTris),
		}); */

		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: None,
			source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/main.wgsl"))),
		});
		let bindGroupLayout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
		let bindGroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &bindGroupLayout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: cameraBuffer.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: atlasDiameters.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&blockTextureView),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&blockTextureSampler),
				},
			],
		});
		let pipelineLayout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &[&bindGroupLayout],
			push_constant_ranges: &[
				wgpu::PushConstantRange {
					range: 0 .. 4,
					stages: wgpu::ShaderStages::VERTEX,
				},
			],
		});
		let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: None,
			layout: Some(&pipelineLayout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vsMain",
				buffers: &[
					wgpu::VertexBufferLayout {
						array_stride: size_of::<[f32; 6]>() as wgpu::BufferAddress,
						step_mode: wgpu::VertexStepMode::Vertex,
						attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32],
					},
				],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: "fsMain",
				targets: &[Some(
					wgpu::ColorTargetState {
						format: frameFormat,
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
					},
				)],
			}),
			primitive: wgpu::PrimitiveState {
				cull_mode: None, // Some(wgpu::Face::Back),
				..wgpu::PrimitiveState::default()
			},
			depth_stencil: Some(wgpu::DepthStencilState {
				format: wgpu::TextureFormat::Depth24Plus,
				depth_write_enabled: true,
				depth_compare: wgpu::CompareFunction::Less,
				stencil: wgpu::StencilState::default(),
				bias: wgpu::DepthBiasState::default(),
			}),
			multisample: wgpu::MultisampleState {
				count: 4,
				..Default::default()
			},
			multiview: None,
		});

		let mut encoder = device.create_command_encoder(&Default::default());
		{
			let colorView = frameTexture.create_view(&Default::default());
			let multisampleView = frameTextureMultisample.create_view(&Default::default());
			let depthView = frameDepthTexture.create_view(&wgpu::TextureViewDescriptor {
				aspect: wgpu::TextureAspect::DepthOnly,
				..Default::default()
			});

			let mut clearPass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[Some(
					wgpu::RenderPassColorAttachment {
						view: &multisampleView,
						resolve_target: Some(&colorView),
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color {
								r: 1.0,
								g: 0.5,
								b: 0.0,
								a: 1.0,
							}),
							store: true,
						},
					},
				)],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &depthView,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: true,
					}),
					stencil_ops: None,
				}),
			});
			drop(clearPass);

			let mut indirectDraws = vec![];
			for sectionY in chunk.sections() {
				indirectDraws.clear();
				let section = chunk.get_section(sectionY).unwrap();
				let section = section.borrow();
				for blockPos in targetChunk.blocks_in_section(sectionY) {
					let state = section.get_block(blockPos);
					let modelsets = statemap.get(&state).unwrap();
					for set in modelsets {
						// FIXME: weighting
						let model = &set[blockpos_rng(blockPos).rem_euclid(set.len())];
						let modelId = model.model;
						if let Some((baseVertex, numVerts)) =
							geometry.modelInfo.get(&modelId).copied()
						{
							let blockRel = blockPos.chunk_relative();
							let blockIndex = blockRel.y * ChunkPos::diameterBlocks.pow(2) +
								blockRel.z * ChunkPos::diameterBlocks +
								blockRel.x;

							// pack rotations into the unused upper 20 bits of instance id
							// let rot = vec2(45f32.to_radians(), 0.0/* (14.5 * blockIndex as
							// f32).to_radians() */);
							let rot = vec2(
								model.xRotation.unwrap_or(0.0).to_radians(),
								model.yRotation.unwrap_or(0.0).to_radians(),
							);
							let rotTurns =
								Vec2::from((rot / TAU).as_ref().map(|v| v.rem_euclid(1.0)));
							let rotDiscrete = (rotTurns * 1024.0).as_uvec2();
							let rotPacked = (rotDiscrete.y & 1023) << 10 | rotDiscrete.x & 1023;

							let instance = rotPacked << 12 | blockIndex as u32;
							indirectDraws.extend(
								DrawIndirect {
									base_vertex: baseVertex as u32,
									vertex_count: numVerts as u32,
									base_instance: instance,
									instance_count: 1,
								}
								.as_bytes(),
							);
						}
					}
				}

				let indirectBuffer =
					&indirectBuffers[(sectionY - ChunkPos::sections.start()) as usize];
				queue.write_buffer(indirectBuffer, 0, &indirectDraws);
				// queue.submit(None);
				let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: None,
					color_attachments: &[Some(
						wgpu::RenderPassColorAttachment {
							view: &multisampleView,
							resolve_target: Some(&colorView),
							ops: wgpu::Operations {
								load: wgpu::LoadOp::Load,
								store: true,
							},
						},
					)],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &depthView,
						depth_ops: Some(wgpu::Operations {
							load: wgpu::LoadOp::Load,
							store: true,
						}),
						stencil_ops: None,
					}),
				});
				pass.set_pipeline(&pipeline);
				pass.set_bind_group(0, &bindGroup, &[]);
				pass.set_vertex_buffer(0, blockModelsBuffer.slice(..));
				pass.set_push_constants(
					wgpu::ShaderStages::VERTEX,
					0,
					bytemuck::bytes_of(&(sectionY as i32)),
				);
				// pass.set_push_constants(wgpu::ShaderStages::VERTEX, 4, );
				pass.multi_draw_indirect(
					indirectBuffer,
					0,
					(indirectDraws.len() / size_of::<DrawIndirect>()) as u32,
				);
				// drop(pass);
				// queue.submit(None);
			}
			// drop(pass);

			encoder.copy_texture_to_buffer(
				frameTexture.as_image_copy(),
				wgpu::ImageCopyBuffer {
					buffer: &frameCopyBuffer,
					layout: wgpu::ImageDataLayout {
						offset: 0,
						bytes_per_row: Some(
							(frameCopyBufferSize.bplPadded as u32).try_into().unwrap(),
						),
						rows_per_image: None,
					},
				},
				frameSize,
			)
		}
		let submission = queue.submit(Some(encoder.finish()));

		let slice = frameCopyBuffer.slice(..);
		slice.map_async(wgpu::MapMode::Read, |_| {});
		if !device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission)) {
			std::thread::sleep(std::time::Duration::from_secs_f32(1.5));
		}

		let padded = slice.get_mapped_range();
		let mut pixels = vec![0u8; frameCopyBufferSize.bplUnpadded * frameCopyBufferSize.height];
		let mut pixslice = &mut pixels[..];
		for chunk in padded.chunks(frameCopyBufferSize.bplPadded) {
			let len = frameCopyBufferSize.bplUnpadded;
			pixslice[0 .. len].copy_from_slice(&chunk[0 .. len]);
			pixslice = &mut pixslice[len ..];
		}
		drop(padded);
		frameCopyBuffer.unmap();

		let file = std::fs::OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(true)
			.open("out.png")
			.unwrap();
		let mut encoder = png::Encoder::new(file, frameSize.width, frameSize.height);
		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);
		let mut writer = encoder.write_header().unwrap();
		writer.write_image_data(&pixels).unwrap();
	});

	#[cfg(none)]
	{
		// searching for chunks using global palette
		let regions = loader.probe_regions(&dim);
		for (ri, &regionPos) in regions.iter().enumerate() {
			let region = loader.load_region(&dim, regionPos);
			let anvil = region.borrow_mut().load_anvil().unwrap();
			let chunks = loader.probe_chunks(&region);
			for (ci, &chunkPos) in chunks.iter().enumerate() {
				// let chunk: nbt::Blob = anvil.load_chunk(chunkPos).unwrap();
				// dbg!(&chunk);
				// todo!();
				let chunk: mc1_18::Chunk = anvil.load_chunk(chunkPos).unwrap();
				if ci % 8 == 0 || ci == chunks.len() - 1 {
					eprint!(
						"\rregion {}/{} ({},{}) chunk {}/{}            ",
						ri + 1,
						regions.len(),
						regionPos.x,
						regionPos.z,
						ci + 1,
						chunks.len()
					);
				}
			}
			eprintln!();
		}

		return;
	}
}

fn blockpos_rng(pos: BlockPos) -> usize {
	let mut hasher = DefaultHasher::new();
	pos.hash(&mut hasher);
	hasher.finish() as usize
}

#[derive(Clone, Copy, Debug)]
struct ImgBufferSize {
	pub width: usize,
	pub height: usize,
	pub bplUnpadded: usize,
	pub bplPadded: usize,
}

impl ImgBufferSize {
	pub fn new(extent: wgpu::Extent3d) -> Self {
		let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
		let bpl = extent.width * std::mem::size_of::<u32>() as u32;
		let padding = (align - bpl % align) % align;
		Self {
			width: extent.width as usize,
			height: extent.height as usize,
			bplUnpadded: bpl as usize,
			bplPadded: (bpl + padding) as usize,
		}
	}
}

#[cfg(none)]
pub fn parse_nbt_value<T: DeserializeOwned>(v: &nbt::Value) -> Result<T, nbt::Error> {
	use serde::de::DeserializeOwned;
	let mut buf = Vec::new();
	buf.resize(v.len_bytes(), Default::default());
	v.to_writer(&mut buf)?;
	nbt::from_reader(buf.as_slice())
}
