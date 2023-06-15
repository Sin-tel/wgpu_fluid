use crate::mesh::Mesh;
use cgmath::prelude::*;
use cgmath::{Matrix4, Vector3};
use rand::{thread_rng, Rng};

const MAX_PARTICLES: usize = 600;

const DT: f32 = 0.5;

const H: f32 = 16.0;

// const REST_DENSITY: f32 = 0.0;
const REST_DENSITY: f32 = 0.003;
const GAS_CONST: f32 = 2.0;

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
		let mesh = Mesh::load("arrow.obj", device).unwrap();

		let mut rng = thread_rng();
		let mut list = Vec::new();

		for _ in 0..MAX_PARTICLES {
			let x = rng.sample::<f32, _>(rand_distr::StandardNormal) * 20.0;
			let y = rng.sample::<f32, _>(rand_distr::StandardNormal) * 20.0;
			let z = rng.sample::<f32, _>(rand_distr::StandardNormal) * 20.0;
			let position = Vector3 { x, y, z };
			// let radius = (rng.sample::<f32, _>(rand_distr::StandardNormal) * 0.1).exp() * 0.5;
			let radius = H / 10.0;
			let color = [rng.gen(), 0.8, rng.gen()];

			let x = rng.sample::<f32, _>(rand_distr::StandardNormal);
			let y = rng.sample::<f32, _>(rand_distr::StandardNormal);
			let z = rng.sample::<f32, _>(rand_distr::StandardNormal);
			let normal = Vector3 { x, y, z }.normalize();
			// let normal = position.normalize();
			list.push(Particle {
				position,
				velocity: Vector3::zero(),
				force: Vector3::zero(),
				torque: Vector3::zero(),
				radius,
				color,
				normal,
				mass: 10.0,
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
		for _ in 0..10 {
			self.update_pressure();
			self.update_forces();
			self.list.iter_mut().for_each(|p| p.integrate());
		}
		self.update_buffer(queue);
	}

	pub fn relax(&mut self) {
		for _ in 0..60 {
			self.update_pressure();
			self.update_forces();
			self.list.iter_mut().for_each(|p| p.relax());
		}
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
		let n = self.list.len();

		for i in 0..n {
			let p_i = self.list[i];
			let mut f_press = Vector3::zero();
			let mut f_dipole = Vector3::zero();
			// let mut f_visc = Vector3::zero();
			let mut torque = Vector3::zero();
			for j in 0..n {
				if i == j {
					continue;
					// break;
				}
				let p_j = self.list[j];

				let r_ij = p_j.position - p_i.position;
				let r_sq = r_ij.magnitude2();

				if r_sq < H.powi(2) {
					let r_n = r_ij.normalize();

					let r_tan = r_n - (r_n.dot(p_i.normal)) * p_i.normal;

					f_press += -r_tan
						* p_j.mass * (p_i.pressure + p_j.pressure)
						* w_spiky_grad(r_sq) / (2.0 * p_j.density);

					// torque += -10.0
					// 	* w_poly6(r_sq) * (p_i.normal.cross(p_j.normal)
					// 	- 3.0 * (p_j.normal.dot(r_ij) * p_i.normal.cross(r_ij) / r_sq));

					torque += 2.0 * w_poly6(r_sq) * p_j.normal.cross(p_i.normal);

					// f_visc += 0.005 * p_j.mass * (p_j.velocity - p_i.velocity) * w_visc(r_sq)
					// 	/ (p_j.density);

					f_dipole += 0.3
						* w_poly6(r_sq) * (r_n * p_i.normal.dot(p_j.normal)
						+ p_i.normal * (r_n.dot(p_j.normal))
						+ p_j.normal * (r_n.dot(p_i.normal))
						- 5.0 * r_n * r_n.dot(p_i.normal) * r_n.dot(p_j.normal));
					// f_dipole +=
					// 	10.0 * w_visc(r_sq) * r_n * r_n.dot(p_i.normal) * r_n.dot(p_j.normal);
				}
			}

			let f_well = -0.000001 * p_i.position;

			let f_friction = -0.0001 * p_i.velocity;

			let f_normal = 0.00001 * p_i.normal;

			self.list[i].force += f_press + f_well /*+ f_dipole*/ + f_friction + f_normal;
			self.list[i].torque += torque;
		}
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
	torque: Vector3<f32>,
	radius: f32,
	mass: f32,
	density: f32,
	pressure: f32,
	normal: Vector3<f32>,
	color: [f32; 3],
}

impl Particle {
	pub fn integrate(&mut self) {
		self.velocity += (self.force / self.density) * DT;
		self.position += self.velocity * DT;

		// self.position += self.force * DT * 100.0;

		self.normal += self.normal.cross(self.torque) * DT * 100.0;
		self.normal = self.normal.normalize();
		self.force = Vector3::zero();
		self.torque = Vector3::zero();
	}

	pub fn relax(&mut self) {
		// self.velocity += (self.force / self.density) * DT;
		// self.position += self.velocity * DT;

		self.position += self.force * DT * 100.0;

		// self.normal += self.normal.cross(self.torque) * DT * 100.0;
		// self.normal = self.normal.normalize();
		self.force = Vector3::zero();
		self.torque = Vector3::zero();
	}

	pub fn to_raw(&self) -> ParticleRaw {
		ParticleRaw {
			model: (Matrix4::from_translation(self.position)
				* dir_to_mat4(self.normal)
				* Matrix4::from_scale(self.radius))
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

fn dir_to_mat4<S: cgmath::BaseFloat>(dir: Vector3<S>) -> Matrix4<S> {
	// let f = dir.normalize();
	// assume dir is normalized
	let f = dir;
	let s = f.cross(Vector3::unit_y()).normalize();
	let u = s.cross(f);

	#[cfg_attr(rustfmt, rustfmt_skip)]
    Matrix4::new(
        s.x.clone(), s.y.clone(),  s.z.clone(), S::zero(),
        f.x.clone(), f.y.clone(),  f.z.clone(), S::zero(),
        u.x.clone(), u.y.clone(),  u.z.clone(), S::zero(),
        S::zero(),   S::zero(),    S::zero(),   S::one(),
    )
}
