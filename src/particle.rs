use crate::mesh::Mesh;
use cgmath::prelude::*;
use cgmath::{Matrix4, Vector3};
use nalgebra::{DMatrix, Dyn, OMatrix, U3};
use rand::{thread_rng, Rng};
use std::iter::zip;

use std::f32::consts::PI;
const TWO_PI: f32 = std::f32::consts::TAU;

const MAX_PARTICLES: usize = 5_000;
const DT: f32 = 0.5;
const REST_DENSITY: f32 = 1.2;
const GAS_CONST: f32 = 0.0003;
const FRICTION: f32 = 0.00001;
const VISCOSITY: f32 = 0.0001;
// const FRICTION: f32 = 0.01;
// const VISCOSITY: f32 = 0.;
const DIV_LENGTH: f32 = 0.7;

type MatrixNx3 = OMatrix<f32, Dyn, U3>;

fn random_axis() -> Vector3<f32> {
	let mut rng = thread_rng();
	let theta = rng.gen::<f32>() * TWO_PI;

	let phi = rng.gen_range(-1.0f32..1.0).acos();

	let x = theta.cos() * phi.sin();
	let y = theta.sin() * phi.sin();
	let z = phi.cos();
	(x, y, z).into()
}

// we take density = 1.0
fn mass_to_radius(m: f32) -> f32 {
	(3.0 * m / (4.0 * PI)).powf(1.0 / 3.0)
}

fn w_poly6(r_squared: f32, h: f32) -> f32 {
	(315.0 / (64.0 * PI * h.powi(9))) * (h.powi(2) - r_squared).powi(3)
}

// fn w_poly6_grad(r_squared: f32, h: f32) -> f32 {
// 	(945.0 / (32.0 * PI * h.powi(9))) * r_squared.sqrt() * (h.powi(2) - r_squared).powi(2)
// }

fn w_spiky_grad(r_squared: f32, h: f32) -> f32 {
	(45.0 / (PI * h.powi(6))) * (h - r_squared.sqrt()).powi(2)
}

fn w_visc(r_squared: f32, h: f32) -> f32 {
	(45.0 / (PI * h.powi(6))) * (h - r_squared.sqrt())
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

		// let mut rng = thread_rng();
		let mut list = Vec::with_capacity(MAX_PARTICLES);

		for _ in 0..1 {
			// let x = rng.sample::<f32, _>(rand_distr::StandardNormal) * 20.0;
			// let y = rng.sample::<f32, _>(rand_distr::StandardNormal) * 10.0;
			// let z = rng.sample::<f32, _>(rand_distr::StandardNormal) * 10.0;
			// let position = Vector3 { x, y, z };
			// let mass: f32 = (rng.sample::<f32, _>(rand_distr::StandardNormal) * 0.5).exp() * 500.0;

			let position = Vector3::zero();
			let mass = 1.0;
			list.push(Particle::new(position, mass, [0.5, 0.5, 0.5]));
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
			self.divide();
			self.update_pressure();
			self.update_forces();
			self.list.iter_mut().for_each(|p| p.integrate());
		}
		self.update_buffer(queue);
	}

	pub fn divide(&mut self) {
		let mut rng = thread_rng();

		self.list
			.iter_mut()
			.for_each(|p| p.age += 0.05 * rng.gen::<f32>());

		let n = self.list.len();
		let index = rng.gen_range(0..n);

		self.list[index].age += 0.05;

		if (self.list[index].age > 1000.0 && n < 200) || n == 1 {
			println!("{:?}", n);
			let p = self.list[index].position;
			let axis = 0.5 * DIV_LENGTH * self.list[index].radius * random_axis();

			let mass_div: f32 = rng.gen_range(0.4..0.6);

			let mut newcol = self.list[index].color;

			newcol[0] += rng.gen_range(-0.1..0.1);
			newcol[1] += rng.gen_range(-0.1..0.1);
			newcol[2] += rng.gen_range(-0.1..0.1);

			let m = self.list[index].mass;

			self.list[index] = Particle::new(p + axis, mass_div * m, newcol);
			self.list
				.push(Particle::new(p - axis, (1.0 - mass_div) * m, newcol));
		}
	}

	// pub fn relax(&mut self) {
	// 	self.update_pressure();
	// 	self.update_forces();
	// 	self.list.iter_mut().for_each(|p| p.relax());
	// }

	pub fn update_pressure(&mut self) {
		let n = self.list.len();

		for i in 0..n {
			let p_i = self.list[i];

			let mut density = 0.0;

			for j in 0..n {
				// todo optimize symmetry and own mass
				let p_j = self.list[j];

				let r_sum = p_i.radius + p_j.radius;

				let r_ij = p_i.position - p_j.position;
				let r_sq = r_ij.magnitude2();

				if r_sq < r_sum.powi(2) {
					density += p_j.mass * w_poly6(r_sq, r_sum);
				}
			}
			self.list[i].density = density;
			// self.list[i].pressure = GAS_CONST * (density - REST_DENSITY).powi(3)
			self.list[i].pressure = GAS_CONST * (density - REST_DENSITY)
		}
	}

	pub fn update_forces(&mut self) {
		let n = self.list.len();
		// let mut rng = thread_rng();

		// friction matrix
		let mut gamma = DMatrix::from_diagonal_element(n, n, FRICTION);

		for i in 0..n {
			let p_i = self.list[i];
			let mut f_press = Vector3::zero();
			// let mut f_hooke = Vector3::zero();
			let mut visc_sum = 0.0;
			for j in 0..n {
				if i == j {
					continue;
					// break;
				}
				let p_j = self.list[j];

				let r_sum = p_i.radius + p_j.radius;

				let r_ij = p_j.position - p_i.position;
				let r_sq = r_ij.magnitude2();

				if r_sq < r_sum.powi(2) {
					f_press += -r_ij.normalize()
						* p_j.mass * (p_i.pressure + p_j.pressure)
						* w_spiky_grad(r_sq, r_sum)
						/ (2.0 * p_j.density);

					let visc = VISCOSITY * p_j.mass * w_visc(r_sq, r_sum) / p_j.density;
					visc_sum += visc;
					gamma[(i, j)] = -visc;

					// f_hooke += 0.000001
					// 	* p_j.mass * r_ij.normalize()
					// 	* (r_sq.sqrt() - r_sum) * w_poly6(r_sq, r_sum)
					// 	/ p_j.density;
				}
			}
			gamma[(i, i)] += visc_sum;

			let p = p_i.position;
			let f_well = -0.00001
				* Vector3 {
					x: p.x * 0.3,
					y: p.y,
					z: p.z,
				};

			let f_polar = 0.000001 * p_i.polarization;

			self.list[i].force += f_press + f_well + f_polar;
		}

		// build matrix of forces

		// TODO: do some iterator magic here instead?
		let mut res = Vec::with_capacity(3 * n);
		for p in &self.list {
			res.push(p.force.x);
			res.push(p.force.y);
			res.push(p.force.z);
		}

		let mut b = MatrixNx3::from_row_slice(&res);

		gamma.lu().solve_mut(&mut b);

		for (p, v) in zip(self.list.iter_mut(), b.row_iter()) {
			// let x = rng.sample::<f32, _>(rand_distr::StandardNormal);
			// let y = rng.sample::<f32, _>(rand_distr::StandardNormal);
			// let z = rng.sample::<f32, _>(rand_distr::StandardNormal);
			// let f_brownian = 0.001 * Vector3 { x, y, z };

			p.force = Vector3::new(v[0], v[1], v[2]); // + f_brownian;
		}
	}

	fn update_buffer(&mut self, queue: &wgpu::Queue) {
		let instance_data = self.list.iter().map(Particle::as_raw).collect::<Vec<_>>();
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
	force: Vector3<f32>,
	polarization: Vector3<f32>,
	radius: f32,
	mass: f32,
	density: f32,
	pressure: f32,
	age: f32,
	color: [f32; 3],
}

impl Particle {
	pub fn new(position: Vector3<f32>, mass: f32, color: [f32; 3]) -> Self {
		// let radius = (rng.sample::<f32, _>(rand_distr::StandardNormal) * 0.1).exp() * 0.5;
		// let radius = H * 0.5;

		let radius = mass_to_radius(mass);

		let polarization = random_axis();

		// println!("{:?}", radius);

		// let mut rng = thread_rng();
		// let color = [rng.gen(), 0.8, rng.gen()];

		Particle {
			position,
			force: Vector3::zero(),
			polarization,
			radius,
			mass,
			color,
			pressure: 0.0,
			density: 0.0,
			age: 0.0,
		}
	}

	pub fn integrate(&mut self) {
		// self.velocity += (self.force / self.density) * DT;
		// self.position += self.velocity * DT;
		self.position += DT * self.force;
		self.force = Vector3::zero();
	}

	// pub fn relax(&mut self) {
	// 	// self.position += (self.force / self.density) * 0.5;
	// 	self.position += self.force * 100000.0;
	// 	self.force = Vector3::zero();
	// }

	pub fn as_raw(&self) -> ParticleRaw {
		ParticleRaw {
			model: (Matrix4::from_translation(self.position)
				* Matrix4::from_scale(self.radius * 1.0))
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
