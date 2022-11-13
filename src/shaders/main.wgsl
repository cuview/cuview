struct VIn {
	@builtin(instance_index)
	instance: u32,
	
	@location(0)
	pos: vec3<f32>,
	
	@location(1)
	uv: vec2<f32>,
	
	@location(2)
	texSlot: u32,
}

struct VOut {
	@builtin(position)
	pos: vec4<f32>,
	
	@location(0)
	uv: vec2<f32>,
	
	@location(1)
	texLayer: u32,
}

struct Camera {
	projection: mat4x4<f32>,
	view: mat4x4<f32>,
}

@group(0)
@binding(0)
var<uniform> camera: Camera;

@group(0)
@binding(1)
var<storage, read> atlasDiameters: array<u32>;

@group(0)
@binding(2)
var<storage, read> texturesForSlot: array<u32>;

@group(0)
@binding(3)
var atlas: texture_2d_array<f32>;

var<push_constant> section: i32;

fn translation(blockId: u32) -> vec3<f32> {
	let chunkWidth: u32 = u32(16);
	let blocksInLayer: u32 = chunkWidth * chunkWidth;
	
	let ty = blockId / blocksInLayer;
	let blockId = blockId - ty * blocksInLayer;
	let tz = f32(blockId / chunkWidth);
	let tx = f32(blockId % chunkWidth);
	
	// section translation
	let ty = f32(ty) + 16.0 * f32(section);
	
	// debugging
	let tx = tx + 16.0 * f32(section);
	
	return vec3<f32>(tx, ty, tz);
}

@vertex
fn vsMain(in: VIn) -> VOut {
	let blockId = in.instance & 0xFFFu;
	let baseSlot = in.instance >> 12u;
	
	let model = mat4x4<f32>(
		vec4<f32>(1.0, 0.0, 0.0, 0.0),
		vec4<f32>(0.0, 1.0, 0.0, 0.0),
		vec4<f32>(0.0, 0.0, 1.0, 0.0),
		vec4<f32>(translation(blockId), 1.0),
	);
	let pos = camera.projection * camera.view * model * vec4<f32>(in.pos, 1.0);
	
	let texId = texturesForSlot[baseSlot + in.texSlot];
	let texLayer = (texId & (0xFFu << 24u)) >> 24u;
	let texId = texId & 0xFFFFFFu;
	
	let diameter = atlasDiameters[texLayer];
	let atlasSize = textureDimensions(atlas);
	let scale = f32(diameter) / vec2<f32>(atlasSize);
	let widthInElems = u32(atlasSize.x) / u32(diameter);
	let offset = vec2<f32>(
		f32(texId % widthInElems),
		f32(texId / widthInElems),
	);
	
	let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y); // origin swap
	// let uv = in.uv;
	let uv = scale * uv + scale * offset;
	
	return VOut(
		pos,
		uv,
		texLayer,
	);
}

@group(0)
@binding(4)
var atlasSampler: sampler;

@fragment
fn fsMain(in: VOut) -> @location(0) vec4<f32> {
	// FIXME: currently (0.9.0) Naga does not respect spec and only accepts i32s
	let layer = i32(in.texLayer);
	let res =  textureSample(atlas, atlasSampler, in.uv, layer);
	
	// cheap hack to fix blending of overlapping transparency
	if res.a <= 5.0 / 255.0 { discard; }
	return res;
}
