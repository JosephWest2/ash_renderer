use std::marker::PhantomData;

use ash::vk;

use super::buffer::{self, Buffer};

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

pub struct VertexBuffer {
    vertex_buffer: Buffer<Vertex>,
    staging_buffer: Buffer<Vertex>,
}

impl VertexBuffer {
    pub fn new(
        device: &ash::Device,
        device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> Self {
        let vertex_buffer = Buffer::new(
            device,
            device_memory_properties,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            PhantomData::<Vertex>,
            VERTICES.len(),
        );
        let staging_buffer = Buffer::new(
            device,
            device_memory_properties,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            PhantomData::<Vertex>,
            VERTICES.len(),
        );
        Self {
            vertex_buffer,
            staging_buffer,
        }
    }
    pub fn update_vertices(&self, device: &ash::Device, vertices: &[Vertex]) {
        self.staging_buffer.write_data_direct(device, vertices);
        self.vertex_buffer.write_from_staging(self.staging_buffer);
    }
}
