use std::{slice, sync::Arc};

use vulkano::{
	buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage},
	memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
	pipeline::{
		DynamicState, GraphicsPipeline, PipelineShaderStageCreateInfo,
		graphics::{
			GraphicsPipelineCreateInfo,
			color_blend::{ColorBlendAttachmentState, ColorBlendState},
			input_assembly::InputAssemblyState,
			multisample::MultisampleState,
			rasterization::RasterizationState,
			vertex_input::{Vertex, VertexDefinition},
			viewport::ViewportState,
		},
	},
	render_pass::Subpass,
	swapchain::Swapchain,
};
use vulkano_taskgraph::{Id, Task, resource::HostAccessType};

use crate::{constants, error};

use super::{RenderContext, RenderEngine};

#[derive(Clone, Copy, BufferContents, Vertex)]
#[repr(C)]
pub struct TriangleVertex {
	#[format(R32G32B32_SFLOAT)]
	color: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	position: [f32; 3],
}

pub struct SceneTask {
	pipeline: Option<Arc<GraphicsPipeline>>,
	vertex_buffer_id: Id<Buffer>,
	virtual_swapchain_id: Id<Swapchain>,
}

impl SceneTask {
	pub fn new(
		render_engine: &RenderEngine,
		virtual_swapchain_id: Id<Swapchain>,
	) -> Result<Self, error::InitError> {
		// Create vertex buffer
		let vertices = [
			TriangleVertex {
				color: [1.0, 0.0, 0.0],
				position: [-0.5, -0.5, 0.0],
			},
			TriangleVertex {
				color: [0.0, 1.0, 0.0],
				position: [0.0, 0.5, 0.0],
			},
			TriangleVertex {
				color: [0.0, 0.0, 1.0],
				position: [0.5, -0.5, 0.0],
			},
		];
		let vertex_buffer_id = render_engine.resources.create_buffer(
			&BufferCreateInfo {
				usage: BufferUsage::VERTEX_BUFFER,
				..Default::default()
			},
			&AllocationCreateInfo {
				memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
					| MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
				..Default::default()
			},
			DeviceLayout::for_value(vertices.as_slice())
				.expect("triangle vertex buffer size should be nonzero"),
		)?;

		// FIXME: https://github.com/vulkano-rs/vulkano/blob/3b2b6441177491f69111981a86fa998a3bff4abb/examples/deferred/scene.rs#L69
		render_engine
			.resources
			.flight(render_engine.flight_id)?
			.wait(None)?;

		unsafe {
			vulkano_taskgraph::execute(
				&render_engine.queues.graphics,
				&render_engine.resources,
				render_engine.flight_id,
				|_command_buffer, task_context| {
					task_context
						.write_buffer::<[TriangleVertex]>(vertex_buffer_id, ..)?
						.copy_from_slice(&vertices);

					Ok(())
				},
				[(vertex_buffer_id, HostAccessType::Write)],
				[],
				[],
			)?
		}

		Ok(Self {
			pipeline: None,
			vertex_buffer_id,
			virtual_swapchain_id,
		})
	}

	pub fn create_pipeline(
		&mut self,
		render_engine: &RenderEngine,
		subpass: Subpass,
	) -> Result<(), error::InitError> {
		let bindless_context = render_engine
			.resources
			.bindless_context()
			.expect("bindless_context should be enabled");

		// Create graphics pipeline
		let vs = triangle_vs::load(render_engine.device.clone())?
			.entry_point("main")
			.unwrap();
		let fs = triangle_fs::load(render_engine.device.clone())?
			.entry_point("main")
			.unwrap();
		let vertex_input_state = TriangleVertex::per_vertex().definition(&vs)?;
		let pipeline_stages = [
			PipelineShaderStageCreateInfo::new(vs),
			PipelineShaderStageCreateInfo::new(fs),
		];
		let pipeline_layout = bindless_context.pipeline_layout_from_stages(&pipeline_stages)?;
		let pipeline = GraphicsPipeline::new(
			render_engine.device.clone(),
			None,
			GraphicsPipelineCreateInfo {
				stages: pipeline_stages.into_iter().collect(),
				vertex_input_state: Some(vertex_input_state),
				input_assembly_state: Some(InputAssemblyState::default()),
				viewport_state: Some(ViewportState::default()),
				rasterization_state: Some(RasterizationState::default()),
				//depth_stencil_state: Some(DepthStencilState {
				//	depth: Some(DepthState::simple()),
				//	..Default::default()
				//}),
				multisample_state: Some(MultisampleState::default()),
				color_blend_state: Some(ColorBlendState::with_attachment_states(
					subpass.num_color_attachments(),
					ColorBlendAttachmentState::default(),
				)),
				dynamic_state: [DynamicState::Viewport].into_iter().collect(),
				subpass: Some(subpass.clone().into()),
				..GraphicsPipelineCreateInfo::new(pipeline_layout)
			},
		)?;

		self.pipeline = Some(pipeline);

		Ok(())
	}
}

impl Task for SceneTask {
	type World = RenderContext;

	fn clear_values(&self, clear_values: &mut vulkano_taskgraph::ClearValues<'_>) {
		clear_values.set(
			self.virtual_swapchain_id.current_image_id(),
			constants::SKY_COLOR,
		);
	}

	unsafe fn execute(
		&self,
		cbf: &mut vulkano_taskgraph::command_buffer::RecordingCommandBuffer<'_>,
		_tcx: &mut vulkano_taskgraph::TaskContext<'_>,
		rcx: &Self::World,
	) -> vulkano_taskgraph::TaskResult {
		unsafe {
			// All command buffer calls comply with the Vulkan spec
			cbf.set_viewport(0, slice::from_ref(&rcx.viewport))?;
			cbf.bind_pipeline_graphics(
				self.pipeline
					.as_ref()
					.expect("scene task graphics pipeline should exist"),
			)?;
			cbf.bind_vertex_buffers(0, &[self.vertex_buffer_id], &[0], &[], &[])?;

			cbf.draw(3, 1, 0, 0)?;
		}

		Ok(())
	}
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
