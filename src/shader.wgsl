// Vertex shader

struct Camera {
	view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
}

struct InstanceInput {
	@location(5) model_matrix_0: vec4<f32>,
	@location(6) model_matrix_1: vec4<f32>,
	@location(7) model_matrix_2: vec4<f32>,
	@location(8) model_matrix_3: vec4<f32>,
	@location(9) color: vec3<f32>,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) color: vec3<f32>,
	@location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(
	model: VertexInput,
	instance: InstanceInput,
) -> VertexOutput {
	let model_matrix = mat4x4<f32>(
		instance.model_matrix_0,
		instance.model_matrix_1,
		instance.model_matrix_2,
		instance.model_matrix_3,
	);
	var out: VertexOutput;
	out.clip_position = camera.view_proj * model_matrix * vec4<f32>(model.position, 1.0);
	out.normal = model.normal;
	out.color = instance.color;
	return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	//let light_dir4 = camera.view_proj * vec4(normalize(vec3(1.0, 2.0, 1.0)), 1.0);
	//let light_dir = light_dir4.xyz;

	// setting direction here doesnt work properly??
	let light_dir = normalize(vec3(1.0, 2.0, 1.0));

	let ind_dir = normalize(light_dir * vec3(-1.0, -0.5, -1.0));

	let sun: f32 = saturate(dot(light_dir, in.normal));
	let sky: f32 = saturate(0.5 + 0.5 * in.normal.y);
	let ind: f32 = saturate(dot(in.normal, ind_dir));
	var lighting: vec3<f32> = sun * vec3(1.64, 1.27, 0.99);
	lighting += sky * vec3(0.16, 0.20, 0.28);
	lighting += ind * vec3(0.60, 0.42, 0.32);

	let comp = lighting * max(vec3(0.05), in.color);

	// smoothstep(vec3(-0.1), vec3(1.5), comp)

	return vec4(comp, 1.0);
}
