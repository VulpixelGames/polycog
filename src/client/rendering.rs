use vulkano::{
	instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
	VulkanLibrary,
};

pub fn init() -> anyhow::Result<()> {
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
