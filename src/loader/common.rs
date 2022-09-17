use std::convert::TryInto;
use std::io::{self, Read};
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::types::{ChunkPos, RegionPos};

#[derive(Debug)]
pub struct AnvilRegion {
	pos: RegionPos,
	bytes: Vec<u8>,
	chunkOffsets: [(usize, usize); 1024],
}

impl AnvilRegion {
	pub fn new(regionDir: impl AsRef<Path>, pos: RegionPos) -> Result<Self, std::io::Error> {
		let regionFile = regionDir
			.as_ref()
			.join(format!("r.{}.{}.mca", pos.x, pos.z));
		let regionFileName = regionFile.display();
		let mut file = std::fs::File::open(&regionFile)?;

		let fileLen = file.metadata()?.len() as usize;
		if fileLen & 0xFFF != 0 {
			return Err(io::Error::new(
				io::ErrorKind::Other,
				format!("{regionFileName}: file size is not a multiple of 4KiB"),
			));
		}

		let mut bytes = Vec::with_capacity(fileLen);
		file.read_to_end(&mut bytes)?;

		let mut chunkOffsets = [(0usize, 0usize); 1024];
		for index in 0 .. chunkOffsets.len() {
			let packed = u32::from_be_bytes(bytes[index * 4 .. index * 4 + 4].try_into().unwrap());
			let offset = (packed & 0xFF_FF_FF_00) >> 8;
			let len = packed & 0xFF;
			chunkOffsets[index] = ((offset as usize) * 4096, (len as usize) * 4096);
		}

		Ok(Self {
			pos,
			bytes,
			chunkOffsets,
		})
	}

	fn get_offsets(&self, pos: ChunkPos) -> (usize, usize) {
		let pos = pos.region_relative();
		self.chunkOffsets[(pos.z * RegionPos::diameterChunks + pos.x) as usize]
	}

	fn get_compressed_chunk(&self, pos: ChunkPos) -> &[u8] {
		let regionPos = self.pos;
		debug_assert!(
			!self.is_empty(pos),
			"Attempt to load compressed chunk at {pos:?} but it is empty (region {regionPos:?})"
		);
		let otherRegion = RegionPos::from(pos);
		debug_assert!(
			otherRegion == self.pos,
			"Attempt to get compressed chunk {pos:?} belonging to different region: belongs to \
			 {otherRegion:?} but is being requested from {regionPos:?}"
		);

		let (offset, len) = self.get_offsets(pos);
		&self.bytes[offset .. offset + len]
	}

	pub fn is_empty(&self, pos: ChunkPos) -> bool {
		self.get_offsets(pos).1 == 0
	}

	pub fn load_chunk<T: DeserializeOwned>(&self, pos: ChunkPos) -> Result<T, nbt::Error> {
		let regionPos = self.pos;
		let raw = self.get_compressed_chunk(pos);
		assert!(raw.len() > 5);

		let len = u32::from_be_bytes(raw[0 .. 4].try_into().unwrap());
		assert!(
			len as usize <= raw.len() - 4,
			"Raw chunk {pos:?} (region {regionPos:?}) has bad length in header"
		);

		let compression = raw[4];
		match compression {
			1 => nbt::from_gzip_reader(&raw[5 ..]),
			2 => nbt::from_zlib_reader(&raw[5 ..]),
			_ => panic!(
				"Raw chunk {pos:?} (region {regionPos:?}) has bad compression scheme in header"
			),
		}
	}
}
