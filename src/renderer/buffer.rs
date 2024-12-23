use std::marker::PhantomData;

use ash::vk;

use crate::renderer::record_submit_commandbuffer;

use super::find_memorytype_index;

pub struct Buffer<T> {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    size: usize,
    usage: vk::BufferUsageFlags,
    memory_properties: vk::MemoryPropertyFlags,
    _data_type: PhantomData<T>,
}

impl<T: Copy> Buffer<T> {
    pub fn new(
        device: &ash::Device,
        device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        memory_properties: vk::MemoryPropertyFlags,
        data_type: PhantomData<T>,
        buffer_size: usize,
    ) -> Self {
        let buffer_create_info = vk::BufferCreateInfo::default()
            .size(buffer_size as u64)
            .usage(usage)
            .sharing_mode(sharing_mode);

        let buffer = unsafe { device.create_buffer(&buffer_create_info, None).unwrap() };

        let buffer_memory_reqs = unsafe { device.get_buffer_memory_requirements(buffer) };

        let buffer_memory_index = find_memorytype_index(
            &buffer_memory_reqs,
            device_memory_properties,
            memory_properties,
        )
        .expect("Failed to find suitable memory type for buffer");

        let buffer_allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(buffer_memory_reqs.size)
            .memory_type_index(buffer_memory_index);

        let memory = unsafe { device.allocate_memory(&buffer_allocate_info, None).unwrap() };

        unsafe {
            device
                .bind_buffer_memory(buffer, memory, 0)
                .expect("Failed to bind buffer memory")
        };

        Self {
            buffer,
            memory,
            size: buffer_size,
            usage,
            memory_properties,
            _data_type: data_type,
        }
    }

    pub fn write_data_direct(&self, device: &ash::Device, data: &[T]) {
        assert_eq!(
            self.memory_properties & vk::MemoryPropertyFlags::HOST_VISIBLE,
            vk::MemoryPropertyFlags::HOST_VISIBLE
        );
        assert_eq!(
            self.memory_properties & vk::MemoryPropertyFlags::HOST_COHERENT,
            vk::MemoryPropertyFlags::HOST_COHERENT
        );
        assert!(data.len() <= self.size);
        let buffer_memory_reqs = unsafe { device.get_buffer_memory_requirements(self.buffer) };

        let data_ptr = unsafe {
            device
                .map_memory(
                    self.memory,
                    0,
                    buffer_memory_reqs.size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap()
        };

        let mut vert_align = unsafe {
            ash::util::Align::new(data_ptr, align_of::<T>() as u64, buffer_memory_reqs.size)
        };
        vert_align.copy_from_slice(data);

        unsafe {
            device.unmap_memory(self.memory);
        };
    }
    pub fn write_from_staging(
        &self,
        staging_buffer: &Buffer<T>,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        command_buffer_reuse_fence: vk::Fence,
        submit_queue: vk::Queue,
    ) {
        assert_eq!(
            self.usage & vk::BufferUsageFlags::TRANSFER_DST,
            vk::BufferUsageFlags::TRANSFER_DST
        );
        assert_eq!(
            staging_buffer.usage & vk::BufferUsageFlags::TRANSFER_SRC,
            vk::BufferUsageFlags::TRANSFER_SRC
        );
        assert!(self.size >= staging_buffer.size);
        let copy_region = vk::BufferCopy::default().size(staging_buffer.size as u64);

        record_submit_commandbuffer(
            device,
            command_buffer,
            command_buffer_reuse_fence,
            submit_queue,
            &[],
            &[],
            &[],
            |device, command_buffer| unsafe {
                device.cmd_copy_buffer(
                    command_buffer,
                    self.buffer,
                    staging_buffer.buffer,
                    &[copy_region],
                );
            },
        );
    }
}
