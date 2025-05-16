use winit::{application::ApplicationHandler, event::WindowEvent, window::WindowAttributes};

use crate::client::rendering::{RenderContext, RenderEngine};

/// The game's event loop handler.
pub struct App {
	pub window_attributes: Option<WindowAttributes>,
	pub render_engine: Option<RenderEngine>,
	pub headless: bool,
}

impl App {
	pub fn new_windowed(
		window_attributes: WindowAttributes,
		event_loop: &winit::event_loop::EventLoop<()>,
	) -> Self {
		App {
			window_attributes: Some(window_attributes),
			render_engine: Some(RenderEngine::new(event_loop).unwrap()),
			headless: false,
		}
	}

	pub fn new_headless() -> Self {
		App {
			window_attributes: None,
			render_engine: None,
			headless: true,
		}
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
		if let Some(window_attributes) = self.window_attributes.as_ref() {
			if let Some(render_engine) = self.render_engine.as_mut() {
				// Rendering initialization
				let render_context =
					RenderContext::new(render_engine, window_attributes, event_loop).unwrap();

				render_engine.render_context = Some(render_context);
			}
		}
	}

	fn window_event(
		&mut self,
		event_loop: &winit::event_loop::ActiveEventLoop,
		_window_id: winit::window::WindowId,
		event: winit::event::WindowEvent,
	) {
		match event {
			WindowEvent::CloseRequested => {
				event_loop.exit();
			},
			WindowEvent::Resized(_) => {
				if let Some(render_engine) = &mut self.render_engine {
					if let Some(render_context) = &mut render_engine.render_context {
						render_context.recreate_swapchain = true;
					}
				}
			},
			WindowEvent::RedrawRequested => {
				if let Some(render_engine) = &mut self.render_engine {
					if render_engine.render_context.is_some() {
						render_engine.render(event_loop).unwrap();
						render_engine
							.render_context
							.as_ref()
							.unwrap()
							.window
							.request_redraw();
					}
				}
			},
			_ => (),
		}
	}

	fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
		if let Some(render_engine) = &self.render_engine {
			if let Some(rcx) = &render_engine.render_context {
				rcx.window.request_redraw();
			}
		}
	}
}
