use ash::vk;

use crate::renderer::{find_memorytype_index, record_submit_commandbuffer};

pub struct DepthImageComponents {
    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,
}

pub const DEPTH_IMAGE_FORMAT: vk::Format = vk::Format::D16_UNORM;

impl DepthImageComponents {
    pub fn new(
        device: &ash::Device,
        device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        surface_resolution: &vk::Extent2D,
        setup_command_buffer: &vk::CommandBuffer,
        setup_commands_reuse_fence: &vk::Fence,
        present_queue: &vk::Queue,
    ) -> Self {
        let sr = surface_resolution.clone();
        let depth_image_create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(DEPTH_IMAGE_FORMAT)
            .extent(sr.into())
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let depth_image = unsafe { device.create_image(&depth_image_create_info, None).unwrap() };

        let depth_image_memory_reqs = unsafe { device.get_image_memory_requirements(depth_image) };

        let depth_image_memory_index = find_memorytype_index(
            &depth_image_memory_reqs,
            &device_memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .expect("Cannot find suitable memory index for depth image");

        let depth_image_allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(depth_image_memory_reqs.size)
            .memory_type_index(depth_image_memory_index);

        let depth_image_memory = unsafe {
            device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap()
        };

        unsafe {
            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("Faile to bind depth image memory")
        };

        record_submit_commandbuffer(
            &device,
            *setup_command_buffer,
            *setup_commands_reuse_fence,
            *present_queue,
            &[],
            &[],
            &[],
            |device, setup_command_buffer| {
                let layout_transition_barrier = vk::ImageMemoryBarrier::default()
                    .image(depth_image)
                    .dst_access_mask(
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .layer_count(1)
                            .level_count(1),
                    );
                unsafe {
                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barrier],
                    )
                };
            },
        );

        let depth_image_view_info = vk::ImageViewCreateInfo::default()
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .level_count(1)
                    .layer_count(1),
            )
            .image(depth_image)
            .format(depth_image_create_info.format)
            .view_type(vk::ImageViewType::TYPE_2D);

        let depth_image_view = unsafe {
            device
                .create_image_view(&depth_image_view_info, None)
                .unwrap()
        };

        Self {
            depth_image,
            depth_image_memory,
            depth_image_view,
        }
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.device_wait_idle().unwrap();
            device.destroy_image_view(self.depth_image_view, None);
            device.destroy_image(self.depth_image, None);
            device.free_memory(self.depth_image_memory, None);
        }
    }
}
