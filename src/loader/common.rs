use std::{
	convert::TryInto,
	io::{self, Read},
	path::Path,
};

use serde::de::DeserializeOwned;

use crate::types::{ChunkPos, RegionPos};

#[derive(Debug)]
pub struct AnvilRegion {
	pos: RegionPos,
	bytes: Vec<u8>,
	chunk_offsets: [(usize, usize); 1024],
}

impl AnvilRegion {
	pub fn new(region_dir: impl AsRef<Path>, pos: RegionPos) -> Result<Self, std::io::Error> {
		let region_file = region_dir
			.as_ref()
			.join(format!("r.{}.{}.mca", pos.x, pos.z));
		let region_file_name = region_file.display();
		let mut file = std::fs::File::open(&region_file)?;

		let file_len = file.metadata()?.len() as usize;
		if file_len & 0xFFF != 0 {
			return Err(io::Error::new(
				io::ErrorKind::Other,
				format!("{region_file_name}: file size is not a multiple of 4KiB"),
			));
		}

		let mut bytes = Vec::with_capacity(file_len);
		file.read_to_end(&mut bytes)?;

		let mut chunk_offsets = [(0usize, 0usize); 1024];
		for index in 0 .. chunk_offsets.len() {
			let packed = u32::from_be_bytes(bytes[index * 4 .. index * 4 + 4].try_into().unwrap());
			let offset = (packed & 0xFF_FF_FF_00) >> 8;
			let len = packed & 0xFF;
			chunk_offsets[index] = ((offset as usize) * 4096, (len as usize) * 4096);
		}

		Ok(Self {
			pos,
			bytes,
			chunk_offsets,
		})
	}

	fn get_offsets(&self, pos: ChunkPos) -> (usize, usize) {
		let pos = pos.region_relative();
		self.chunk_offsets[(pos.z * RegionPos::DIAMETER_CHUNKS + pos.x) as usize]
	}

	fn get_compressed_chunk(&self, pos: ChunkPos) -> &[u8] {
		let region_pos = self.pos;
		debug_assert!(
			!self.is_empty(pos),
			"Attempt to load compressed chunk at {pos:?} but it is empty (region {region_pos:?})"
		);
		let other_region = RegionPos::from(pos);
		debug_assert!(
			other_region == self.pos,
			"Attempt to get compressed chunk {pos:?} belonging to different region: belongs to \
			 {other_region:?} but is being requested from {region_pos:?}"
		);

		let (offset, len) = self.get_offsets(pos);
		&self.bytes[offset .. offset + len]
	}

	pub fn is_empty(&self, pos: ChunkPos) -> bool {
		self.get_offsets(pos).1 == 0
	}

	pub fn load_chunk<T: DeserializeOwned>(&self, pos: ChunkPos) -> Result<T, nbt::Error> {
		let region_pos = self.pos;
		let raw = self.get_compressed_chunk(pos);
		assert!(raw.len() > 5);

		let len = u32::from_be_bytes(raw[0 .. 4].try_into().unwrap());
		assert!(
			len as usize <= raw.len() - 4,
			"Raw chunk {pos:?} (region {region_pos:?}) has bad length in header"
		);

		let compression = raw[4];
		match compression {
			1 => nbt::from_gzip_reader(&raw[5 ..]),
			2 => nbt::from_zlib_reader(&raw[5 ..]),
			_ => panic!(
				"Raw chunk {pos:?} (region {region_pos:?}) has bad compression scheme in header"
			),
		}
	}
}

pub fn biterator(bits: usize, mut words: &[u64]) -> impl '_ + Iterator<Item = u32> {
	let bits = bits as u32;
	let mask = (1 << bits) - 1;
	let mut current_word = words[0];
	words = &words[1 ..];
	let mut bits_remaining = u64::BITS;
	std::iter::from_fn(move || {
		if bits_remaining == 0 && words.len() == 0 {
			None
		} else {
			if bits_remaining == 0 {
				current_word = words[0];
				words = &words[1 ..];
				bits_remaining = u64::BITS;
			}

			let elem = current_word & mask;
			current_word >>= bits;
			if let Some(v) = bits_remaining.checked_sub(bits) {
				bits_remaining = v;
				// TODO: <=1.15 wraps entries across words
				if bits_remaining < bits {
					bits_remaining = 0;
				}
			} else {
				bits_remaining = 0;
			}
			Some(elem as u32)
		}
	})
}

#[test]
fn test_biterator() {
	let inp: Vec<u64> = (0 .. 256).collect();
	let res: Vec<u32> = biterator(4, &inp).collect();
	assert_eq!(res.len(), 4096);
}
