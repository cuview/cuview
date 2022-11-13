use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Seek};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use zip::read::ZipFile;
use zip::result::ZipResult;
use zip::{ZipArchive, ZipWriter};

use crate::types::resource_location::ResourceKind;

enum ZipInput {
	File(File),
	Memory(Cursor<Vec<u8>>),
}

impl Read for ZipInput {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		match self {
			ZipInput::File(src) => src.read(buf),
			ZipInput::Memory(src) => src.read(buf),
		}
	}
}

impl Seek for ZipInput {
	fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
		match self {
			ZipInput::File(src) => src.seek(pos),
			ZipInput::Memory(src) => src.seek(pos),
		}
	}
}

struct JarFile {
	path: PathBuf,
	zipfile: RefCell<ZipArchive<ZipInput>>,
}

impl JarFile {
	pub fn new(path: &Path) -> io::Result<Self> {
		Ok(Self {
			path: path.to_owned(),
			zipfile: ZipArchive::new(ZipInput::File(File::open(path)?))?.into(),
		})
	}

	pub fn from_memory(filename: &Path, zip: Vec<u8>) -> anyhow::Result<Self> {
		let path = {
			let mut p = PathBuf::from("::memory::");
			p.push(filename);
			p
		};
		Ok(Self {
			path,
			zipfile: ZipArchive::new(ZipInput::Memory(Cursor::new(zip)))?.into(),
		})
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InsertJar {
	Before,
	After,
}

pub struct JarFS(Vec<JarFile>);

impl JarFS {
	pub fn new<P: AsRef<Path>>(paths: Vec<P>) -> anyhow::Result<Self> {
		if paths.is_empty() {
			return Err(anyhow!("no jars specified"));
		}

		let mut jars = Vec::with_capacity(paths.len());
		for path in paths {
			jars.push(JarFile::new(path.as_ref())?);
		}

		let new = Self(jars);
		if new.read("assets/.mcassetsroot").is_err() {
			return Err(anyhow!("no Minecraft client jar specified"));
		}
		Ok(new)
	}

	pub fn insert_jar(
		&mut self,
		filename: &Path,
		zip: Vec<u8>,
		insert: InsertJar,
	) -> anyhow::Result<()> {
		let jar = JarFile::from_memory(filename, zip)?;
		match insert {
			InsertJar::Before => self.0.insert(0, jar),
			InsertJar::After => self.0.push(jar),
		}
		Ok(())
	}

	pub fn all_files(&self) -> BTreeSet<PathBuf> {
		let mut res = BTreeSet::new();
		for jar in &self.0 {
			res.extend(
				jar.zipfile
					.borrow()
					.file_names()
					.filter(|s| !s.ends_with("/"))
					.map(Into::into),
			);
		}
		res
	}

	pub fn files(&self, kind: ResourceKind) -> BTreeSet<PathBuf> {
		let mut files = self.all_files();
		files.retain(|path| {
			let components: Vec<_> = path
				.components()
				.map(|v| v.as_os_str().to_str().unwrap())
				.collect();
			let extension = path.extension().unwrap_or_default();
			match components.as_slice() {
				["assets", _, "blockstates", ..] => {
					kind == ResourceKind::BlockState &&
						extension == ResourceKind::BlockState.extension()
				},
				["assets", _, "models", "block", ..] => {
					kind == ResourceKind::Model && extension == ResourceKind::Model.extension()
				},
				["assets", _, "textures", ..] => {
					kind == ResourceKind::Texture && extension == ResourceKind::Texture.extension()
				},
				_ => false,
			}
		});
		files
	}

	#[rustfmt::skip]
	pub fn read(&self, path: impl AsRef<Path> + std::fmt::Debug) -> anyhow::Result<Vec<u8>> {
		for jar in self.0.iter().rev() /* reversed for overrides */ {
			if let Ok(mut file) = jar.zipfile.borrow_mut().by_name(path.as_ref().to_str().unwrap()) {
				let mut buf = Vec::with_capacity(file.size() as usize);
				file.read_to_end(&mut buf);
				return Ok(buf);
			}
		}

		Err(anyhow!("Path `{path:?}` could not be found in any loaded jars"))
	}

	pub fn read_text(&self, path: &Path) -> anyhow::Result<String> {
		Ok(String::from_utf8(self.read(path)?)?)
	}
}
