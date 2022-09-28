use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

struct JarFile {
	path: PathBuf,
	zipfile: RefCell<ZipArchive<File>>,
}

impl JarFile {
	pub fn new(path: &Path) -> io::Result<Self> {
		Ok(Self {
			path: path.to_owned(),
			zipfile: ZipArchive::new(File::open(path)?)?.into(),
		})
	}
}

pub struct JarFS(Vec<JarFile>);

impl JarFS {
	pub fn new(paths: Vec<&Path>) -> io::Result<Self> {
		let mut jars = Vec::with_capacity(paths.len());
		for path in paths {
			jars.push(JarFile::new(path)?);
		}
		Ok(Self(jars))
	}

	pub fn all_files(&self) -> BTreeSet<PathBuf> {
		let mut res = BTreeSet::new();
		for jar in &self.0 {
			res.extend(jar.zipfile.borrow().file_names().map(Into::into));
		}
		res
	}

	#[rustfmt::skip]
	pub fn read(&self, path: &Path) -> anyhow::Result<Vec<u8>> {
		for jar in self.0.iter().rev() /* reversed for overrides */ {
			if let Ok(mut file) = jar.zipfile.borrow_mut().by_name(path.to_str().unwrap()) {
				let mut buf = Vec::with_capacity(file.size() as usize);
				file.read_to_end(&mut buf);
				return Ok(buf);
			}
		}

		Err(io::Error::new(
			io::ErrorKind::NotFound,
			format!("Path `{path:?}` could not be found in any loaded jars"),
		))?
	}

	pub fn read_text(&self, path: &Path) -> anyhow::Result<String> {
		Ok(String::from_utf8(self.read(path)?)?)
	}
}
