struct VIn {
	@builtin(instance_index)
	instance: u32,
	
	@location(0)
	pos: vec3<f32>,
	
	@location(1)
	uv: vec2<f32>,
}

struct VOut {
	@builtin(position)
	pos: vec4<f32>,
	
	@location(0)
	uv: vec2<f32>,
}

struct Camera {
	projection: mat4x4<f32>,
	view: mat4x4<f32>,
}

@group(0)
@binding(0)
var<uniform> camera: Camera;

var<push_constant> section: i32;

fn translation(id: u32) -> vec3<f32> {
	let chunkWidth: u32 = u32(16);
	let blocksInLayer: u32 = chunkWidth * chunkWidth;
	
	let ty = id / blocksInLayer;
	let id = id - ty * blocksInLayer;
	let tz = f32(id / chunkWidth);
	let tx = f32(id % chunkWidth);
	
	// section translation
	let ty = f32(ty) + 16.0 * f32(section);
	
	// debugging
	let tx = tx + 16.0 * f32(section);
	
	return vec3<f32>(tx, ty, tz);
}

@vertex
fn vsMain(in: VIn) -> VOut {
	let model = mat4x4<f32>(
		vec4<f32>(1.0, 0.0, 0.0, 0.0),
		vec4<f32>(0.0, 1.0, 0.0, 0.0),
		vec4<f32>(0.0, 0.0, 1.0, 0.0),
		vec4<f32>(translation(in.instance), 1.0),
	);
	let pos = camera.projection * camera.view * model * vec4<f32>(in.pos, 1.0);
	return VOut(
		pos,
		in.uv,
	);
}

@fragment
fn fsMain(in: VOut) -> @location(0) vec4<f32> {
	return vec4<f32>(in.uv, 0.0, 1.0);
}
