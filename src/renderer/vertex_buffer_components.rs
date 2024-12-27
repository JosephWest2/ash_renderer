use ash::vk;

use super::buffer::Buffer;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

pub const VERTICES: [Vertex; 6] = [
    Vertex {
        position: [-1.0, 1.0, 2.0],
        color: [1.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 2.0],
        color: [1.0, 0.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.0, -1.0, 2.0],
        color: [1.0, 1.0, 0.0, 1.0],
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

pub struct VertexBufferComponents {
    pub vertex_buffer: Buffer<Vertex>,
    pub vertex_staging_buffer: Buffer<Vertex>,
}
impl VertexBufferComponents {
    pub fn new_unintialized(
        device: &ash::Device,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> VertexBufferComponents {
        let vertex_buffer = Buffer::<Vertex>::new(
            device,
            physical_device_memory_properties,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            VERTICES.len(),
            false,
        );
        let vertex_staging_buffer = Buffer::<Vertex>::new(
            device,
            physical_device_memory_properties,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            VERTICES.len(),
            false,
        );
        VertexBufferComponents {
            vertex_buffer,
            vertex_staging_buffer,
        }
    }
    pub fn update_vertices(
        &mut self,
        device: &ash::Device,
        vertices: &[Vertex],
        command_buffer: vk::CommandBuffer,
        command_buffer_reuse_fence: vk::Fence,
        queue: vk::Queue,
    ) {
        self.vertex_staging_buffer.write_data_direct(device, vertices);
        self.vertex_buffer.write_from_staging(
            &self.vertex_staging_buffer,
            device,
            command_buffer,
            command_buffer_reuse_fence,
            queue,
        );
    }
    pub fn cleanup(&self, device: &ash::Device) {
        self.vertex_buffer.cleanup(device);
        self.vertex_staging_buffer.cleanup(device);
    }

}
