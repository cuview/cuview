use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use glam::{ivec2, uvec2, IVec2, UVec2};

use super::model::ModelCache;
use crate::jarfs::JarFS;
use crate::types::resource_location::ResourceKind;
use crate::types::ResourceLocation;

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
	texDiameter: usize,
	entries: Vec<ResourceLocation>,
}

impl Atlas {
	fn new(id: u8, texDiameter: usize) -> Self {
		Self {
			id,
			texDiameter,
			entries: vec![],
		}
	}

	fn max_entries(&self, maxTextureDiameter: usize) -> usize {
		let maxSize = UVec2::splat((maxTextureDiameter / self.texDiameter) as u32);
		(maxSize.x * maxSize.y) as usize
	}

	fn full(&self, maxTextureDiameter: usize) -> bool {
		self.entries.len() >= self.max_entries(maxTextureDiameter)
	}

	fn merged_size(&self, maxTextureDiameter: usize) -> UVec2 {
		let width = (maxTextureDiameter / self.texDiameter) as u32;
		let len = self.entries.len() as u32;
		let y = len / width;
		let x = if y == 0 { len % width } else { width };
		let res = uvec2(x, y) * UVec2::splat(self.texDiameter as u32);
		// powers of two required for mipmapping
		let res = uvec2(res.x.next_power_of_two(), res.y.next_power_of_two());
		assert!(res.x <= maxTextureDiameter as u32);
		assert!(res.y <= maxTextureDiameter as u32);
		res
	}

	fn origin(&self, maxTextureDiameter: usize, tid: u32) -> UVec2 {
		let width = (maxTextureDiameter / self.texDiameter) as u32;
		uvec2(tid % width, tid / width) * UVec2::splat(self.texDiameter as u32)
	}
}

#[derive(Debug)]
pub struct Cartographer {
	pub size: UVec2,
	pub textures: HashMap<ResourceLocation, TextureId>,
	elementDiameters: Vec<u32>,
}

impl Cartographer {
	pub fn load(
		fs: &JarFS,
		models: &ModelCache,
		device: &wgpu::Device,
	) -> anyhow::Result<(Self, Vec<Image>)> {
		let limits = device.limits();
		assert!(limits.max_texture_array_layers >= u8::MAX as u32);
		let maxTextureDiameter = limits.max_texture_dimension_3d as usize;
		let mut images = HashMap::new();
		let mut textures = HashMap::new();
		let mut atlases: Vec<Atlas> = Vec::with_capacity(u8::MAX as usize);

		let mut add_texture = |loc: ResourceLocation, img: Image| {
			let diameter = img.size.x as usize;
			let atlas = if let Some(atlas) = atlases
				.iter_mut()
				.filter(|a| a.texDiameter == diameter && !a.full(maxTextureDiameter))
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
	
		let missingTex = "cuview:missing_texture".into();
		let missingTexImage = missing_texture(0xFF_FF00FF);
		add_texture(missingTex, missingTexImage.clone());
		
		for loc in models
			.all_block_textures()
			.into_iter()
			.collect::<BTreeSet<_>>()
		{
			let path = loc.into_path(ResourceKind::Texture);
			let mut image = Image::from_jarfs(fs, &path).unwrap_or_else(|_| missingTexImage.clone());

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
					let srcModels: BTreeSet<_> =
						models.models_using_texture(loc).into_iter().collect();
					eprintln!(
						"texture {path:?} is not square ({width}x{height}, used by models: \
						 {srcModels:?})"
					);

					// TODO: properly handling this will require more sophisticated texture packing
					// and should probably just spill any models using such textures into
					// the (future) .obj pipeline
					image = image.crop(UVec2::splat(width.min(height)));
				}
			}

			add_texture(loc, image);
		}

		let diameters: Vec<_> = atlases.iter().map(|a| a.texDiameter as u32).collect();
		let layerSize = atlases
			.iter()
			.map(|a| a.merged_size(maxTextureDiameter))
			.fold(UVec2::splat(0), |res, v| {
				uvec2(res.x.max(v.x), res.y.max(v.y))
			});
		let mut layers = Vec::with_capacity(atlases.len());
		for (aid, atlas) in atlases.iter().enumerate() {
			let mut layer = Image::empty(layerSize);
			let destSize = layer.size;
			for (tid, tex) in atlas.entries.iter().copied().enumerate() {
				let srcImage = images.get(&tex).unwrap();
				let srcSize = srcImage.size;
				let origin = atlas.origin(maxTextureDiameter, tid as u32);
				layer.blit_from(srcImage, origin, None);
			}
			layers.push(layer);
		}

		let new = Self {
			size: layerSize,
			textures,
			elementDiameters: diameters,
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
		self.elementDiameters.len()
	}

	pub fn element_diameters(&self) -> &[u32] {
		&self.elementDiameters
	}
}

fn missing_texture(color: u32) -> Image {
	const diameter: u32 = 16;
	let color = Image::solid_color(UVec2::splat(diameter / 2), color);
	let mut img = Image::empty(UVec2::splat(diameter));
	img.blit_from(&color, UVec2::ZERO, None);
	img.blit_from(&color, UVec2::splat(diameter / 2), None);
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
		let mut decoder = png::Decoder::new(bytes);
		decoder.set_ignore_text_chunk(true);
		decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
		decoder.set_limits(png::Limits {
			bytes: 32 * 1024 * 1024,
			..Default::default()
		});

		let mut reader = decoder.read_info()?;
		let mut srcPixels = vec![0u8; reader.output_buffer_size()];
		let info = reader.next_frame(&mut srcPixels).unwrap();
		assert_eq!(info.bit_depth, png::BitDepth::Eight);

		let (width, height) = (info.width as usize, info.height as usize);
		let pixels = match info.color_type {
			png::ColorType::Rgba => {
				// cannot `cast_vec` due to misaligned `Vec<u8>`s :\
				bytemuck::cast_slice(&srcPixels).to_vec()
			},
			png::ColorType::Rgb => {
				let chunks = srcPixels.chunks_exact(3);
				assert!(chunks.remainder().is_empty());
				chunks
					.map(|chunk| {
						u32::from_be_bytes([
							0xFF, chunk[2], chunk[1], chunk[0],
						])
					})
					.collect()
			},
			png::ColorType::Grayscale => {
				assert_eq!(srcPixels.len(), width * height);
				srcPixels
					.into_iter()
					.map(|v| {
						let v = v as u32;
						0xFF << 24 | v << 16 | v << 8 | v
					})
					.collect()
			},
			png::ColorType::GrayscaleAlpha => {
				let chunks = srcPixels.chunks_exact(2);
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

	pub fn blit_from(&mut self, src: &Self, destOrigin: UVec2, srcSize: Option<UVec2>) {
		let size = srcSize.unwrap_or(src.size);
		assert!(size.x <= src.size.x && size.y <= src.size.y);
		assert!(destOrigin.x <= self.size.x - size.x);
		assert!(destOrigin.y <= self.size.y - size.y);
		for sy in 0 .. size.y {
			fn index(pos: UVec2, width: u32) -> usize {
				(pos.y * width + pos.x) as usize
			}

			let srcSlice =
				&src.pixels[index(uvec2(0, sy), size.x) .. index(uvec2(0, sy + 1), size.x)];
			let dy = destOrigin.y + sy;
			let destSlice = &mut self.pixels[index(uvec2(destOrigin.x, dy), self.size.x) ..
				index(uvec2(destOrigin.x + size.x, dy), self.size.x)];
			destSlice.copy_from_slice(srcSlice);
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
		pixels: vec![
			0xFFFF_FFFF,
			0xFFFF_0000,
		],
	};

	dest.blit_from(&src, uvec2(0, 0), None);
	assert_eq!(
		dest.pixels,
		[
			0xFFFF_FFFF,
			0xFFFF_0000,
			0,
			0
		]
	);

	dest.blit_from(&src, uvec2(0, 1), None);
	assert_eq!(
		dest.pixels,
		[
			0xFFFF_FFFF,
			0xFFFF_0000,
			0xFFFF_FFFF,
			0xFFFF_0000
		]
	);

	dest.pixels.fill(0);
	dest.blit_from(&src, uvec2(0, 0), Some(uvec2(1, 1)));
	assert_eq!(dest.pixels, [0xFFFF_FFFF, 0, 0, 0]);

	for height in 1u32 .. 6 {
		const width: u32 = 2;
		let mut pixels: Vec<_> = (1 ..= width * height).collect();
		let mut img = Image {
			size: uvec2(width, height),
			pixels: pixels.clone(),
		};
		img.flip_y();
		(bytemuck::cast_slice_mut::<u32, [u32; 2]>(&mut pixels)).reverse();
		assert_eq!(img.pixels, pixels);
	}
}
