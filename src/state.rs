use crate::camera::Camera;
use crate::mesh::Vertex;
use crate::particle::{ParticleRaw, Particles};
use crate::texture::Texture;
use std::iter;
use std::time::Instant;
use winit::window::Window;

pub struct State {
	window: Window,
	surface: wgpu::Surface,
	smaa_target: smaa::SmaaTarget,
	depth_texture: Texture,
	device: wgpu::Device,
	queue: wgpu::Queue,
	config: wgpu::SurfaceConfiguration,
	size: winit::dpi::PhysicalSize<u32>,
	global_bind_group: wgpu::BindGroup,
	render_pipeline: wgpu::RenderPipeline,
	pub camera: Camera,
	particles: Particles,
	timer: Instant,
}

impl State {
	pub async fn new(window: Window) -> Self {
		let size = window.inner_size();

		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			// backends: wgpu::Backends::all(),
			// backends: wgpu::Backends::DX12,
			backends: wgpu::Backends::VULKAN,
			dx12_shader_compiler: wgpu::Dx12Compiler::default(),
		});

		let surface = unsafe { instance.create_surface(&window) }.unwrap();

		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				// power_preference: wgpu::PowerPreference::default(),
				power_preference: wgpu::PowerPreference::HighPerformance,
				compatible_surface: Some(&surface),
				force_fallback_adapter: false,
			})
			.await
			.unwrap();

		// println!("Selected adapter: {:?}", adapter.get_info());

		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					features: wgpu::Features::empty(),
					limits: wgpu::Limits::default(),
				},
				None,
			)
			.await
			.unwrap();

		// let surface_caps = surface.get_capabilities(&adapter);
		// dbg!(surface.get_capabilities(&adapter));

		// for f in surface.get_capabilities(&adapter).formats.iter() {
		// 	dbg!(f);
		// 	let srgb = f.is_srgb();
		// 	dbg!(srgb);
		// }

		// let surface_format = wgpu::TextureFormat::Rgba8UnormSrgb;
		// let surface_format = wgpu::TextureFormat::Rgba8Unorm;
		let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
		// let surface_format = surface.get_capabilities(&adapter).formats[0];

		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width: size.width,
			height: size.height,
			// Fifo is a strange way to spell vsync
			// present_mode: wgpu::PresentMode::Fifo,
			present_mode: wgpu::PresentMode::Mailbox,
			// present_mode: wgpu::PresentMode::Immediate,
			alpha_mode: wgpu::CompositeAlphaMode::Opaque,
			view_formats: vec![],
		};

		surface.configure(&device, &config);

		let smaa_target = smaa::SmaaTarget::new(
			&device,
			&queue,
			window.inner_size().width,
			window.inner_size().height,
			surface_format,
			smaa::SmaaMode::Smaa1X,
		);

		let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("shader.wgsl"),
			source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
		});

		// setup
		let camera = Camera::new(&device, &config);
		let particles = Particles::new(&device);

		// pipeline
		let global_bind_group_layout =
			device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &[wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}],
				label: Some("global_bind_group_layout"),
			});

		let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &global_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: camera.buffer.as_entire_binding(),
			}],
			label: Some("global_bind_group"),
		});

		let render_pipeline_layout =
			device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("Render Pipeline Layout"),
				bind_group_layouts: &[&global_bind_group_layout],
				push_constant_ranges: &[],
			});

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Render Pipeline"),
			layout: Some(&render_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: &[Vertex::LAYOUT, ParticleRaw::LAYOUT],
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: "fs_main",
				targets: &[Some(wgpu::ColorTargetState {
					format: config.format,
					blend: Some(wgpu::BlendState {
						color: wgpu::BlendComponent::REPLACE,
						alpha: wgpu::BlendComponent::REPLACE,
					}),
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: Some(wgpu::Face::Back),
				polygon_mode: wgpu::PolygonMode::Fill,
				unclipped_depth: false,
				conservative: false,
			},
			depth_stencil: Some(wgpu::DepthStencilState {
				format: Texture::DEPTH_FORMAT,
				depth_write_enabled: true,
				depth_compare: wgpu::CompareFunction::Less,
				stencil: wgpu::StencilState::default(),
				bias: wgpu::DepthBiasState::default(),
			}),
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			multiview: None,
		});

		Self {
			window,
			surface,
			smaa_target,
			depth_texture,
			device,
			queue,
			config,
			size,
			global_bind_group,
			render_pipeline,
			camera,
			particles,
			timer: Instant::now(),
		}
	}

	pub fn window(&self) -> &Window {
		&self.window
	}

	pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
		if new_size.width > 0 && new_size.height > 0 {
			self.camera
				.set_aspect(new_size.width as f32 / new_size.height as f32);
			self.size = new_size;
			self.config.width = new_size.width;
			self.config.height = new_size.height;
			self.surface.configure(&self.device, &self.config);
			self.depth_texture =
				Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
			self.smaa_target
				.resize(&self.device, new_size.width, new_size.height);
		}
	}

	pub fn update(&mut self) {
		self.particles.update(&self.queue);
		self.camera.update(&self.queue);
	}

	pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
		// println!(
		// 	"{:.1}",
		// 	1_000_000.0 / self.timer.elapsed().as_micros() as f64
		// );

		self.timer = Instant::now();

		let output = self.surface.get_current_texture()?;
		let view = output
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		let smaa_frame = self
			.smaa_target
			.start_frame(&self.device, &self.queue, &view);

		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("Render Encoder"),
			});

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &smaa_frame,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color {
							// these are linear
							r: 0.006,
							g: 0.02,
							b: 0.05,
							a: 1.0,
						}),
						store: true,
					},
				})],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &self.depth_texture.view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: true,
					}),
					stencil_ops: None,
				}),
			});

			self.particles.draw(
				&mut render_pass,
				&self.render_pipeline,
				&self.global_bind_group,
			);
		}

		self.queue.submit(iter::once(encoder.finish()));
		smaa_frame.resolve();
		output.present();

		Ok(())
	}
}
