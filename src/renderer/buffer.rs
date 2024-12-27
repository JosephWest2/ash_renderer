use ash::vk;

use crate::renderer::command_buffer_components::record_submit_commandbuffer;

use super::find_memorytype_index;

pub struct Buffer<T> {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    size: usize,
    usage: vk::BufferUsageFlags,
    memory_properties: vk::MemoryPropertyFlags,
    mapping: Option<ash::util::Align<T>>,
}

impl<T: Copy> Buffer<T> {
    pub fn new(
        device: &ash::Device,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        memory_properties: vk::MemoryPropertyFlags,
        buffer_len: usize,
        persistent_mapping: bool,
    ) -> Self {
        let buffer_size = size_of::<T>() * buffer_len;
        let buffer_create_info = vk::BufferCreateInfo::default()
            .size(buffer_size as u64)
            .usage(usage)
            .sharing_mode(sharing_mode);

        let buffer = unsafe { device.create_buffer(&buffer_create_info, None).unwrap() };

        let buffer_memory_reqs = unsafe { device.get_buffer_memory_requirements(buffer) };

        let buffer_memory_index = find_memorytype_index(
            &buffer_memory_reqs,
            physical_device_memory_properties,
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

        let mapping = match persistent_mapping {
            true => {
                let data_ptr = unsafe {
                    device
                        .map_memory(
                            memory,
                            0,
                            buffer_memory_reqs.size,
                            vk::MemoryMapFlags::empty(),
                        )
                        .unwrap()
                };

                let vert_align = unsafe {
                    ash::util::Align::new(data_ptr, align_of::<T>() as u64, buffer_memory_reqs.size)
                };
                Some(vert_align)
            }
            false => None,
        };

        Self {
            buffer,
            memory,
            size: buffer_size,
            usage,
            memory_properties,
            mapping,
        }
    }
    pub fn write_data_direct(&mut self, device: &ash::Device, data: &[T]) {
        assert_eq!(
            self.memory_properties & vk::MemoryPropertyFlags::HOST_VISIBLE,
            vk::MemoryPropertyFlags::HOST_VISIBLE
        );
        assert_eq!(
            self.memory_properties & vk::MemoryPropertyFlags::HOST_COHERENT,
            vk::MemoryPropertyFlags::HOST_COHERENT
        );
        assert!(data.len() <= self.size);
        if self.mapping.is_some() {
            self.mapping.as_mut().unwrap().copy_from_slice(data);
            return;
        }
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
            submit_queue,
            command_buffer,
            command_buffer_reuse_fence,
            &[],
            &[],
            &[],
            |device, command_buffer| unsafe {
                device.cmd_copy_buffer(
                    command_buffer,
                    staging_buffer.buffer,
                    self.buffer,
                    &[copy_region],
                );
            },
        );
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.destroy_buffer(self.buffer, None);
            device.free_memory(self.memory, None);
        }
    }
}
