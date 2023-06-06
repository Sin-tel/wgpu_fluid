use std::ops::Range;
use wgpu::util::DeviceExt;

pub fn load_mesh(file_name: &str, device: &wgpu::Device) -> anyhow::Result<Mesh> {
	let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("res")
		.join(file_name);

	let (mut models, _) = tobj::load_obj(
		&path,
		&tobj::LoadOptions {
			triangulate: true,
			single_index: true,
			..Default::default()
		},
	)
	.expect("Failed to OBJ load file");

	// we only need one mesh
	assert!(models.len() == 1);

	let mesh = models.remove(0).mesh;

	let vertices = (0..mesh.positions.len() / 3)
		.map(|i| MeshVertex {
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
		label: Some(&format!("{:?} Vertex Buffer", file_name)),
		contents: bytemuck::cast_slice(&vertices),
		usage: wgpu::BufferUsages::VERTEX,
	});
	let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some(&format!("{:?} Index Buffer", file_name)),
		contents: bytemuck::cast_slice(&mesh.indices),
		usage: wgpu::BufferUsages::INDEX,
	});

	Ok(Mesh {
		name: file_name.to_string(),
		vertex_buffer,
		index_buffer,
		num_elements: mesh.indices.len() as u32,
	})
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
	pub position: [f32; 3],
	pub normal: [f32; 3],
}

impl MeshVertex {
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		use std::mem;
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &[
				wgpu::VertexAttribute {
					offset: 0,
					shader_location: 0,
					format: wgpu::VertexFormat::Float32x3,
				},
				wgpu::VertexAttribute {
					offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
					shader_location: 1,
					format: wgpu::VertexFormat::Float32x2,
				},
				wgpu::VertexAttribute {
					offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
					shader_location: 2,
					format: wgpu::VertexFormat::Float32x3,
				},
			],
		}
	}
}
#[derive(Debug)]
pub struct Mesh {
	pub name: String,
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub num_elements: u32,
}

pub trait DrawMesh<'a> {
	fn draw_mesh(&mut self, mesh: &'a Mesh, camera_bind_group: &'a wgpu::BindGroup);
	fn draw_mesh_instanced(
		&mut self,
		mesh: &'a Mesh,
		instances: Range<u32>,
		camera_bind_group: &'a wgpu::BindGroup,
	);
}

impl<'a, 'b> DrawMesh<'b> for wgpu::RenderPass<'a>
where
	'b: 'a,
{
	fn draw_mesh(&mut self, mesh: &'b Mesh, camera_bind_group: &'b wgpu::BindGroup) {
		self.draw_mesh_instanced(mesh, 0..1, camera_bind_group);
	}

	fn draw_mesh_instanced(
		&mut self,
		mesh: &'b Mesh,
		instances: Range<u32>,
		camera_bind_group: &'b wgpu::BindGroup,
	) {
		self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
		self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		self.set_bind_group(0, camera_bind_group, &[]);
		self.draw_indexed(0..mesh.num_elements, 0, instances);
	}
}
