struct VIn {
	@builtin(instance_index)
	instance: u32,

	@location(0)
	pos: vec3<f32>,

	@location(1)
	uv: vec2<f32>,

	@location(2)
	texId: u32,
}

struct VOut {
	@builtin(position)
	pos: vec4<f32>,

	@location(0)
	uv: vec2<f32>,

	@interpolate(flat)
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
var atlas: texture_2d_array<f32>;

struct Immediates {
	chunkPos: vec2<i32>,
	section: i32,
}

var<immediate> immediates: Immediates;

fn translationMat(t: vec3<f32>) -> mat4x4<f32> {
	return mat4x4<f32>(
		vec4<f32>(1.0, 0.0, 0.0, 0.0),
		vec4<f32>(0.0, 1.0, 0.0, 0.0),
		vec4<f32>(0.0, 0.0, 1.0, 0.0),
		vec4<f32>(t, 1.0),
	);
}

fn rotationMat(axis: vec3<f32>, angle: f32) -> mat4x4<f32> {
	// adapted from DPlug: https://github.com/AuburnSounds/Dplug/blob/141d8b5d5896da29/math/dplug/math/matrix.d#L551-L578
	let cos = cos(angle);
	let invCos = 1.0 - cos;
	let sin = sin(angle);
	let x = axis.x;
	let y = axis.y;
	let z = axis.z;
	return mat4x4<f32>(
		vec4(x * x * invCos + cos,
		x * y * invCos - z * sin,
		x * z * invCos + y * sin, 0.0,),
		vec4(y * x * invCos + z * sin,
		y * y * invCos + cos,
		y * z * invCos - x * sin, 0.0),
		vec4(z * x * invCos - y * sin,
		z * y * invCos + x * sin,
		z * z * invCos + cos, 0.0),
		vec4(0.0, 0.0, 0.0, 1.0),
	);
}

fn blockTranslation(rawBlockId: u32) -> vec3<f32> {
	let chunkWidth: u32 = u32(16);
	let blocksInLayer: u32 = chunkWidth * chunkWidth;

	let iy = rawBlockId / blocksInLayer;
	let blockId = rawBlockId - iy * blocksInLayer;
	let tz = f32(blockId / chunkWidth);
	let tx = f32(blockId % chunkWidth);

	// section translation
	let ty = f32(iy) + 16.0 * f32(immediates.section);

	// debugging
	// let tx = tx + 16.0 * f32(section);

	let chunkPos = vec3<f32>(f32(immediates.chunkPos.x), 0, f32(immediates.chunkPos.y)) * 16;
	return vec3<f32>(tx, ty, tz) + chunkPos;
}


@vertex
fn vsMain(in: VIn) -> VOut {
	let instance = in.instance & 4095u;
	let tau = 6.28318530717958647692528676655900577;
	let rotX = f32((in.instance & 1023u << 12u) >> 12u) / 1024.0 * tau;
	let rotY = f32((in.instance & 1023u << 22u) >> 22u) / 1024.0 * tau;
	let model =
		translationMat(blockTranslation(instance)) *
		// TODO: assuming models fit into unit cube
		translationMat(vec3(0.5)) *
		rotationMat(vec3(0.0, 1.0, 0.0), rotY) *
		rotationMat(vec3(1.0, 0.0, 0.0), rotX) *
		translationMat(vec3(-0.5))
	;

	let pos = camera.projection * camera.view * model * vec4<f32>(in.pos, 1.0);

	let texLayer = (in.texId & (0xFFu << 24u)) >> 24u;
	let texId = in.texId & 0xFFFFFFu;

	let diameter = atlasDiameters[texLayer];
	let atlasSize = textureDimensions(atlas);
	let scale = f32(diameter) / vec2<f32>(atlasSize);
	let widthInElems = u32(atlasSize.x) / u32(diameter);
	let offset = vec2<f32>(
		f32(texId % widthInElems),
		f32(texId / widthInElems),
	);

	var uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y); // origin swap
	uv = scale * uv + scale * offset;

	return VOut(
		pos,
		uv,
		texLayer,
	);
}

@group(0)
@binding(3)
var atlasSampler: sampler;

@fragment
fn fsMain(in: VOut) -> @location(0) vec4<f32> {
	let res = textureSample(atlas, atlasSampler, in.uv, in.texLayer);

	// cheap hack to fix blending of overlapping transparency
	if res.a <= 5.0 / 255.0 { discard; }
	return res;
}
