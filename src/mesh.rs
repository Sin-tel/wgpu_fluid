use std::ops::Range;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
	pub position: [f32; 3],
	pub normal: [f32; 3],
}

impl Vertex {
	pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
	};
}

#[derive(Debug)]
pub struct Mesh {
	vertex_buffer: wgpu::Buffer,
	index_buffer: wgpu::Buffer,
	num_elements: u32,
}

impl Mesh {
	pub fn load(file_name: &str, device: &wgpu::Device) -> anyhow::Result<Self> {
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("res")
			.join(file_name);

		let (mut models, _) = tobj::load_obj(
			path,
			&tobj::LoadOptions {
				triangulate: true,
				single_index: true,
				..Default::default()
			},
		)?;

		// we only need one mesh
		assert!(models.len() == 1);

		let mesh = models.remove(0).mesh;

		let vertices = (0..mesh.positions.len() / 3)
			.map(|i| Vertex {
				position: [
					mesh.positions[i * 3],
					mesh.positions[i * 3 + 1],
					mesh.positions[i * 3 + 2],
				],
				normal: [
					mesh.normals[i * 3],
					mesh.normals[i * 3 + 1],
					mesh.normals[i * 3 + 2],
				],
			})
			.collect::<Vec<_>>();

		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{file_name} Vertex Buffer")),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{file_name} Index Buffer")),
			contents: bytemuck::cast_slice(&mesh.indices),
			usage: wgpu::BufferUsages::INDEX,
		});

		Ok(Mesh {
			vertex_buffer,
			index_buffer,
			num_elements: u32::try_from(mesh.indices.len())?,
		})
	}

	// pub fn draw<'a>(
	// 	&'a self,
	// 	render_pass: &mut wgpu::RenderPass<'a>,
	// 	global_bind_group: &'a wgpu::BindGroup,
	// ) {
	// 	self.draw_instanced(render_pass, 0..1, global_bind_group);
	// }

	pub fn draw_instanced<'a>(
		&'a self,
		render_pass: &mut wgpu::RenderPass<'a>,
		instances: Range<u32>,
		global_bind_group: &'a wgpu::BindGroup,
	) {
		render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
		render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		render_pass.set_bind_group(0, global_bind_group, &[]);
		render_pass.draw_indexed(0..self.num_elements, 0, instances);
	}
}
