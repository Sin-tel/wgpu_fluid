pub struct Instance {
	pub position: cgmath::Vector3<f32>,
	pub scale: f32,
	pub color: [f32; 3],
}

impl Instance {
	pub fn to_raw(&self) -> InstanceRaw {
		InstanceRaw {
			model: (cgmath::Matrix4::from_translation(self.position)
				* cgmath::Matrix4::from_scale(self.scale))
			.into(),
			color: self.color,
		}
	}
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
	#[allow(dead_code)]
	model: [[f32; 4]; 4],
	color: [f32; 3],
}

impl InstanceRaw {
	const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x3];
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		use std::mem;
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &Self::ATTRIBS,
		}
	}
}
