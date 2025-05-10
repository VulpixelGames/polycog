use std::sync::Arc;

use vulkano::{
	VulkanLibrary,
	instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
	swapchain::Surface,
};
use winit::window::Window;

use crate::{constants, error};

pub struct RenderData {
	pub window: Arc<Window>,
	pub surface: Arc<Surface>,
}

/// Rendering initialization called upon first Resumed event in the event loop.
/// See [crate::event::App]
pub fn init(
	window: Arc<Window>,
	event_loop: &winit::event_loop::ActiveEventLoop,
) -> Result<Arc<Surface>, error::InitError> {
	// Initialize Vulkan
	let vk_library = VulkanLibrary::new()?;
	let required_extensions = Surface::required_extensions(&event_loop)?;
	let vk_instance = Instance::new(
		vk_library,
		InstanceCreateInfo {
			flags: InstanceCreateFlags::empty(),
			application_name: Some(constants::NAME.to_string()),
			enabled_extensions: required_extensions,
			..Default::default()
		},
	)?;
	let vk_surface = Surface::from_window(vk_instance.clone(), window.clone())?;

	Ok(vk_surface)
}

