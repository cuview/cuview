use std::fs::File;
use std::path::Path;

pub mod mc1_18;

pub fn identify_version(worldRoot: impl AsRef<Path>) -> Option<(u8, u8, u8)> {
	let mut levelDat = File::open(worldRoot.as_ref().join("level.dat")).ok()?;
	let nbt: nbt::Blob = nbt::from_gzip_reader(&mut levelDat).ok()?;
	let nbt = nbt.get("Data")?;
	
	let ver = match nbt {
		nbt::Value::Compound(map) => map.get("Version"),
		_ => None,
	}?;
	let ver = match ver {
		nbt::Value::Compound(map) => map.get("Name"),
		_ => None,
	}?;
	let ver = match ver {
		nbt::Value::String(s) => Some(s),
		_ => None,
	}?;
	
	let (v1, rest) = ver.split_once(".")?;
	let (v2, v3) = rest.split_once(".").unwrap_or((rest, "0"));
	Some((v1.parse().ok()?, v2.parse().ok()?, v3.parse().ok()?))
}
