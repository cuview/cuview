use std::{
	collections::{BTreeSet, HashMap, HashSet},
	path::Path,
};

use anyhow::Context;
use glam::{IVec2, UVec2, ivec2, uvec2};

use super::model::ModelCache;
use crate::{
	jarfs::JarFS,
	types::{ResourceLocation, resource_location::ResourceKind},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId {
	pub atlas: u8,
	pub texture: u32,
}

impl TextureId {
	pub fn packed(&self) -> u32 {
		assert!(self.texture < 1 << 24);
		(self.atlas as u32) << 24 | self.texture
	}
}

#[derive(Clone, Copy, Debug)]
struct Rect {
	mins: IVec2,
	maxs: IVec2,
}

impl Rect {
	fn new(p1: IVec2, p2: IVec2) -> Self {
		Self {
			mins: ivec2(p1.x.min(p2.x), p1.y.min(p2.y)),
			maxs: ivec2(p1.x.max(p2.x), p1.y.max(p2.y)),
		}
	}

	fn size(&self) -> UVec2 {
		(self.maxs - self.mins).as_uvec2()
	}
}

#[derive(Debug)]
struct Atlas {
	id: u8,
	tex_diameter: usize,
	entries: Vec<ResourceLocation>,
}

impl Atlas {
	fn new(id: u8, tex_diameter: usize) -> Self {
		Self {
			id,
			tex_diameter,
			entries: vec![],
		}
	}

	fn max_entries(&self, max_texture_diameter: usize) -> usize {
		let max_size = UVec2::splat((max_texture_diameter / self.tex_diameter) as u32);
		(max_size.x * max_size.y) as usize
	}

	fn full(&self, max_texture_diameter: usize) -> bool {
		self.entries.len() >= self.max_entries(max_texture_diameter)
	}

	fn merged_size(&self, max_texture_diameter: usize) -> UVec2 {
		let width = (max_texture_diameter / self.tex_diameter) as u32;
		let len = self.entries.len() as u32;
		let y = len / width;
		let x = if y == 0 { len % width } else { width };
		let res = uvec2(x, y) * UVec2::splat(self.tex_diameter as u32);
		// powers of two required for mipmapping
		let res = uvec2(res.x.next_power_of_two(), res.y.next_power_of_two());
		assert!(res.x <= max_texture_diameter as u32);
		assert!(res.y <= max_texture_diameter as u32);
		res
	}

	fn origin(&self, max_texture_diameter: usize, tid: u32) -> UVec2 {
		let width = (max_texture_diameter / self.tex_diameter) as u32;
		uvec2(tid % width, tid / width) * UVec2::splat(self.tex_diameter as u32)
	}
}

#[derive(Debug)]
pub struct Cartographer {
	pub size: UVec2,
	pub textures: HashMap<ResourceLocation, TextureId>,
	element_diameters: Vec<u32>,
}

impl Cartographer {
	pub fn load(
		fs: &JarFS,
		models: &ModelCache,
		device: &wgpu::Device,
	) -> anyhow::Result<(Self, Vec<Image>)> {
		let limits = device.limits();
		assert!(limits.max_texture_array_layers >= u8::MAX as u32);
		let max_texture_diameter = limits.max_texture_dimension_3d as usize;
		let mut images = HashMap::new();
		let mut textures = HashMap::new();
		let mut atlases: Vec<Atlas> = Vec::with_capacity(u8::MAX as usize);

		let mut add_texture = |loc: ResourceLocation, img: Image| {
			let diameter = img.size.x as usize;
			let atlas = if let Some(atlas) = atlases
				.iter_mut()
				.filter(|a| a.tex_diameter == diameter && !a.full(max_texture_diameter))
				.next()
			{
				atlas
			} else {
				let id = atlases.len();
				assert!(id < u8::MAX as usize);
				atlases.push(Atlas::new(id as u8, diameter));
				&mut atlases[id]
			};
			let id = atlas.entries.len();
			atlas.entries.push(loc);

			let tid = TextureId {
				atlas: atlas.id,
				texture: id as u32,
			};
			textures.insert(loc, tid);
			images.insert(loc, img);
		};

		let missing_tex = "cuview:missing_texture".into();
		let missing_tex_image = missing_texture(0xFF_FF00FF);
		add_texture(missing_tex, missing_tex_image.clone());

		for loc in models
			.all_block_textures()
			.into_iter()
			.collect::<BTreeSet<_>>()
		{
			let path = loc.into_path(ResourceKind::Texture);
			let mut image =
				Image::from_jarfs(fs, &path).unwrap_or_else(|_| missing_tex_image.clone());

			let UVec2 {
				x: width,
				y: height,
			} = image.size;
			if width != height {
				let mut path = path;
				path.set_extension(ResourceKind::TextureMeta.extension());
				if let Ok(json) = fs.read_text(&path) {
					path.set_extension("");
					// TODO: also actually verify that the json specifies an animation
					assert_eq!(
						height % width,
						0,
						"malformed animated texture: {path:?} is {width}x{height}"
					);

					// crop out only first frame.
					// TODO: in future this should instead register all frames, to be chosen from
					// randomly per block
					image = image.crop(UVec2::splat(width));
				} else {
					path.set_extension("");
					let src_models: BTreeSet<_> =
						models.models_using_texture(loc).into_iter().collect();
					eprintln!(
						"texture {path:?} is not square ({width}x{height}, used by models: \
						 {src_models:?})"
					);

					// TODO: properly handling this will require more sophisticated texture packing
					// and should probably just spill any models using such textures into
					// the (future) .obj pipeline
					image = image.crop(UVec2::splat(width.min(height)));
				}
			}

			add_texture(loc, image);
		}

		let diameters: Vec<_> = atlases.iter().map(|a| a.tex_diameter as u32).collect();
		let layer_size = atlases
			.iter()
			.map(|a| a.merged_size(max_texture_diameter))
			.fold(UVec2::splat(0), |res, v| {
				uvec2(res.x.max(v.x), res.y.max(v.y))
			});
		let mut layers = Vec::with_capacity(atlases.len());
		for (aid, atlas) in atlases.iter().enumerate() {
			let mut layer = Image::empty(layer_size);
			let dest_size = layer.size;
			for (tid, tex) in atlas.entries.iter().copied().enumerate() {
				let src_image = images.get(&tex).unwrap();
				let src_size = src_image.size;
				let origin = atlas.origin(max_texture_diameter, tid as u32);
				layer.blit_from(src_image, origin, None);
			}
			layers.push(layer);
		}

		let new = Self {
			size: layer_size,
			textures,
			element_diameters: diameters,
		};
		Ok((new, layers))
	}

	pub fn id_for_texture(&self, tex: ResourceLocation) -> Option<TextureId> {
		self.textures.get(&tex).copied()
	}

	pub fn texture_for_id(&self, id: TextureId) -> Option<ResourceLocation> {
		let TextureId { atlas, texture } = id;
		self.textures
			.iter()
			.filter(|&(_, &id)| id.texture == texture)
			.map(|(&loc, _)| loc)
			.next()
	}

	pub fn layers(&self) -> usize {
		self.element_diameters.len()
	}

	pub fn element_diameters(&self) -> &[u32] {
		&self.element_diameters
	}
}

fn missing_texture(color: u32) -> Image {
	const DIAMETER: u32 = 16;
	let color = Image::solid_color(UVec2::splat(DIAMETER / 2), color);
	let mut img = Image::empty(UVec2::splat(DIAMETER));
	img.blit_from(&color, UVec2::ZERO, None);
	img.blit_from(&color, UVec2::splat(DIAMETER / 2), None);
	img
}

#[derive(Clone)]
pub struct Image {
	pub size: UVec2,
	pub pixels: Vec<u32>,
}

impl Image {
	pub fn empty(size: UVec2) -> Self {
		Self {
			size,
			pixels: vec![0xFF_000000; (size.x * size.y) as usize],
		}
	}

	pub fn solid_color(size: UVec2, color: u32) -> Self {
		Self {
			size,
			pixels: vec![color; (size.x * size.y) as usize],
		}
	}

	pub fn from_jarfs(fs: &JarFS, path: &Path) -> anyhow::Result<Self> {
		let bytes = fs.read(path)?;
		Self::from_png_bytes(&bytes, path)
	}

	pub fn from_png_bytes(bytes: &[u8], p: &Path) -> anyhow::Result<Self> {
		let mut cursor = std::io::Cursor::new(bytes);
		let mut decoder = png::Decoder::new(cursor);
		decoder.set_ignore_text_chunk(true);
		decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
		decoder.set_limits(png::Limits {
			bytes: 32 * 1024 * 1024,
			..Default::default()
		});

		let mut reader = decoder.read_info()?;
		let num_pixels = reader
			.output_buffer_size()
			.expect("output image is too large to fit into RAM");
		let mut src_pixels = vec![0u8; num_pixels];
		let info = reader.next_frame(&mut src_pixels).unwrap();
		assert_eq!(info.bit_depth, png::BitDepth::Eight);

		let (width, height) = (info.width as usize, info.height as usize);
		let pixels = match info.color_type {
			png::ColorType::Rgba => {
				// cannot `cast_vec` due to misaligned `Vec<u8>`s :\
				bytemuck::cast_slice(&src_pixels).to_vec()
			},
			png::ColorType::Rgb => {
				let chunks = src_pixels.chunks_exact(3);
				assert!(chunks.remainder().is_empty());
				chunks
					.map(|chunk| u32::from_be_bytes([0xFF, chunk[2], chunk[1], chunk[0]]))
					.collect()
			},
			png::ColorType::Grayscale => {
				assert_eq!(src_pixels.len(), width * height);
				src_pixels
					.into_iter()
					.map(|v| {
						let v = v as u32;
						0xFF << 24 | v << 16 | v << 8 | v
					})
					.collect()
			},
			png::ColorType::GrayscaleAlpha => {
				let chunks = src_pixels.chunks_exact(2);
				assert!(chunks.remainder().is_empty());
				chunks
					.map(|chunk| {
						let v = chunk[0] as u32;
						let a = chunk[1] as u32;
						a << 24 | v << 16 | v << 8 | v
					})
					.collect()
			},
			png::ColorType::Indexed => {
				unreachable!("should have been handled by `png::Transformations::EXPAND`")
			},
		};
		assert_eq!(pixels.len(), width * height);
		Ok(Self {
			size: uvec2(width as u32, height as u32),
			pixels,
		})
	}

	pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
		let mut file = std::fs::File::create(path)?;
		let mut encoder = png::Encoder::new(&mut file, self.size.x, self.size.y);
		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);

		let mut writer = encoder.write_header()?;
		writer.write_image_data(bytemuck::cast_slice(&self.pixels))?;
		Ok(())
	}

	pub fn blit_from(&mut self, src: &Self, dest_origin: UVec2, src_size: Option<UVec2>) {
		let size = src_size.unwrap_or(src.size);
		assert!(size.x <= src.size.x && size.y <= src.size.y);
		assert!(dest_origin.x <= self.size.x - size.x);
		assert!(dest_origin.y <= self.size.y - size.y);
		for sy in 0 .. size.y {
			fn index(pos: UVec2, width: u32) -> usize {
				(pos.y * width + pos.x) as usize
			}

			let src_slice =
				&src.pixels[index(uvec2(0, sy), size.x) .. index(uvec2(0, sy + 1), size.x)];
			let dy = dest_origin.y + sy;
			let dest_slice = &mut self.pixels[index(uvec2(dest_origin.x, dy), self.size.x) ..
				index(uvec2(dest_origin.x + size.x, dy), self.size.x)];
			dest_slice.copy_from_slice(src_slice);
		}
	}

	pub fn crop(&self, size: UVec2) -> Self {
		assert!(size.x <= self.size.x && size.y <= self.size.y);
		let mut new = Self::empty(size);
		new.blit_from(self, UVec2::ZERO, Some(size));
		new
	}

	pub fn flip_y(&mut self) {
		let [width, height] = self.size.to_array().map(|v| v as usize);
		if height < 2 {
			return;
		}

		let (mut l, mut r) = (0, height - 1);
		while l < r {
			let (li, lr) = (l * width, r * width);
			let (ls, rs) = self.pixels.split_at_mut(lr);
			(&mut ls[li .. li + width]).swap_with_slice(&mut rs[0 .. width]);
			l += 1;
			r -= 1;
		}
	}
}

impl std::fmt::Debug for Image {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Image").field("size", &self.size).finish()
	}
}

#[test]
fn test_image() {
	let mut dest = Image {
		size: uvec2(2, 2),
		pixels: vec![0u32; 4],
	};
	let src = Image {
		size: uvec2(2, 1),
		pixels: vec![0xFFFF_FFFF, 0xFFFF_0000],
	};

	dest.blit_from(&src, uvec2(0, 0), None);
	assert_eq!(dest.pixels, [0xFFFF_FFFF, 0xFFFF_0000, 0, 0]);

	dest.blit_from(&src, uvec2(0, 1), None);
	assert_eq!(dest.pixels, [
		0xFFFF_FFFF,
		0xFFFF_0000,
		0xFFFF_FFFF,
		0xFFFF_0000
	]);

	dest.pixels.fill(0);
	dest.blit_from(&src, uvec2(0, 0), Some(uvec2(1, 1)));
	assert_eq!(dest.pixels, [0xFFFF_FFFF, 0, 0, 0]);

	for height in 1u32 .. 6 {
		const WIDTH: u32 = 2;
		let mut pixels: Vec<_> = (1 ..= WIDTH * height).collect();
		let mut img = Image {
			size: uvec2(WIDTH, height),
			pixels: pixels.clone(),
		};
		img.flip_y();
		(bytemuck::cast_slice_mut::<u32, [u32; 2]>(&mut pixels)).reverse();
		assert_eq!(img.pixels, pixels);
	}
}
