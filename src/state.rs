use crate::camera::Camera;
use crate::instance::{Instance, InstanceRaw};
use crate::mesh::{load_mesh, DrawMesh, Mesh, MeshVertex};
use crate::texture::Texture;
use rand::{thread_rng, Rng};
use std::iter;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{event::*, window::Window};

pub struct State {
	surface: wgpu::Surface,
	device: wgpu::Device,
	queue: wgpu::Queue,
	config: wgpu::SurfaceConfiguration,
	size: winit::dpi::PhysicalSize<u32>,
	render_pipeline: wgpu::RenderPipeline,
	sphere_mesh: Mesh,
	camera: Camera,
	instances: Vec<Instance>,
	#[allow(dead_code)]
	instance_buffer: wgpu::Buffer,
	depth_texture: Texture,
	window: Window,
	smaa_target: smaa::SmaaTarget,
	timer: Instant,
}

impl State {
	pub async fn new(window: Window) -> Self {
		let size = window.inner_size();

		log::warn!("WGPU setup");
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: wgpu::Backends::all(),
			dx12_shader_compiler: Default::default(),
		});

		let surface = unsafe { instance.create_surface(&window) }.unwrap();

		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				compatible_surface: Some(&surface),
				force_fallback_adapter: false,
			})
			.await
			.unwrap();
		log::warn!("device and queue");
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					features: wgpu::Features::empty(),
					limits: wgpu::Limits::default(),
				},
				// Some(&std::path::Path::new("trace")), // Trace path
				None, // Trace path
			)
			.await
			.unwrap();

		log::warn!("Surface");
		let surface_caps = surface.get_capabilities(&adapter);
		let surface_format = surface_caps
			.formats
			.iter()
			.copied()
			.find(|f| f.describe().srgb)
			.unwrap_or(surface_caps.formats[0]);
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width: size.width,
			height: size.height,
			// Fifo is a strange way to spell vsync
			present_mode: wgpu::PresentMode::Fifo,
			alpha_mode: surface_caps.alpha_modes[0],
			view_formats: vec![],
		};

		surface.configure(&device, &config);

		let camera = Camera::new(&device, &config);

		let mut rng = thread_rng();
		let mut instances = Vec::new();

		for _ in 0..500 {
			let x = rng.sample::<f32, _>(rand_distr::StandardNormal) * 10.0;
			let y = rng.sample::<f32, _>(rand_distr::StandardNormal) * 5.0;
			let z = rng.sample::<f32, _>(rand_distr::StandardNormal) * 5.0;
			let position = cgmath::Vector3 { x, y, z };
			let scale = (rng.sample::<f32, _>(rand_distr::StandardNormal) * 0.3).exp() * 2.0;
			let color = [rng.gen(), rng.gen(), rng.gen()];
			instances.push(Instance {
				position,
				scale,
				color,
			});
		}

		let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
		let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Instance Buffer"),
			contents: bytemuck::cast_slice(&instance_data),
			usage: wgpu::BufferUsages::VERTEX,
		});

		log::warn!("Load model");
		let sphere_mesh = load_mesh("sphere.obj", &device).unwrap();

		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("shader.wgsl"),
			source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
		});

		let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

		let render_pipeline_layout =
			device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("Render Pipeline Layout"),
				bind_group_layouts: &[&camera.layout],
				push_constant_ranges: &[],
			});

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Render Pipeline"),
			layout: Some(&render_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: &[MeshVertex::desc(), InstanceRaw::desc()],
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
				// Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
				// or Features::POLYGON_MODE_POINT
				polygon_mode: wgpu::PolygonMode::Fill,
				// Requires Features::DEPTH_CLIP_CONTROL
				unclipped_depth: false,
				// Requires Features::CONSERVATIVE_RASTERIZATION
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

		let smaa_target = smaa::SmaaTarget::new(
			&device,
			&queue,
			window.inner_size().width,
			window.inner_size().height,
			surface_format,
			smaa::SmaaMode::Smaa1X,
		);

		Self {
			surface,
			device,
			queue,
			config,
			size,
			render_pipeline,
			sphere_mesh,
			camera,
			instances,
			instance_buffer,
			depth_texture,
			window,
			smaa_target,
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
	pub fn input(&mut self, event: &WindowEvent) -> bool {
		self.camera.process_events(event)
	}

	pub fn update(&mut self) {
		self.camera.update(&self.queue);
	}

	pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
		// println!(
		//     "{:.1}",
		//     1_000_000.0 / self.timer.elapsed().as_micros() as f64
		// );

		self.timer = Instant::now();

		let output = self.surface.get_current_texture()?;
		let view = output
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		let smaa_frame = self
			.smaa_target
			.start_frame(&self.device, &self.queue, &view);

		// let smaa_frame = view;

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
							r: 0.1,
							g: 0.2,
							b: 0.3,
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

			render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
			render_pass.set_pipeline(&self.render_pipeline);
			render_pass.draw_mesh_instanced(
				&self.sphere_mesh,
				0..self.instances.len() as u32,
				&self.camera.bind_group,
			);
		}

		self.queue.submit(iter::once(encoder.finish()));

		smaa_frame.resolve();

		output.present();

		Ok(())
	}
}
