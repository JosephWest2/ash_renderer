use ash::vk;
use nalgebra::Matrix4;

use super::{buffer::Buffer};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UniformBuffers {
    pub model_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub projection_matrix: Matrix4<f32>,
}

pub struct DescriptorComponents {
    pub descriptor_pool: vk::DescriptorPool,
    pub uniform_buffer_descriptor_sets: Vec<vk::DescriptorSet>,
    pub uniform_buffer_descriptor_set_layout: vk::DescriptorSetLayout,
    pub uniform_buffers: Vec<Buffer<UniformBuffers>>,
}

impl DescriptorComponents {
    pub fn new(
        device: &ash::Device,
        physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        present_image_count: u32,
    ) -> DescriptorComponents {
        // Buffers
        let mut uniform_buffers = Vec::with_capacity(present_image_count as usize);
        for _ in 0..present_image_count {
            let uniform_buffer = Buffer::<UniformBuffers>::new(
                device,
                physical_device_memory_properties,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::SharingMode::EXCLUSIVE,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                1,
                true,
            );
            uniform_buffers.push(uniform_buffer);
        }

        // Uniform Buffer Descriptor Sets
        let uniform_buffer_descriptor_set_layout_bindings =
            [vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX)];

        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&uniform_buffer_descriptor_set_layout_bindings);

        let uniform_buffer_descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                .expect("Failed to create descriptor set layout.")
        };

        let pool_sizes = [vk::DescriptorPoolSize::default()
            .descriptor_count(present_image_count)
            .ty(vk::DescriptorType::UNIFORM_BUFFER)];

        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(present_image_count);

        let descriptor_pool = unsafe {
            device
                .create_descriptor_pool(&pool_create_info, None)
                .expect("Failed to create descriptor pool.")
        };

        let set_layouts = vec![uniform_buffer_descriptor_set_layout; present_image_count as usize];

        let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);

        let uniform_buffer_descriptor_sets = unsafe {
            device
                .allocate_descriptor_sets(&descriptor_set_allocate_info)
                .expect("Failed to allocate descriptor sets.")
        };

        for i in 0..uniform_buffer_descriptor_sets.len() {
            let descriptor_buffer_info = [vk::DescriptorBufferInfo::default()
                .buffer(uniform_buffers[i].buffer)
                .offset(0)
                .range(size_of::<UniformBuffers>() as u64)];

            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(uniform_buffer_descriptor_sets[i])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .buffer_info(&descriptor_buffer_info);

            unsafe {
                device.update_descriptor_sets(&[descriptor_write], &[]);
            }
        }

        DescriptorComponents {
            descriptor_pool,
            uniform_buffer_descriptor_set_layout,
            uniform_buffer_descriptor_sets,
            uniform_buffers,
        }
    }

    pub fn cleanup(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.uniform_buffer_descriptor_set_layout, None);
            for i in 0..self.uniform_buffers.len() {
                self.uniform_buffers[i].cleanup(device);
            }
        }
    }
}
