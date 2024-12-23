use ash::vk;

use super::find_memorytype_index;

pub struct VertexBufferComponents {
    pub vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}
pub const VERTICES: [Vertex; 6] = [
    Vertex {
        position: [-1.0, 1.0, 2.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 2.0],
        color: [0.0, 0.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.0, -1.0, 2.0],
        color: [1.0, 0.0, 0.0, 1.0],
    },
    Vertex {
        position: [-1.0, -1.0, 3.0],
        color: [0.0, 1.0, 0.5, 1.0],
    },
    Vertex {
        position: [1.0, -1.0, 3.0],
        color: [0.5, 0.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.0, 1.0, 3.0],
        color: [1.0, 0.5, 0.0, 1.0],
    },
];

impl VertexBufferComponents {
    pub fn new(device: &ash::Device, device_memory_properties: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let vertex_input_buffer_info = vk::BufferCreateInfo::default()
            .size(VERTICES.len() as u64 * size_of::<Vertex>() as u64)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let vertex_buffer = unsafe {
            device
                .create_buffer(&vertex_input_buffer_info, None)
                .unwrap()
        };

        let vertex_buffer_memory_reqs =
            unsafe { device.get_buffer_memory_requirements(vertex_buffer) };

        let vertex_input_buffer_memory_index = find_memorytype_index(
            &vertex_buffer_memory_reqs,
            device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Failed to find suitable memory type for vertex buffer");

        let vertex_buffer_allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(vertex_buffer_memory_reqs.size)
            .memory_type_index(vertex_input_buffer_memory_index);

        let vertex_buffer_memory = unsafe {
            device
                .allocate_memory(&vertex_buffer_allocate_info, None)
                .unwrap()
        };
            unsafe { device
                .bind_buffer_memory(vertex_buffer, vertex_buffer_memory, 0)
                .unwrap() };

        let vert_ptr = unsafe {
            device
                .map_memory(
                    vertex_buffer_memory,
                    0,
                    vertex_buffer_memory_reqs.size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap()
        };

        let mut vert_align = unsafe {
            ash::util::Align::new(
                vert_ptr,
                align_of::<Vertex>() as u64,
                vertex_buffer_memory_reqs.size,
            )
        };
        vert_align.copy_from_slice(&VERTICES);

        unsafe {
            device.unmap_memory(vertex_buffer_memory);
        };

        Self {
            vertex_buffer,
            vertex_buffer_memory
        }

    }

    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.free_memory(self.vertex_buffer_memory, None);
            device.destroy_buffer(self.vertex_buffer, None);
        }
    }

}
