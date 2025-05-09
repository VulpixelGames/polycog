use std::panic::PanicHookInfo;

use thiserror::Error;
use vulkano::VulkanError;

use crate::constants;

/// Sets the default panic hook to display a user-friendly UI box.
pub fn set_panic_hook() {
	std::panic::set_hook(Box::new(|info: &PanicHookInfo| {
		println!(
			"{} has encountered an error and must close.",
			constants::NAME
		);

		let payload_msg = if let Some(error) = info.payload().downcast_ref::<GameError>() {
			error.to_string()
		} else if let Some(str_) = info.payload().downcast_ref::<&str>() {
			str_.to_string()
		} else if let Some(string) = info.payload().downcast_ref::<String>() {
			string.clone()
		} else {
			GameError::Unknown.to_string()
		};

		if let Some(location) = info.location() {
			println!("[{}:{}] {payload_msg}", location.file(), location.line())
		} else {
			println!("{info}");
		}
	}));
}

#[derive(Error, Debug)]
pub enum GameError {
	#[error(
		"Invalid panic message. This means the error is either unknown or (more likely) something has gone horribly wrong."
	)]
	Unknown,
	#[error("error in initialization: {0}")]
	Init(#[from] InitError),
}

#[derive(Error, Debug)]
pub enum InitError {
	#[error("Vulkan is missing or unsupported; check to see if your system supports Vulkan")]
	VulkanLoad(#[from] vulkano::LoadingError),
	#[error("rendering: {0}")]
	Rendering(#[from] vulkano::Validated<VulkanError>),
}
