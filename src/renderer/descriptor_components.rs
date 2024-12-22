use ash::{util::Align, vk};
use nalgebra::Matrix4;

use super::find_memorytype_index;

pub struct DescriptorComponents {
    descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    uniform_buffers: Vec<vk::Buffer>,
    uniform_buffer_memories: Vec<vk::DeviceMemory>,
    pub uniform_buffer_mappings: Option<Vec<Align<UniformBufferObject>>>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UniformBufferObject {
    pub model_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub projection_matrix: Matrix4<f32>,
}

impl DescriptorComponents {
    pub fn new(
        device: &ash::Device,
        device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        present_image_count: u32,
    ) -> Self {
        // Buffers

        let mut uniform_buffers = Vec::with_capacity(present_image_count as usize);
        let mut uniform_buffer_memories = Vec::with_capacity(present_image_count as usize);
        let mut uniform_buffer_mappings = Vec::with_capacity(present_image_count as usize);

        for _ in 0..present_image_count {
            let uniform_buffer_create_info = vk::BufferCreateInfo::default()
                .size(size_of::<UniformBufferObject>() as u64)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let uniform_buffer = unsafe {
                device
                    .create_buffer(&uniform_buffer_create_info, None)
                    .expect("Failed to create Uniform Buffer.")
            };

            let memory_reqs = unsafe { device.get_buffer_memory_requirements(uniform_buffer) };
            let memory_index = find_memorytype_index(
                &memory_reqs,
                device_memory_properties,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
            .expect("Failed to get uniform buffer memtype index.");

            let uniform_buffer_allocate_info = vk::MemoryAllocateInfo::default()
                .allocation_size(memory_reqs.size)
                .memory_type_index(memory_index);
            let uniform_buffer_memory = unsafe {
                device
                    .allocate_memory(&uniform_buffer_allocate_info, None)
                    .unwrap()
            };
            unsafe {
                device
                    .bind_buffer_memory(uniform_buffer, uniform_buffer_memory, 0)
                    .expect("Failed to bind uniform buffer memory");
            }
            let memory_ptr = unsafe {
                device
                    .map_memory(
                        uniform_buffer_memory,
                        0,
                        memory_reqs.size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap()
            };

            let uniform_buffer_align: Align<UniformBufferObject> = unsafe {
                Align::new(
                    memory_ptr,
                    align_of::<UniformBufferObject>() as u64,
                    memory_reqs.size,
                )
            };

            uniform_buffers.push(uniform_buffer);
            uniform_buffer_memories.push(uniform_buffer_memory);
            uniform_buffer_mappings.push(uniform_buffer_align);
        }

        // Descriptor Sets
        let descriptor_set_layout_bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)];

        let descriptor_set_layout_create_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&descriptor_set_layout_bindings);

        let descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                .expect("Failed to create descriptor set layout.")
        };

        let pool_sizes = [vk::DescriptorPoolSize::default().descriptor_count(present_image_count)];

        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(present_image_count);

        let descriptor_pool = unsafe {
            device
                .create_descriptor_pool(&pool_create_info, None)
                .expect("Failed to create descriptor pool.")
        };

        let set_layouts = vec![descriptor_set_layout; present_image_count as usize];

        let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);

        let descriptor_sets = unsafe {
            device
                .allocate_descriptor_sets(&descriptor_set_allocate_info)
                .expect("Failed to allocate descriptor sets.")
        };

        for i in 0..descriptor_sets.len() {
            let descriptor_buffer_info = [vk::DescriptorBufferInfo::default()
                .buffer(uniform_buffers[i])
                .offset(0)
                .range(size_of::<UniformBufferObject>() as u64)];

            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(descriptor_sets[i])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .buffer_info(&descriptor_buffer_info);

            unsafe {
                device.update_descriptor_sets(&[descriptor_write], &[]);
            }
        }

        Self {
            descriptor_pool,
            descriptor_set_layout,
            descriptor_sets,
            uniform_buffers,
            uniform_buffer_memories,
            uniform_buffer_mappings: Some(uniform_buffer_mappings),
        }
    }

    pub fn cleanup(&mut self, device: &ash::Device) {
        unsafe {
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            for i in 0..self.uniform_buffers.len() {
                device.unmap_memory(self.uniform_buffer_memories[i]);
                device.free_memory(self.uniform_buffer_memories[i], None);
                device.destroy_buffer(self.uniform_buffers[i], None);
            }
        }
        self.uniform_buffer_mappings = None;
    }
}
