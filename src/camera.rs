use cgmath::prelude::*;
// use wgpu::util::DeviceExt;

const YAW_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Matrix4([[f32; 4]; 4]);

pub struct Camera {
	// eye: cgmath::Point3<f32>,
	yaw: f32,
	pitch: f32,
	dist: f32,
	target: cgmath::Point3<f32>,
	up: cgmath::Vector3<f32>,
	aspect: f32,
	fovy: f32,
	znear: f32,
	zfar: f32,
	pub buffer: wgpu::Buffer,
}

impl Camera {
	pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("Camera Buffer"),
			size: std::mem::size_of::<Matrix4>() as u64,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});

		Camera {
			yaw: 0.0,
			pitch: 0.0,
			dist: 300.0,
			target: (0.0, 0.0, 0.0).into(),
			up: cgmath::Vector3::unit_y(),
			aspect: config.width as f32 / config.height as f32,
			fovy: 45.0,
			znear: 0.1,
			zfar: 2000.0,
			buffer,
		}
	}

	fn eye(&self) -> cgmath::Point3<f32> {
		let target = self.target;
		let px = target.x + self.dist * self.yaw.sin() * self.pitch.cos();
		let py = target.y + self.dist * self.pitch.sin();
		let pz = target.z + self.dist * self.yaw.cos() * self.pitch.cos();
		cgmath::Point3::new(px, py, pz)
	}

	fn get_matrix(&self) -> Matrix4 {
		let view = cgmath::Matrix4::look_at_rh(self.eye(), self.target, self.up);
		let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
		Matrix4((OPENGL_TO_WGPU_MATRIX * proj * view).into())
	}

	pub fn set_aspect(&mut self, aspect: f32) {
		self.aspect = aspect;
	}

	pub fn pan(&mut self, delta: cgmath::Vector2<f32>) {
		let eye = self.eye();
		let dir = (self.target - eye).normalize();
		let tangent = self.up.cross(dir).normalize();
		let bitangent = dir.cross(tangent);
		self.target = self.target
			+ tangent * (delta.x * self.dist) * 0.001
			+ bitangent * (delta.y * self.dist) * 0.001;
	}

	pub fn arcball(&mut self, delta: cgmath::Vector2<f32>) {
		self.yaw -= delta.x * 0.005;
		self.pitch += delta.y * 0.005;

		self.pitch = self.pitch.clamp(-YAW_LIMIT, YAW_LIMIT);
	}

	pub fn zoom(&mut self, delta: f32) {
		self.dist *= 1.002_f32.powf(delta);
	}

	pub fn look_at_origin(&mut self) {
		self.target = (0.0, 0.0, 0.0).into();
	}

	pub fn update(&mut self, queue: &wgpu::Queue) {
		queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.get_matrix()]));
	}
}
