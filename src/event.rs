use std::{panic::panic_any, sync::Arc};

use vulkano::swapchain::Surface;
use winit::{
	application::ApplicationHandler,
	event::WindowEvent,
	window::{Window, WindowAttributes},
};

use crate::{client::rendering, error};

pub struct RenderData {
	pub window: Arc<Window>,
	pub vk_surface: Arc<Surface>,
}

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
			let window;
			let result = event_loop.create_window(window_attributes.clone());
			match result {
				Ok(window_ok) => window = Some(Arc::new(window_ok)),
				Err(error) => {
					panic_any(error::GameError::Init(error.into()));
				},
			}

			// Rendering initialization
			let vk_surface;
			let result = rendering::init(window.clone().unwrap(), event_loop);
			match result {
				Ok(vk_surface_ok) => vk_surface = Some(vk_surface_ok),
				Err(error) => {
					panic_any(error::GameError::Init(error));
				},
			}

			self.render_data = Some(RenderData {
				window: window.unwrap(),
				vk_surface: vk_surface.unwrap(),
			});
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
			WindowEvent::RedrawRequested => {},
			_ => (),
		}
	}
}
