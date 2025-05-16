pub mod scene;

use std::sync::Arc;

use scene::SceneTask;
use vulkano::{
	Validated, VulkanError, VulkanLibrary,
	device::{
		Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo,
		QueueFlags, physical::PhysicalDeviceType,
	},
	format::Format,
	image::{Image, ImageCreateInfo, ImageUsage},
	instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
	memory::allocator::AllocationCreateInfo,
	pipeline::graphics::viewport::Viewport,
	swapchain::{Surface, Swapchain, SwapchainCreateInfo},
};
use vulkano_taskgraph::{
	Id, QueueFamilyType,
	descriptor_set::{BindlessContext, BindlessContextCreateInfo, LocalDescriptorSetCreateInfo},
	graph::{AttachmentInfo, CompileInfo, ExecutableTaskGraph, ExecuteError, TaskGraph},
	resource::{AccessTypes, Flight, ImageLayoutType, Resources, ResourcesCreateInfo},
	resource_map,
};
use winit::window::{Window, WindowAttributes};

use crate::{constants, error};

const MAX_FRAMES_IN_FLIGHT: u32 = 1;
const MIN_SWAPCHAIN_IMAGES: u32 = MAX_FRAMES_IN_FLIGHT + 1;

pub struct Queues {
	pub graphics: Arc<Queue>,
}

pub struct RenderEngine {
	pub instance: Arc<Instance>,
	pub device: Arc<Device>,
	pub queues: Queues,
	pub resources: Arc<Resources>,
	pub flight_id: Id<Flight>,
	pub render_context: Option<RenderContext>,
}

pub struct RenderContext {
	pub window: Arc<Window>,
	pub swapchain_id: Id<Swapchain>,
	pub virtual_swapchain_id: Id<Swapchain>,
	pub viewport: Viewport,
	/// Set to true when the swapchain must be recreated.
	/// This is typically after a window resize occurs.
	pub recreate_swapchain: bool,
	pub task_graph: ExecutableTaskGraph<Self>,
	pub images: Images,
	pub virtual_images: Arc<Images>,
}

impl RenderEngine {
	/// Rendering initialization called upon event loop creation.
	/// See [crate::event::App]
	pub fn new(event_loop: &winit::event_loop::EventLoop<()>) -> Result<Self, error::InitError> {
		// Initialize Vulkan
		let vk_library = VulkanLibrary::new()?;
		let instance_extensions = Surface::required_extensions(&event_loop)?;
		let instance_create_info = InstanceCreateInfo {
			flags: InstanceCreateFlags::empty(),
			application_name: Some(constants::NAME),
			enabled_extensions: &instance_extensions,
			..Default::default()
		};
		let instance = Instance::new(&vk_library, &instance_create_info)?;

		// Find and rank devices
		let device_extensions = DeviceExtensions {
			khr_swapchain: true,
			khr_synchronization2: true,
			..BindlessContext::required_extensions(&instance)
		};
		let device_features = DeviceFeatures {
			dynamic_rendering: true,
			..BindlessContext::required_features(&instance)
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
								.presentation_support(queue_family_index as u32, event_loop)
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
			"Found suitable graphics device: {} @ {:02x}:{:02x}:{:02x} (type {:?})",
			physical_device.properties().device_name,
			physical_device.properties().pci_bus.unwrap_or(256),
			physical_device.properties().pci_device.unwrap_or(256),
			physical_device.properties().pci_function.unwrap_or(256),
			physical_device.properties().device_type,
		);

		// Create device
		let queue_create_infos = [QueueCreateInfo {
			queue_family_index,
			..Default::default()
		}];
		let device_create_info = DeviceCreateInfo {
			enabled_extensions: &device_extensions,
			enabled_features: &device_features,
			queue_create_infos: &queue_create_infos,
			..Default::default()
		};
		let (device, mut queues) = Device::new(&physical_device, &device_create_info)?;

		let graphics_queue = queues.next().unwrap();

		let queues = Queues {
			graphics: graphics_queue,
		};

		let resources = Resources::new(
			&device,
			&ResourcesCreateInfo {
				bindless_context: Some(&BindlessContextCreateInfo {
					local_set: Some(&LocalDescriptorSetCreateInfo::default()),
					..Default::default()
				}),
				..Default::default()
			},
		)?;

		let flight_id = resources.create_flight(MAX_FRAMES_IN_FLIGHT)?;

		Ok(Self {
			instance,
			device,
			queues,
			resources,
			flight_id,
			render_context: None,
		})
	}

	pub fn render(
		&mut self,
		_event_loop: &winit::event_loop::ActiveEventLoop,
	) -> Result<(), error::RenderError> {
		let rcx = self
			.render_context
			.as_mut()
			.expect("render_context must be initialized before rendering can begin");
		let window_size = rcx.window.inner_size();

		let flight = self
			.resources
			.flight(self.flight_id)
			.expect("flight resource should be initialized");

		// If the window was resized or the swapchain needs to be recreated, recreate it.
		if rcx.recreate_swapchain {
			rcx.swapchain_id =
				self.resources
					.recreate_swapchain(rcx.swapchain_id, |create_info| SwapchainCreateInfo {
						image_extent: window_size.into(),
						..*create_info
					})?;

			rcx.viewport.extent = window_size.into();

			self.resources
				.create_deferred_batch()
				.destroy_image(rcx.images.light_heatmap_image_id);

			rcx.images = Images::new(&self.resources, rcx.swapchain_id)?;

			rcx.recreate_swapchain = false;
		}

		flight.wait(None)?;

		let resource_map = resource_map!(
			&rcx.task_graph,
			rcx.virtual_swapchain_id => rcx.swapchain_id,
			rcx.virtual_images.light_heatmap_image_id => rcx.images.light_heatmap_image_id,
		)?;

		match unsafe {
			rcx.task_graph
				.execute(resource_map, rcx, || rcx.window.pre_present_notify())
		} {
			Ok(()) => {},
			Err(ExecuteError::Swapchain {
				error: Validated::Error(VulkanError::OutOfDate),
				..
			}) => {
				rcx.recreate_swapchain = true;
			},
			Err(e) => {
				error!("failed to execute next frame");
				return Err(e.into());
			},
		}

		Ok(())
	}
}

impl RenderContext {
	/// Rendering initialization called during the resumed event in the event loop.
	/// See [crate::event::App]
	pub fn new(
		render_engine: &RenderEngine,
		window_attributes: &WindowAttributes,
		event_loop: &winit::event_loop::ActiveEventLoop,
	) -> Result<Self, error::InitError> {
		// Create window & surface
		let window = Arc::new(event_loop.create_window(window_attributes.clone())?);
		let surface = Surface::from_window(&render_engine.instance, &window)?;

		// Create swapchain
		let surface_capabilities = render_engine
			.device
			.physical_device()
			.surface_capabilities(&surface, &Default::default())?;
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
		let (swapchain_image_format, _) = render_engine
			.device
			.physical_device()
			.surface_formats(&surface, &Default::default())?[0];
		let swapchain_id = render_engine.resources.create_swapchain(
			render_engine.flight_id,
			&surface,
			&SwapchainCreateInfo {
				min_image_count: MIN_SWAPCHAIN_IMAGES,
				image_format: swapchain_image_format,
				image_extent: dimensions.into(),
				image_usage: ImageUsage::COLOR_ATTACHMENT,
				composite_alpha,
				..Default::default()
			},
		)?;

		let mut task_graph = TaskGraph::new(&render_engine.resources);

		let images = Images::new(&render_engine.resources, swapchain_id)?;
		let virtual_images = Arc::new(Images::new_virtual(&mut task_graph));
		let virtual_swapchain_id = task_graph.add_swapchain(&SwapchainCreateInfo {
			image_format: swapchain_image_format,
			..Default::default()
		});
		let virtual_framebuffer_id = task_graph.add_framebuffer();

		let scene_node_id = task_graph
			.create_task_node(
				"Scene",
				QueueFamilyType::Graphics,
				SceneTask::new(render_engine, virtual_swapchain_id)?,
			)
			.framebuffer(virtual_framebuffer_id)
			.color_attachment(
				virtual_swapchain_id.current_image_id(),
				AccessTypes::COLOR_ATTACHMENT_READ | AccessTypes::COLOR_ATTACHMENT_WRITE,
				ImageLayoutType::Optimal,
				&AttachmentInfo {
					clear: true,
					..Default::default()
				},
			)
			.build();

		let mut task_graph = unsafe {
			task_graph.compile(&CompileInfo {
				queues: &[&render_engine.queues.graphics],
				present_queue: Some(&render_engine.queues.graphics),
				flight_id: render_engine.flight_id,
				..Default::default()
			})?
		};

		let scene_node = task_graph.task_node_mut(scene_node_id)?;
		let subpass = scene_node
			.subpass()
			.expect("scene node should be part of a subpass")
			.clone();
		scene_node
			.task_mut()
			.downcast_mut::<SceneTask>()
			.unwrap()
			.create_pipeline(render_engine, subpass)?;

		// Create viewport
		let viewport = Viewport {
			extent: window.inner_size().into(),
			..Default::default()
		};

		Ok(Self {
			window,
			swapchain_id,
			virtual_swapchain_id,
			viewport,
			recreate_swapchain: false,
			task_graph,
			images,
			virtual_images,
		})
	}
}

pub struct Images {
	pub light_heatmap_image_id: Id<Image>,
}

impl Images {
	pub fn new(
		resources: &Resources,
		swapchain_id: Id<Swapchain>,
	) -> Result<Self, error::RenderError> {
		let swapchain_state = resources.swapchain(swapchain_id)?;
		let images = swapchain_state.images();
		let extent = images[0].extent();

		let light_heatmap_image_id = resources.create_image(
			&ImageCreateInfo {
				extent: [extent[0] / 2, extent[1] / 2, 1],
				format: Format::R16G16_SFLOAT,
				usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
				..Default::default()
			},
			&AllocationCreateInfo::default(),
		)?;

		Ok(Self {
			light_heatmap_image_id,
		})
	}

	pub fn new_virtual<W>(task_graph: &mut TaskGraph<W>) -> Self {
		let light_heatmap_image_id = task_graph.add_image(&ImageCreateInfo {
			format: Format::R16G16_SFLOAT,
			usage: ImageUsage::TRANSIENT_ATTACHMENT,
			..Default::default()
		});

		Self {
			light_heatmap_image_id,
		}
	}
}
