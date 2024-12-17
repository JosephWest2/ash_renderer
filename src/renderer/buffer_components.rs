use ash::vk;

use super::find_memorytype_index;

pub struct BufferComponents {
    vertex_input_buffer_memory: vk::DeviceMemory,
    pub vertex_input_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
}

#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 4],
    pub color: [f32; 4],
}
const VERTICES: [Vertex; 3] = [
    Vertex {
        position: [-1.0, 1.0, 0.0, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 0.0, 1.0],
        color: [0.0, 0.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.0, -1.0, 0.0, 1.0],
        color: [1.0, 0.0, 0.0, 1.0],
    },
];
pub const INDICES: [u32; 3] = [0, 1, 2];

impl BufferComponents {
    pub fn new(
        device: &ash::Device,
        device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> Self {
        let index_buffer_info = vk::BufferCreateInfo::default()
            .size(size_of_val(&INDICES) as u64)
            .usage(vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let index_buffer = unsafe { device.create_buffer(&index_buffer_info, None).unwrap() };

        let index_buffer_memory_reqs =
            unsafe { device.get_buffer_memory_requirements(index_buffer) };

        let index_buffer_memory_index = find_memorytype_index(
            &index_buffer_memory_reqs,
            &device_memory_properties,
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

        unsafe {
            device.unmap_memory(index_buffer_memory);
            device
                .bind_buffer_memory(index_buffer, index_buffer_memory, 0)
                .unwrap()
        };

        let vertex_input_buffer_info = vk::BufferCreateInfo::default()
            .size(3 * size_of::<Vertex>() as u64)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let vertex_input_buffer = unsafe {
            device
                .create_buffer(&vertex_input_buffer_info, None)
                .unwrap()
        };

        let vertex_input_buffer_memory_reqs =
            unsafe { device.get_buffer_memory_requirements(vertex_input_buffer) };

        let vertex_input_buffer_memory_index = find_memorytype_index(
            &vertex_input_buffer_memory_reqs,
            &device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Failed to find suitable memory type for vertex buffer");

        let vertex_buffer_allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(vertex_input_buffer_memory_reqs.size)
            .memory_type_index(vertex_input_buffer_memory_index);

        let vertex_input_buffer_memory = unsafe {
            device
                .allocate_memory(&vertex_buffer_allocate_info, None)
                .unwrap()
        };

        let vert_ptr = unsafe {
            device
                .map_memory(
                    vertex_input_buffer_memory,
                    0,
                    vertex_input_buffer_memory_reqs.size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap()
        };

        let mut vert_align = unsafe {
            ash::util::Align::new(
                vert_ptr,
                align_of::<Vertex>() as u64,
                vertex_input_buffer_memory_reqs.size,
            )
        };
        vert_align.copy_from_slice(&VERTICES);

        unsafe {
            device.unmap_memory(vertex_input_buffer_memory);
            device
                .bind_buffer_memory(vertex_input_buffer, vertex_input_buffer_memory, 0)
                .unwrap();
        };

        Self {
            vertex_input_buffer,
            vertex_input_buffer_memory,
            index_buffer,
            index_buffer_memory,
        }
    }

    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.free_memory(self.vertex_input_buffer_memory, None);
            device.destroy_buffer(self.vertex_input_buffer, None);
            device.free_memory(self.index_buffer_memory, None);
            device.destroy_buffer(self.index_buffer, None);
        }
    }
}
