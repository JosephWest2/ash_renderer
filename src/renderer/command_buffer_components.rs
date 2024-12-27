use ash::vk;

pub struct CommandBufferComponents {
    pub reuse_command_pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub draw_commands_reuse_fence: vk::Fence,
    pub setup_command_buffer: vk::CommandBuffer,
    pub setup_commands_reuse_fence: vk::Fence,
}

impl CommandBufferComponents {
    pub fn new(graphics_queue_family_index: u32, device: &ash::Device) -> CommandBufferComponents {
        let reuse_pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(graphics_queue_family_index);

        let reuse_command_pool = unsafe {
            device
                .create_command_pool(&reuse_pool_create_info, None)
                .unwrap()
        };

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_buffer_count(2)
            .command_pool(reuse_command_pool)
            .level(vk::CommandBufferLevel::PRIMARY);

        let command_buffers = unsafe {
            device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap()
        };

        let setup_command_buffer = command_buffers[0];

        let draw_command_buffer = command_buffers[1];

        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let draw_commands_reuse_fence = unsafe {
            device
                .create_fence(&fence_create_info, None)
                .expect("Failed to create fence")
        };

        let setup_commands_reuse_fence = unsafe {
            device
                .create_fence(&fence_create_info, None)
                .expect("Failed to create fence")
        };

        CommandBufferComponents {
            reuse_command_pool,
            draw_command_buffer,
            draw_commands_reuse_fence,
            setup_command_buffer,
            setup_commands_reuse_fence,
        }
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.destroy_command_pool(self.reuse_command_pool, None);
            device.destroy_fence(self.setup_commands_reuse_fence, None);
            device.destroy_fence(self.draw_commands_reuse_fence, None);
        }
    }
}

pub fn record_submit_commandbuffer<F: FnOnce(&ash::Device, vk::CommandBuffer)>(
    device: &ash::Device,
    queue: vk::Queue,
    command_buffer: vk::CommandBuffer,
    command_buffer_reuse_fence: vk::Fence,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    submission_function: F,
) {
    unsafe {
        device
            .wait_for_fences(&[command_buffer_reuse_fence], true, u64::MAX)
            .expect("Wait for fence failed.");

        device
            .reset_fences(&[command_buffer_reuse_fence])
            .expect("Reset fences failed.");

        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer failed.");

        (submission_function)(device, command_buffer);

        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer failed.");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(queue, &[submit_info], command_buffer_reuse_fence)
            .expect("queue submit failed.");
    }
}


