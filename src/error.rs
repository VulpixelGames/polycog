use std::{backtrace::Backtrace, panic::PanicHookInfo};

use thiserror::Error;

use crate::{client::rendering::RenderContext, constants};

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

		let backtrace = Backtrace::capture();

		if let Some(location) = info.location() {
			println!("[{}:{}] {payload_msg}", location.file(), location.line());
		} else {
			println!("{info}");
		}
		println!("{backtrace}");
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
	#[error("error while rendering: {0}")]
	Render(#[from] RenderError),
	#[error("event loop: {0}")]
	EventLoop(#[from] winit::error::EventLoopError),
}

#[derive(Error, Debug)]
pub enum InitError {
	#[error("{0}")]
	RenderError(#[from] RenderError),
	#[error("Vulkan is missing or unsupported; check to see if your system supports Vulkan: {0}")]
	VulkanLoad(#[from] vulkano::LoadingError),
	#[error("Vulkan: {0:?}")]
	Vulkan(#[from] vulkano::Validated<vulkano::VulkanError>),
	#[error("Vulkan: {0:?}")]
	VulkanUnvalidated(#[from] vulkano::VulkanError),
	#[error("allocating Vulkan image: {0:?}")]
	AllocateImage(#[from] vulkano::Validated<vulkano::image::AllocateImageError>),
	#[error("allocating Vulkan buffer: {0:?}")]
	AllocateBuffer(#[from] vulkano::Validated<vulkano::buffer::AllocateBufferError>),
	#[error("Vulkan validation error: {0}")]
	Validation(#[from] Box<vulkano::ValidationError>),
	#[error("creating IntoPipelineLayoutCreateInfo: {0}")]
	IntoPipelineLayoutCreateInfo(
		#[from] vulkano::pipeline::layout::IntoPipelineLayoutCreateInfoError,
	),
	#[error("no suitable physical graphics device (\"GPU\") found")]
	NoSuitableDevice,
	#[error("no capability: {0}")]
	NoCapability(String),
	#[error("event loop: {0}")]
	EventLoop(#[from] winit::error::EventLoopError),
	#[error("OS error (windowing): {0}")]
	WinitOsError(#[from] winit::error::OsError),
	#[error("window handle error: {0}")]
	WinitHandleError(#[from] winit::raw_window_handle::HandleError),
	#[error("{0}")]
	VulkanoFromWindow(#[from] vulkano::swapchain::FromWindowError),
	#[error("Vulkan task graph compile: {0}")]
	TaskGraphCompile(#[from] vulkano_taskgraph::graph::CompileError<RenderContext>),
	#[error("Vulkan resource does not exist: {0}")]
	InvalidSlot(#[from] vulkano_taskgraph::InvalidSlotError),
	#[error("Vulkan task graph execute: {0} ({0:?})")]
	TaskGraphExecute(#[from] vulkano_taskgraph::graph::ExecuteError),
	#[error("Vulkan task graph: {0}")]
	TaskGraph(#[from] vulkano_taskgraph::graph::TaskGraphError),
}

#[derive(Error, Debug)]
pub enum RenderError {
	#[error("Vulkan: {0:?}")]
	Vulkan(#[from] vulkano::Validated<vulkano::VulkanError>),
	#[error("Vulkan: {0}")]
	VulkanUnvalidated(#[from] vulkano::VulkanError),
	#[error("Vulkan validation error: {0}")]
	Validation(#[from] Box<vulkano::ValidationError>),
	#[error("allocating Vulkan buffer: {0:?}")]
	AllocateBuffer(#[from] vulkano::Validated<vulkano::buffer::AllocateBufferError>),
	#[error("Vulkan command buffer execution: {0}")]
	CommandBufferExec(#[from] vulkano::command_buffer::CommandBufferExecError),
	#[error("allocating Vulkan image: {0:?}")]
	AllocateImage(#[from] vulkano::Validated<vulkano::image::AllocateImageError>),
	#[error("Vulkan resource does not exist: {0}")]
	InvalidSlot(#[from] vulkano_taskgraph::InvalidSlotError),
	#[error("Vulkan task graph execute: {0} ({0:?})")]
	TaskGraphExecute(#[from] vulkano_taskgraph::graph::ExecuteError),
}
