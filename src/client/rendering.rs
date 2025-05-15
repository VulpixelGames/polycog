use std::sync::Arc;

use vulkano::{
	Validated, VulkanError, VulkanLibrary,
	buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
	command_buffer::{
		AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
		RenderPassBeginInfo, SubpassBeginInfo, SubpassContents, SubpassEndInfo,
		allocator::{CommandBufferAllocator, StandardCommandBufferAllocator},
	},
	device::{
		Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo,
		QueueFlags, physical::PhysicalDeviceType,
	},
	image::{Image, ImageLayout, ImageUsage, view::ImageView},
	instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
	memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
	pipeline::{
		DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo,
		graphics::{
			GraphicsPipelineCreateInfo,
			color_blend::{ColorBlendAttachmentState, ColorBlendState},
			input_assembly::InputAssemblyState,
			multisample::MultisampleState,
			rasterization::RasterizationState,
			vertex_input::{Vertex, VertexDefinition},
			viewport::{Viewport, ViewportState},
		},
		layout::PipelineDescriptorSetLayoutCreateInfo,
	},
	render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
	shader::ShaderModule,
	swapchain::{
		Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo, acquire_next_image,
	},
	sync::{self, GpuFuture},
};
use winit::window::Window;

use crate::{constants, error};

#[derive(BufferContents, Vertex)]
#[repr(C)]
pub struct TriangleVertex {
	#[format(R32G32_SFLOAT)]
	position: [f32; 2],
}

pub struct RenderData {
	pub window: Arc<Window>,
	pub device: Arc<Device>,
	pub queues: Vec<Arc<Queue>>,
	pub swapchain: Arc<Swapchain>,
	/// Images used as an attachment.
	pub attachment_image_views: Vec<Arc<ImageView>>,
	/// Set to true when the swapchain must be recreated.
	/// This is typically after a window resize occurs.
	pub recreate_swapchain: bool,
	pub memory_allocator: Arc<StandardMemoryAllocator>,
	pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
	pub render_pass: Arc<RenderPass>,
	pub framebuffers: Vec<Arc<Framebuffer>>,
	pub vertex_buffer: Subbuffer<[TriangleVertex]>,
	pub triangle_vs: Arc<ShaderModule>,
	pub triangle_fs: Arc<ShaderModule>,
	pub pipeline: Arc<GraphicsPipeline>,
	pub viewport: Viewport,
	pub previous_frame_end: Option<Box<dyn GpuFuture>>,
}

/// Rendering initialization called upon first Resumed event in the event loop.
/// See [crate::event::App]
pub fn init(
	window: Arc<Window>,
	event_loop: &winit::event_loop::ActiveEventLoop,
) -> Result<RenderData, error::InitError> {
	// Initialize Vulkan
	let vk_library = VulkanLibrary::new()?;
	let window_extensions = Surface::required_extensions(&event_loop)?;
	let instance = Instance::new(
		vk_library,
		InstanceCreateInfo {
			flags: InstanceCreateFlags::empty(),
			application_name: Some(constants::NAME.to_string()),
			enabled_extensions: window_extensions,
			..Default::default()
		},
	)?;
	let surface = Surface::from_window(instance.clone(), window.clone())?;

	// Find and rank devices
	let device_extensions = DeviceExtensions {
		khr_swapchain: true,
		khr_synchronization2: true,
		..Default::default()
	};
	let device_features = DeviceFeatures {
		dynamic_rendering: true,
		..Default::default()
	};
	let physical_device_optional = instance
		.enumerate_physical_devices()?
		.filter(|device| device.supported_extensions().contains(&device_extensions))
		.filter(|device| device.supported_features().contains(&device_features))
		.filter_map(|device| {
			device
				.queue_family_properties()
				.iter()
				.enumerate()
				.position(|(queue_family_index, queue)| {
					// Find the first queue family that supports graphics and our surface
					queue.queue_flags.contains(QueueFlags::GRAPHICS)
						&& device
							.surface_support(queue_family_index as u32, &surface)
							.unwrap_or(false)
				})
				.map(|queue_family_index| (device, queue_family_index as u32))
		})
		.min_by_key(|(device, _)| {
			// Pick the best type of physical graphics device.
			// Lowest score is preferred.
			match device.properties().device_type {
				PhysicalDeviceType::DiscreteGpu => 0,
				PhysicalDeviceType::IntegratedGpu => 1,
				PhysicalDeviceType::VirtualGpu => 2,
				PhysicalDeviceType::Cpu => 3,
				_ => 4,
			}
		});
	let (physical_device, queue_family_index) = if let Some(x) = physical_device_optional {
		x
	} else {
		return Err(error::InitError::NoSuitableDevice);
	};
	log::info!(
		"Found suitable physical graphics device: {} @ {:02x}:{:02x}:{:02x}",
		physical_device.properties().device_name,
		physical_device.properties().pci_bus.unwrap_or(256),
		physical_device.properties().pci_device.unwrap_or(256),
		physical_device.properties().pci_function.unwrap_or(256),
	);

	// Create device
	let (device, queues) = Device::new(
		Arc::clone(&physical_device),
		DeviceCreateInfo {
			enabled_extensions: device_extensions,
			enabled_features: device_features,
			queue_create_infos: vec![QueueCreateInfo {
				queue_family_index,
				..Default::default()
			}],
			..Default::default()
		},
	)
	.map(|(device, queues)| (device, queues.collect::<Vec<_>>()))?;
	log::info!("Found suitable graphics device: {device:?}");

	// Create swapchain
	let surface_capabilities =
		physical_device.surface_capabilities(&surface, Default::default())?;
	let dimensions = window.inner_size();
	let composite_alpha_optional = surface_capabilities
		.supported_composite_alpha
		.into_iter()
		.next();
	let composite_alpha = if let Some(x) = composite_alpha_optional {
		x
	} else {
		return Err(error::InitError::NoCapability(
			"Composite Alpha".to_string(),
		));
	};
	// Just get the first image format, nobody cares.
	let image_format = physical_device.surface_formats(&surface, Default::default())?[0].0;
	let (swapchain, images) = Swapchain::new(
		Arc::clone(&device),
		Arc::clone(&surface),
		SwapchainCreateInfo {
			min_image_count: surface_capabilities.min_image_count + 1,
			image_format,
			image_extent: dimensions.into(),
			image_usage: ImageUsage::COLOR_ATTACHMENT,
			composite_alpha,
			..Default::default()
		},
	)?;
	let attachment_image_views = create_swapchain_images(&images)?;

	// Create allocator
	let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(Arc::clone(&device)));
	let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
		Arc::clone(&device),
		Default::default(),
	));

	// Create render pass
	let render_pass = vulkano::single_pass_renderpass!(
		Arc::clone(&device),
		attachments: {
			color: {
				format: swapchain.image_format(),
				samples: 1,
				load_op: Clear,
				store_op: Store,
			},
		},
		pass: {
			color: [color],
			depth_stencil: {},
		},
	)?;

	// Create framebuffers
	let framebuffers = create_framebuffers(render_pass.clone(), &attachment_image_views)?;

	// Create vertex buffer
	let vertex1 = TriangleVertex {
		position: [-0.5, -0.5],
	};
	let vertex2 = TriangleVertex {
		position: [0.0, 0.5],
	};
	let vertex3 = TriangleVertex {
		position: [0.5, -0.5],
	};
	let vertex_buffer = Buffer::from_iter(
		memory_allocator.clone(),
		BufferCreateInfo {
			usage: BufferUsage::VERTEX_BUFFER,
			..Default::default()
		},
		AllocationCreateInfo {
			memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
				| MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
			..Default::default()
		},
		vec![vertex1, vertex2, vertex3],
	)?;

	// Create graphics pipeline
	let vs = triangle_vs::load(Arc::clone(&device))?;
	let fs = triangle_fs::load(Arc::clone(&device))?;
	let pipeline = create_pipeline(
		Arc::clone(&device),
		Arc::clone(&render_pass),
		Arc::clone(&vs),
		Arc::clone(&fs),
	)?;

	// Create viewport
	let viewport = Viewport {
		extent: window.inner_size().into(),
		..Default::default()
	};

	// Create synchronization objects
	let previous_frame_end = Some(sync::now(device.clone()).boxed());

	Ok(RenderData {
		window,
		device,
		queues,
		swapchain,
		attachment_image_views,
		recreate_swapchain: false,
		memory_allocator,
		command_buffer_allocator,
		render_pass,
		framebuffers,
		vertex_buffer,
		triangle_vs: vs,
		triangle_fs: fs,
		pipeline,
		viewport,
		previous_frame_end,
	})
}

pub fn create_swapchain_images(
	images: &[Arc<Image>],
) -> Result<Vec<Arc<ImageView>>, error::RenderError> {
	Ok(images
		.iter()
		.map(|image| ImageView::new_default(image.clone()))
		.collect::<Result<Vec<_>, _>>()?)
}

pub fn create_framebuffers(
	render_pass: Arc<RenderPass>,
	attachment_image_views: &[Arc<ImageView>],
) -> Result<Vec<Arc<Framebuffer>>, error::RenderError> {
	Ok(attachment_image_views
		.iter()
		.map(|image_view| {
			Framebuffer::new(
				render_pass.clone(),
				FramebufferCreateInfo {
					attachments: vec![image_view.clone()],
					..Default::default()
				},
			)
		})
		.collect::<Result<Vec<_>, _>>()?)
}

pub fn create_command_buffers<V: Vertex>(
	framebuffers: &[Arc<Framebuffer>],
	command_buffer_allocator: Arc<dyn CommandBufferAllocator>,
	queue: Arc<Queue>,
	viewport: Viewport,
	pipeline: Arc<GraphicsPipeline>,
	vertex_buffer: Subbuffer<[V]>,
) -> Result<Vec<Arc<PrimaryAutoCommandBuffer>>, error::RenderError> {
	Ok(framebuffers
		.iter()
		.map(|framebuffer| {
			let mut builder = AutoCommandBufferBuilder::primary(
				command_buffer_allocator.clone(),
				queue.queue_family_index(),
				CommandBufferUsage::OneTimeSubmit,
			)?;

			builder
				.begin_render_pass(
					RenderPassBeginInfo {
						clear_values: vec![Some(constants::SKY_COLOR.into())],
						..RenderPassBeginInfo::framebuffer(framebuffer.clone())
					},
					SubpassBeginInfo {
						contents: SubpassContents::Inline,
						..Default::default()
					},
				)?
				.set_viewport(0, [viewport.clone()].into_iter().collect())?
				.bind_pipeline_graphics(pipeline.clone())?
				.bind_vertex_buffers(0, vertex_buffer.clone())?;
			unsafe { builder.draw(vertex_buffer.len() as u32, 1, 0, 0)? }
				.end_render_pass(SubpassEndInfo::default())?;
			builder.build()
		})
		.collect::<Result<Vec<_>, _>>()?)
}

pub fn render(
	render_data: &mut RenderData,
	_event_loop: &winit::event_loop::ActiveEventLoop,
) -> Result<(), error::RenderError> {
	let window_size = render_data.window.inner_size();
	let queue = &render_data.queues[0];

	let mut previous_frame_end = render_data
		.previous_frame_end
		.take()
		.expect("previous_frame_end should've been replaced");

	// Free unnecessary resources primarily to avoid OOM'ing.
	// According to the Vulkano Triangle 1.3 example, this polls a few fences to see what the GPU has already processed.
	previous_frame_end.cleanup_finished();

	// If the window was resized or the swapchain needs to be recreated, recreate it.
	if render_data.recreate_swapchain {
		let (new_swapchain, new_images) = render_data.swapchain.recreate(SwapchainCreateInfo {
			image_extent: window_size.into(),
			..render_data.swapchain.create_info()
		})?;
		render_data.swapchain = new_swapchain;
		render_data.attachment_image_views = create_swapchain_images(&new_images)?;
		render_data.framebuffers = create_framebuffers(
			render_data.render_pass.clone(),
			&render_data.attachment_image_views,
		)?;
		render_data.viewport.extent = window_size.into();
		render_data.recreate_swapchain = false;
	}

	// Acquire the image before rendering. If no image is available (draw commands were issued too quickly), the
	// function will block.
	let (image_index, suboptimal, acquire_future) =
		match acquire_next_image(render_data.swapchain.clone(), None) {
			Ok(r) => r,
			Err(Validated::Error(VulkanError::OutOfDate)) => {
				render_data.recreate_swapchain = true;
				return Ok(());
			},
			Err(e) => return Err(error::RenderError::Vulkan(e)),
		};

	// If the acquired image is suboptimal, it may not display correctly, so fix it in that case.
	if suboptimal {
		render_data.recreate_swapchain = true;
	}

	let command_buffers = create_command_buffers(
		&render_data.framebuffers,
		render_data.command_buffer_allocator.clone(),
		queue.clone(),
		render_data.viewport.clone(),
		render_data.pipeline.clone(),
		render_data.vertex_buffer.clone(),
	)?;

	let future = previous_frame_end
		.join(acquire_future)
		.then_execute(queue.clone(), command_buffers[image_index as usize].clone())?
		.then_swapchain_present(
			queue.clone(),
			SwapchainPresentInfo::swapchain_image_index(render_data.swapchain.clone(), image_index),
		)
		.then_signal_fence_and_flush()?;

	match future.wait(None) {
		Err(Validated::Error(VulkanError::OutOfDate)) => {
			render_data.recreate_swapchain = true;
		},
		Err(e) => {
			log::error!("failed to flush future: {e}");
		},
		_ => (),
	}

	// Recreate synchronization objects
	render_data.previous_frame_end = Some(sync::now(render_data.device.clone()).boxed());

	Ok(())
}

pub fn create_pipeline(
	device: Arc<Device>,
	render_pass: Arc<RenderPass>,
	vs: Arc<ShaderModule>,
	fs: Arc<ShaderModule>,
) -> Result<Arc<GraphicsPipeline>, error::InitError> {
	let vs = vs.entry_point("main").unwrap();
	let fs = fs.entry_point("main").unwrap();

	let vertex_input_state = TriangleVertex::per_vertex().definition(&vs)?;

	let stages = [
		PipelineShaderStageCreateInfo::new(vs),
		PipelineShaderStageCreateInfo::new(fs),
	];

	let layout = PipelineLayout::new(
		Arc::clone(&device),
		PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
			.into_pipeline_layout_create_info(Arc::clone(&device))?,
	)?;

	let subpass = Subpass::from(Arc::clone(&render_pass), 0).expect("subpass should exist");

	Ok(GraphicsPipeline::new(
		Arc::clone(&device),
		None,
		GraphicsPipelineCreateInfo {
			stages: stages.into_iter().collect(),
			vertex_input_state: Some(vertex_input_state),
			input_assembly_state: Some(InputAssemblyState::default()),
			viewport_state: Some(ViewportState::default()),
			rasterization_state: Some(RasterizationState::default()),
			multisample_state: Some(MultisampleState::default()),
			color_blend_state: Some(ColorBlendState::with_attachment_states(
				subpass.num_color_attachments(),
				ColorBlendAttachmentState::default(),
			)),
			subpass: Some(subpass.into()),
			dynamic_state: [DynamicState::Viewport].into_iter().collect(),
			..GraphicsPipelineCreateInfo::layout(layout)
		},
	)?)
}

mod triangle_vs {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "assets/shader/triangle.vert",
	}
}

mod triangle_fs {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "assets/shader/triangle.frag",
	}
}
