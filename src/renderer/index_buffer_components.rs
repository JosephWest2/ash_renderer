use ash::vk;

use super::find_memorytype_index;

pub struct IndexBufferComponents {
    pub index_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
}

pub const INDICES: [u32; 6] = [0, 1, 2, 3, 4, 5];

impl IndexBufferComponents {
    pub fn new(device: &ash::Device, device_memory_properties: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let index_buffer_info = vk::BufferCreateInfo::default()
            .size(size_of_val(&INDICES) as u64)
            .usage(vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let index_buffer = unsafe { device.create_buffer(&index_buffer_info, None).unwrap() };

        let index_buffer_memory_reqs =
            unsafe { device.get_buffer_memory_requirements(index_buffer) };

        let index_buffer_memory_index = find_memorytype_index(
            &index_buffer_memory_reqs,
            device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Failed to find memory type index for index buffer");

        let index_allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(index_buffer_memory_reqs.size)
            .memory_type_index(index_buffer_memory_index);

        let index_buffer_memory =
            unsafe { device.allocate_memory(&index_allocate_info, None).unwrap() };

        let index_ptr = unsafe {
            device
                .map_memory(
                    index_buffer_memory,
                    0,
                    index_buffer_memory_reqs.size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap()
        };

        let mut index_slice: ash::util::Align<u32> = unsafe {
            ash::util::Align::new(
                index_ptr,
                align_of::<u32>() as u64,
                index_buffer_memory_reqs.size,
            )
        };
        index_slice.copy_from_slice(&INDICES);
        index_slice.copy_from_slice(&INDICES);
        index_slice.copy_from_slice(&INDICES);
        index_slice.copy_from_slice(&INDICES);

        unsafe {
            device.unmap_memory(index_buffer_memory);
            device
                .bind_buffer_memory(index_buffer, index_buffer_memory, 0)
                .unwrap()
        };

        Self {
            index_buffer,
            index_buffer_memory
        }
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.free_memory(self.index_buffer_memory, None);
            device.destroy_buffer(self.index_buffer, None);
        }
    }
}
