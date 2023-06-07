use crate::mesh::Mesh;
use rand::{thread_rng, Rng};
use wgpu::util::DeviceExt;

pub struct Particles {
	list: Vec<Particle>,
	buffer: wgpu::Buffer,
	mesh: Mesh,
}

impl Particles {
	pub fn new(device: &wgpu::Device) -> Self {
		let mesh = Mesh::load("sphere.obj", &device).unwrap();

		let mut rng = thread_rng();
		let mut list = Vec::new();

		for _ in 0..500 {
			let x = rng.sample::<f32, _>(rand_distr::StandardNormal) * 10.0;
			let y = rng.sample::<f32, _>(rand_distr::StandardNormal) * 5.0;
			let z = rng.sample::<f32, _>(rand_distr::StandardNormal) * 5.0;
			let position = cgmath::Vector3 { x, y, z };
			let scale = (rng.sample::<f32, _>(rand_distr::StandardNormal) * 0.1).exp() * 1.0;
			let color = [rng.gen(), rng.gen(), rng.gen()];
			list.push(Particle {
				position,
				scale,
				color,
			});
		}

		let instance_data = list.iter().map(Particle::to_raw).collect::<Vec<_>>();
		let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Particle Buffer"),
			contents: bytemuck::cast_slice(&instance_data),
			usage: wgpu::BufferUsages::VERTEX,
		});

		Self { list, buffer, mesh }
	}

	pub fn draw<'a>(
		&'a self,
		render_pass: &mut wgpu::RenderPass<'a>,
		render_pipeline: &'a wgpu::RenderPipeline,
		global_bind_group: &'a wgpu::BindGroup,
	) {
		render_pass.set_vertex_buffer(1, self.buffer.slice(..));
		render_pass.set_pipeline(render_pipeline);
		self.mesh
			.draw_instanced(render_pass, 0..self.list.len() as u32, &global_bind_group);
	}
}

pub struct Particle {
	position: cgmath::Vector3<f32>,
	scale: f32,
	color: [f32; 3],
}

impl Particle {
	pub fn to_raw(&self) -> ParticleRaw {
		ParticleRaw {
			model: (cgmath::Matrix4::from_translation(self.position)
				* cgmath::Matrix4::from_scale(self.scale))
			.into(),
			color: self.color,
		}
	}
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleRaw {
	model: [[f32; 4]; 4],
	color: [f32; 3],
}

impl ParticleRaw {
	pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<ParticleRaw>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Instance,
		attributes: &wgpu::vertex_attr_array![5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x3],
	};
}
