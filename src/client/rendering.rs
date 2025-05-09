use vulkano::{
	instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
	VulkanLibrary,
};

use crate::error;

pub fn init() -> Result<(), error::InitError> {
	let library = VulkanLibrary::new()?;
	let instance = Instance::new(
		library,
		InstanceCreateInfo {
			flags: InstanceCreateFlags::empty(),
			..Default::default()
		},
	)?;

	Ok(())
}
