use cgmath::prelude::*;
use wgpu::util::DeviceExt;
use winit::event::*;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraMatrix([[f32; 4]; 4]);

pub struct Camera {
	settings: Settings,
	buffer: wgpu::Buffer,
	pub layout: wgpu::BindGroupLayout,
	pub bind_group: wgpu::BindGroup,
	pub controller: CameraController,
}

struct Settings {
	eye: cgmath::Point3<f32>,
	target: cgmath::Point3<f32>,
	up: cgmath::Vector3<f32>,
	aspect: f32,
	fovy: f32,
	znear: f32,
	zfar: f32,
}

impl Settings {
	fn get_matrix(&self) -> CameraMatrix {
		let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
		let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
		CameraMatrix((OPENGL_TO_WGPU_MATRIX * proj * view).into())
	}
}

impl Camera {
	pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
		let settings = Settings {
			eye: (0.0, 20.0, 100.0).into(),
			target: (0.0, 0.0, 0.0).into(),
			up: cgmath::Vector3::unit_y(),
			aspect: config.width as f32 / config.height as f32,
			fovy: 45.0,
			znear: 0.1,
			zfar: 500.0,
		};
		let controller = CameraController::new(1.0);

		let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera Buffer"),
			contents: bytemuck::cast_slice(&[settings.get_matrix()]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});
		let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
			label: Some("camera_bind_group_layout"),
		});

		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: buffer.as_entire_binding(),
			}],
			label: Some("camera_bind_group"),
		});
		Camera {
			settings,
			buffer,
			layout,
			bind_group,
			controller,
		}
	}

	pub fn set_aspect(&mut self, aspect: f32) {
		self.settings.aspect = aspect;
	}

	pub fn process_events(&mut self, event: &WindowEvent) -> bool {
		match event {
			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state,
						virtual_keycode: Some(keycode),
						..
					},
				..
			} => {
				let is_pressed = *state == ElementState::Pressed;
				match keycode {
					VirtualKeyCode::Space => {
						self.controller.is_up_pressed = is_pressed;
						true
					}
					VirtualKeyCode::LShift => {
						self.controller.is_down_pressed = is_pressed;
						true
					}
					VirtualKeyCode::W | VirtualKeyCode::Up => {
						self.controller.is_forward_pressed = is_pressed;
						true
					}
					VirtualKeyCode::A | VirtualKeyCode::Left => {
						self.controller.is_left_pressed = is_pressed;
						true
					}
					VirtualKeyCode::S | VirtualKeyCode::Down => {
						self.controller.is_backward_pressed = is_pressed;
						true
					}
					VirtualKeyCode::D | VirtualKeyCode::Right => {
						self.controller.is_right_pressed = is_pressed;
						true
					}
					_ => false,
				}
			}
			_ => false,
		}
	}

	pub fn update(&mut self, queue: &wgpu::Queue) {
		let forward = self.settings.target - self.settings.eye;
		let forward_norm = forward.normalize();
		let forward_mag = forward.magnitude();

		// Prevents glitching when camera gets too close to the
		// center of the scene.
		if self.controller.is_forward_pressed && forward_mag > self.controller.speed {
			self.settings.eye += forward_norm * self.controller.speed;
		}
		if self.controller.is_backward_pressed {
			self.settings.eye -= forward_norm * self.controller.speed;
		}

		let right = forward_norm.cross(self.settings.up);

		// Redo radius calc in case the up/ down is pressed.
		let forward = self.settings.target - self.settings.eye;
		let forward_mag = forward.magnitude();

		if self.controller.is_right_pressed {
			// Rescale the distance between the target and eye so
			// that it doesn't change. The eye therefore still
			// lies on the circle made by the target and eye.
			self.settings.eye = self.settings.target
				- (forward + right * self.controller.speed).normalize() * forward_mag;
		}
		if self.controller.is_left_pressed {
			self.settings.eye = self.settings.target
				- (forward - right * self.controller.speed).normalize() * forward_mag;
		}

		queue.write_buffer(
			&self.buffer,
			0,
			bytemuck::cast_slice(&[self.settings.get_matrix()]),
		);
	}
}

pub struct CameraController {
	speed: f32,
	is_up_pressed: bool,
	is_down_pressed: bool,
	is_forward_pressed: bool,
	is_backward_pressed: bool,
	is_left_pressed: bool,
	is_right_pressed: bool,
}

impl CameraController {
	pub fn new(speed: f32) -> Self {
		Self {
			speed,
			is_up_pressed: false,
			is_down_pressed: false,
			is_forward_pressed: false,
			is_backward_pressed: false,
			is_left_pressed: false,
			is_right_pressed: false,
		}
	}
}
