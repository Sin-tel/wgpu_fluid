use wgpu::util::DeviceExt;

use crate::mesh::{Mesh, MeshVertex};

pub fn load_model(file_name: &str, device: &wgpu::Device) -> anyhow::Result<Mesh> {
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
