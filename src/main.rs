use winit::{
	event::*,
	event_loop::{ControlFlow, EventLoop},
};

mod camera;
mod mesh;
mod particle;
mod state;
mod texture;

use state::State;

fn main() {
	pollster::block_on(run());
}

pub async fn run() {
	env_logger::init();

	let event_loop = EventLoop::new();
	let title = env!("CARGO_PKG_NAME");
	let window = winit::window::WindowBuilder::new()
		.with_title(title)
		.build(&event_loop)
		.unwrap();

	let mut state = State::new(window).await;

	let mut mouse_pos: cgmath::Point2<f32> = cgmath::Point2::new(0.0, 0.0);
	let mut mouse_down_left = false;
	let mut mouse_down_right = false;
	let mut mouse_down_middle = false;

	event_loop.run(move |event, _, control_flow| {
		*control_flow = ControlFlow::Poll;
		match event {
			Event::MainEventsCleared => state.window().request_redraw(),
			Event::WindowEvent {
				ref event,
				window_id,
			} if window_id == state.window().id() => match event {
				WindowEvent::CursorMoved { position, .. } => {
					let prev_mouse_pos = mouse_pos;
					mouse_pos = (position.x as f32, position.y as f32).into();
					let mouse_delta = mouse_pos - prev_mouse_pos;

					if mouse_down_left {
						state.camera.arcball(mouse_delta);
					}
					if mouse_down_right {
						state.camera.zoom(mouse_delta.y);
					}
					if mouse_down_middle {
						state.camera.pan(mouse_delta);
					}
				}
				WindowEvent::MouseWheel {
					delta: MouseScrollDelta::LineDelta(_, dy),
					..
				} => state.camera.zoom(-dy * 25.0),
				WindowEvent::KeyboardInput {
					input:
						KeyboardInput {
							state: ElementState::Pressed,
							virtual_keycode: Some(key),
							..
						},
					..
				} => match key {
					VirtualKeyCode::Escape => *control_flow = ControlFlow::Exit,
					VirtualKeyCode::R => state.camera.look_at_origin(),
					_ => (),
				},
				WindowEvent::MouseInput { state, button, .. } => {
					let is_down = match state {
						ElementState::Pressed => true,
						ElementState::Released => false,
					};
					match button {
						MouseButton::Left => mouse_down_left = is_down,
						MouseButton::Right => mouse_down_right = is_down,
						MouseButton::Middle => mouse_down_middle = is_down,
						_ => (),
					}
				}
				WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
				WindowEvent::Resized(physical_size) => {
					state.resize(*physical_size);
				}
				WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
					state.resize(**new_inner_size);
				}
				_ => {}
			},
			Event::RedrawRequested(window_id) if window_id == state.window().id() => {
				state.update();
				match state.render() {
					Ok(_) => {}
					// Reconfigure the surface if it's lost or outdated
					Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
						// state.resize(state.size)
					}
					// The system is out of memory, we should probably quit
					Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
					// We're ignoring timeouts
					Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
				}
			}
			_ => {}
		}
	});
}
