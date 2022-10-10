use std::collections::HashMap;

use serde::Deserialize;

use crate::types::{IString, ResourceLocation};

#[derive(Clone, Debug, Deserialize)]
pub struct BlockStates(pub HashMap<ResourceLocation, BlockDefinition>);

#[derive(Clone, Debug, Deserialize)]
pub struct BlockDefinition {
	pub properties: Option<HashMap<IString, Vec<IString>>>,

	pub states: Vec<State>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct State {
	pub properties: Option<HashMap<IString, IString>>,

	pub id: u32,

	#[serde(default)]
	pub default: bool,
}
