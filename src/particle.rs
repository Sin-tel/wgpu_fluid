use crate::mesh::Mesh;
use cgmath::prelude::*;
use cgmath::{Matrix4, Vector3};
use rand::{thread_rng, Rng};

const MAX_PARTICLES: usize = 1_000;

const DT: f32 = 0.5;

const H: f32 = 16.0;

const REST_DENSITY: f32 = 0.001;
const GAS_CONST: f32 = 50.0;

use std::f32::consts::PI;

// (h^2 - r^2)^3
fn w_poly6(r_squared: f32) -> f32 {
	(315.0 / (64.0 * PI * H.powi(9))) * (H.powi(2) - r_squared).powi(3)
}

// fn w_poly6_grad(r_squared: f32) -> f32 {}

fn w_spiky_grad(r_squared: f32) -> f32 {
	(45.0 / (PI * H.powi(6))) * (H - r_squared.sqrt()).powi(2)
}

fn w_visc(r_squared: f32) -> f32 {
	(45.0 / (PI * H.powi(6))) * (H - r_squared.sqrt())
}

// TODO: try struct of arrays perf
pub struct Particles {
	list: Vec<Particle>,
	buffer: wgpu::Buffer,
	mesh: Mesh,
}

impl Particles {
	pub fn new(device: &wgpu::Device) -> Self {
		let mesh = Mesh::load("sphere.obj", device).unwrap();

		let mut rng = thread_rng();
		let mut list = Vec::new();

		for _ in 0..MAX_PARTICLES {
			let x = rng.sample::<f32, _>(rand_distr::StandardNormal) * 20.0;
			let y = rng.sample::<f32, _>(rand_distr::StandardNormal) * 10.0;
			let z = rng.sample::<f32, _>(rand_distr::StandardNormal) * 10.0;
			let position = Vector3 { x, y, z };
			// let radius = (rng.sample::<f32, _>(rand_distr::StandardNormal) * 0.1).exp() * 0.5;
			let radius = H / 3.0;
			let color = [rng.gen(), 0.8, rng.gen()];
			list.push(Particle {
				position,
				velocity: Vector3::zero(),
				force: Vector3::zero(),
				radius,
				color,
				mass: 1.0,
				pressure: 0.0,
				density: 0.0,
			});
		}

		let buffer = device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("Particle Buffer"),
			size: (std::mem::size_of::<ParticleRaw>() * MAX_PARTICLES) as u64,
			usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});

		Self { list, buffer, mesh }
	}

	pub fn update(&mut self, queue: &wgpu::Queue) {
		self.update_pressure();
		self.update_forces();
		self.integrate();
		self.update_buffer(queue);
	}

	pub fn update_pressure(&mut self) {
		let n = self.list.len();

		for i in 0..n {
			let p_i = self.list[i];

			let mut density = 0.0;

			for j in 0..n {
				// todo optimize symmetry and own mass
				let p_j = self.list[j];

				let r_ij = p_i.position - p_j.position;
				let r_sq = r_ij.magnitude2();

				if r_sq < H.powi(2) {
					density += p_j.mass * w_poly6(r_sq);
				}
			}
			self.list[i].density = density;
			self.list[i].pressure = GAS_CONST * (density - REST_DENSITY)
		}
	}

	pub fn update_forces(&mut self) {
		let mut rng = thread_rng();

		let n = self.list.len();

		for i in 0..n {
			let p_i = self.list[i];
			let mut f_press = Vector3::zero();
			let mut f_visc = Vector3::zero();
			for j in 0..n {
				if i == j {
					continue;
					// break;
				}
				let p_j = self.list[j];

				let r_ij = p_j.position - p_i.position;
				let r_sq = r_ij.magnitude2();

				if r_sq < H.powi(2) {
					f_press += -r_ij.normalize()
						* p_j.mass * (p_i.pressure + p_j.pressure)
						* w_spiky_grad(r_sq) / (2.0 * p_j.density);

					f_visc += 0.01 * p_j.mass * (p_j.velocity - p_i.velocity) * w_visc(r_sq)
						/ (p_j.density);
				}
			}

			let x = rng.sample::<f32, _>(rand_distr::StandardNormal);
			let y = rng.sample::<f32, _>(rand_distr::StandardNormal);
			let z = rng.sample::<f32, _>(rand_distr::StandardNormal);
			let f_brownian = 0.00001 * Vector3 { x, y, z };

			let f_well = -0.000001 * p_i.position;

			let f_friction = -0.000001 * p_i.velocity;

			self.list[i].force += f_press + f_visc + f_well + f_friction + f_brownian;
		}
	}
	pub fn integrate(&mut self) {
		self.list.iter_mut().for_each(|p| p.integrate());
	}

	fn update_buffer(&mut self, queue: &wgpu::Queue) {
		let instance_data = self.list.iter().map(Particle::to_raw).collect::<Vec<_>>();
		queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&instance_data));
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
			.draw_instanced(render_pass, 0..self.list.len() as u32, global_bind_group);
	}
}

#[derive(Debug, Copy, Clone)]
pub struct Particle {
	position: Vector3<f32>,
	velocity: Vector3<f32>,
	force: Vector3<f32>,
	radius: f32,
	mass: f32,
	density: f32,
	pressure: f32,
	color: [f32; 3],
}

impl Particle {
	pub fn integrate(&mut self) {
		self.velocity += (self.force / self.density) * DT;
		self.position += self.velocity * DT;
		self.force = Vector3::zero();
	}

	pub fn to_raw(&self) -> ParticleRaw {
		ParticleRaw {
			model: (Matrix4::from_translation(self.position) * Matrix4::from_scale(self.radius))
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
