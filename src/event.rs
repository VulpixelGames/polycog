use std::{panic::panic_any, sync::Arc};

use winit::{application::ApplicationHandler, event::WindowEvent, window::WindowAttributes};

use crate::{
	client::rendering::{self, RenderData},
	error,
};

/// The game's event loop handler.
pub struct App {
	pub window_attributes: Option<WindowAttributes>,
	pub render_data: Option<RenderData>,
	pub headless: bool,
}

impl App {
	pub fn new_windowed(window_attributes: WindowAttributes) -> Self {
		App {
			window_attributes: Some(window_attributes),
			render_data: None,
			headless: false,
		}
	}

	pub fn new_headless() -> Self {
		App {
			window_attributes: None,
			render_data: None,
			headless: true,
		}
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
		if let Some(window_attributes) = self.window_attributes.as_ref() {
			// Window initialization
			let window = event_loop.create_window(window_attributes.clone()).unwrap();

			// Rendering initialization
			let render_data = rendering::init(Arc::new(window), event_loop).unwrap();

			self.render_data = Some(render_data);
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
				if let Some(render_data) = &mut self.render_data {
					render_data.recreate_swapchain = true;
				}
			},
			WindowEvent::RedrawRequested => {
				if let Some(render_data) = &mut self.render_data {
					rendering::render(render_data, event_loop).unwrap();
					render_data.window.request_redraw();
				}
			},
			_ => (),
		}
	}
}
