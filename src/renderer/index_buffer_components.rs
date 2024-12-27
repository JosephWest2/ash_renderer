use ash::vk;

use super::buffer::Buffer;

pub type Index = u32;
pub const INDICES: [Index; 6] = [0, 1, 2, 3, 4, 5];

pub struct IndexBufferComponents {
    pub index_buffer: Buffer<Index>,
    pub index_staging_buffer: Buffer<Index>,
}

impl IndexBufferComponents {
    pub fn new_unintiailized(
        device: &ash::Device,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> IndexBufferComponents {
        let index_buffer = Buffer::<Index>::new(
            device,
            physical_device_memory_properties,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            INDICES.len(),
            false,
        );
        let index_staging_buffer = Buffer::<Index>::new(
            device,
            physical_device_memory_properties,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            INDICES.len(),
            false,
        );
        IndexBufferComponents {
            index_buffer,
            index_staging_buffer,
        }
    }
    pub fn update_indices(
        &mut self,
        device: &ash::Device,
        indices: &[Index],
        command_buffer: vk::CommandBuffer,
        command_buffer_reuse_fence: vk::Fence,
        queue: vk::Queue,
    ) {
        self.index_staging_buffer.write_data_direct(device, indices);
        self.index_buffer.write_from_staging(
            &self.index_staging_buffer,
            device,
            command_buffer,
            command_buffer_reuse_fence,
            queue,
        );
    }
    pub fn cleanup(&self, device: &ash::Device) {
        self.index_buffer.cleanup(device);
        self.index_staging_buffer.cleanup(device);
    }
}
