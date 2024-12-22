use std::mem::offset_of;

use ash::vk;

use super::{
    resize_dependent_components::depth_image_components::DEPTH_IMAGE_FORMAT,
    vertex_buffer_components::Vertex,
};

pub struct GraphicsPipelineComponents {
    pub graphics_pipelines: Vec<vk::Pipeline>,
    pub pipeline_layout: vk::PipelineLayout,
}

impl GraphicsPipelineComponents {
    pub fn new(
        device: &ash::Device,
        surface_format: &vk::SurfaceFormatKHR,
        pipeline_shader_stage_infos: &[vk::PipelineShaderStageCreateInfo],
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
        scissors: &[vk::Rect2D],
        viewports: &[vk::Viewport],
    ) -> Self {
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .scissors(scissors)
            .viewports(viewports);

        let noop_stencil_state = vk::StencilOpState::default()
            .fail_op(vk::StencilOp::KEEP)
            .pass_op(vk::StencilOp::KEEP)
            .depth_fail_op(vk::StencilOp::KEEP)
            .compare_op(vk::CompareOp::ALWAYS);

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_bounds_test_enable(true)
            .stencil_test_enable(false)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .front(noop_stencil_state)
            .back(noop_stencil_state)
            .max_depth_bounds(100.0)
            .min_depth_bounds(0.0);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::SRC_COLOR)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_DST_COLOR)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states);

        let layout_create_info =
            vk::PipelineLayoutCreateInfo::default().set_layouts(descriptor_set_layouts);

        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&layout_create_info, None)
                .expect("Failed to create pipeline layout")
        };

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .cull_mode(vk::CullModeFlags::BACK)
            .line_width(1.0)
            .polygon_mode(vk::PolygonMode::FILL);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)];

        let vertex_input_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Vertex, position) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: offset_of!(Vertex, color) as u32,
            },
        ];

        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_attribute_descriptions(&vertex_input_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_input_binding_descriptions);

        let vertex_input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let color_attachment_formats = &[surface_format.format];
        let mut pipeline_rendering_create_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(color_attachment_formats)
            .depth_attachment_format(DEPTH_IMAGE_FORMAT);

        let graphics_pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
            .push_next(&mut pipeline_rendering_create_info)
            .stages(pipeline_shader_stage_infos)
            .dynamic_state(&dynamic_state_info)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .layout(pipeline_layout)
            .rasterization_state(&rasterization_state)
            .viewport_state(&viewport_state)
            .input_assembly_state(&vertex_input_assembly_state)
            .vertex_input_state(&vertex_input_state)
            .depth_stencil_state(&depth_stencil_state);

        let graphics_pipelines = unsafe {
            device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[graphics_pipeline_create_info],
                    None,
                )
                .expect("Failed to create graphics pipelines")
        };

        Self {
            graphics_pipelines,
            pipeline_layout,
        }
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.device_wait_idle().unwrap();
            for &pipeline in self.graphics_pipelines.iter() {
                device.destroy_pipeline(pipeline, None);
            }
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
