use std::ffi::{c_char, CStr};

use ash::{
    khr::{surface, swapchain},
    vk::{self, ClearValue, ImageSubresourceRange, PhysicalDeviceType},
};
use camera::MODEL_MATRIX;
use descriptor_components::UniformBufferObject;
use graphics_pipeline_components::GraphicsPipelineComponents;
use nalgebra::{Matrix4, Normed, Vector3, Vector4};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowAttributes,
};

pub mod camera;
mod debug_components;
mod descriptor_components;
mod graphics_pipeline_components;
mod index_buffer_components;
mod resize_dependent_components;
mod shader_components;
mod vertex_buffer_components;

// Assume unused variables are required for persistence
#[allow(unused)]
pub struct Renderer {
    entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,

    #[cfg(debug_assertions)]
    debug_components: debug_components::DebugComponents,

    surface_loader: surface::Instance,
    swapchain_loader: swapchain::Device,

    physical_device: vk::PhysicalDevice,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    queue_family_index: u32,
    present_queue: vk::Queue,

    pub window: winit::window::Window,
    surface: vk::SurfaceKHR,

    resize_dependent_components: resize_dependent_components::ResizeDependentComponents,

    descriptor_components: descriptor_components::DescriptorComponents,

    command_pool: vk::CommandPool,
    draw_command_buffer: vk::CommandBuffer,
    setup_command_buffer: vk::CommandBuffer,

    present_complete_semaphore: vk::Semaphore,
    rendering_complete_semaphore: vk::Semaphore,

    draw_commands_reuse_fence: vk::Fence,
    setup_commands_reuse_fence: vk::Fence,

    shader_components: shader_components::ShaderComponents,

    graphics_pipeline_components: graphics_pipeline_components::GraphicsPipelineComponents,

    vertex_buffer_components: vertex_buffer_components::VertexBufferComponents,
    index_buffer_components: index_buffer_components::IndexBufferComponents,

    pub resize_dependent_component_rebuild_needed: bool,
}

impl Renderer {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let window = event_loop
            .create_window(WindowAttributes::default())
            .expect("Failed to create winit window");

        let validation_layer_names =
            [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];

        let validation_layer_names_raw: Vec<*const c_char> = if cfg!(debug_assertions) {
            validation_layer_names
                .iter()
                .map(|name| name.as_ptr())
                .collect()
        } else {
            vec![]
        };

        let mut extension_names =
            ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())
                .unwrap()
                .to_vec();
        extension_names.push(ash::ext::debug_utils::NAME.as_ptr());

        let entry = unsafe { ash::Entry::load().unwrap() };

        let application_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_3);

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&application_info)
            .enabled_layer_names(&validation_layer_names_raw)
            .enabled_extension_names(&extension_names);

        let instance = unsafe { entry.create_instance(&instance_create_info, None).unwrap() };

        #[cfg(debug_assertions)]
        let debug_components = debug_components::DebugComponents::new(&entry, &instance);

        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
            .unwrap()
        };

        let physical_devices = unsafe { instance.enumerate_physical_devices().unwrap() };

        let surface_loader = surface::Instance::new(&entry, &instance);

        let (queue_family_index, physical_device) = physical_devices
            .iter()
            .filter_map(|physical_device| unsafe {
                instance
                    .get_physical_device_queue_family_properties(*physical_device)
                    .iter()
                    .enumerate()
                    .find_map(|(index, info)| {
                        let supports_graphics_and_surface =
                            info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                && surface_loader
                                    .get_physical_device_surface_support(
                                        *physical_device,
                                        index as u32,
                                        surface,
                                    )
                                    .unwrap();
                        if supports_graphics_and_surface {
                            Some((index as u32, *physical_device))
                        } else {
                            None
                        }
                    })
            })
            .max_by_key(|(_index, physical_device)| {
                let device_properties =
                    unsafe { instance.get_physical_device_properties(*physical_device) };
                let mut score = 0;
                match device_properties.device_type {
                    PhysicalDeviceType::DISCRETE_GPU => score += 1000,
                    PhysicalDeviceType::INTEGRATED_GPU => score += 100,
                    PhysicalDeviceType::VIRTUAL_GPU => score += 10,
                    PhysicalDeviceType::CPU => score += 1,
                    _ => (),
                }
                score += device_properties.limits.max_image_dimension2_d;
                score
            })
            .expect("No supported physical device found");

        let device_extension_names_raw = [swapchain::NAME.as_ptr()];

        let features = vk::PhysicalDeviceFeatures {
            shader_clip_distance: 1,
            ..Default::default()
        };

        let mut dynamic_rendering_features =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);

        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extension_names_raw)
            .push_next(&mut dynamic_rendering_features)
            .enabled_features(&features);

        let device = unsafe {
            instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap()
        };

        let present_queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        let swapchain_loader = swapchain::Device::new(&instance, &device);

        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family_index);

        let command_pool = unsafe { device.create_command_pool(&pool_create_info, None).unwrap() };

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_buffer_count(2)
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY);

        let command_buffers = unsafe {
            device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap()
        };

        let setup_command_buffer = command_buffers[0];

        let draw_command_buffer = command_buffers[1];

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let draw_commands_reuse_fence = unsafe {
            device
                .create_fence(&fence_create_info, None)
                .expect("Failed to create fence")
        };

        let setup_commands_reuse_fence = unsafe {
            device
                .create_fence(&fence_create_info, None)
                .expect("Failed to create fence")
        };

        let semaphore_create_info = vk::SemaphoreCreateInfo::default();

        let present_complete_semaphore = unsafe {
            device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap()
        };

        let rendering_complete_semaphore = unsafe {
            device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap()
        };

        let index_buffer_components =
            index_buffer_components::IndexBufferComponents::new(&device, &device_memory_properties);

        let vertex_buffer_components = vertex_buffer_components::VertexBufferComponents::new(
            &device,
            &device_memory_properties,
        );

        let shader_components = shader_components::ShaderComponents::new(&device);

        let resize_dependent_components =
            resize_dependent_components::ResizeDependentComponents::new(
                &device,
                &window,
                &surface,
                &surface_loader,
                &swapchain_loader,
                &physical_device,
                &setup_command_buffer,
                &setup_commands_reuse_fence,
                &device_memory_properties,
                &present_queue,
            );

        let descriptor_components = descriptor_components::DescriptorComponents::new(
            &device,
            &device_memory_properties,
            resize_dependent_components
                .swapchain_components
                .present_images
                .len() as u32,
        );

        let graphics_pipeline_components = GraphicsPipelineComponents::new(
            &device,
            &resize_dependent_components
                .swapchain_components
                .surface_format,
            &shader_components.shader_stage_infos(),
            &[descriptor_components.descriptor_set_layout],
            &resize_dependent_components.scissors,
            &resize_dependent_components.viewports,
        );

        Self {
            entry,
            instance,
            device,
            surface_loader,
            swapchain_loader,
            physical_device,
            device_memory_properties,
            window,
            queue_family_index,
            present_queue,
            surface,
            resize_dependent_components,
            command_pool,
            draw_command_buffer,
            setup_command_buffer,
            present_complete_semaphore,
            rendering_complete_semaphore,
            draw_commands_reuse_fence,
            setup_commands_reuse_fence,
            graphics_pipeline_components,
            shader_components,
            index_buffer_components,
            vertex_buffer_components,
            descriptor_components,
            resize_dependent_component_rebuild_needed: false,

            #[cfg(debug_assertions)]
            debug_components,
        }
    }

    pub fn draw_frame(&mut self, camera: &camera::Camera) {
        if self.resize_dependent_component_rebuild_needed {
            unsafe { self.device.device_wait_idle().unwrap() };
            self.resize_dependent_components
                .cleanup(&self.device, &self.swapchain_loader);
            self.resize_dependent_components =
                resize_dependent_components::ResizeDependentComponents::new(
                    &self.device,
                    &self.window,
                    &self.surface,
                    &self.surface_loader,
                    &self.swapchain_loader,
                    &self.physical_device,
                    &self.setup_command_buffer,
                    &self.setup_commands_reuse_fence,
                    &self.device_memory_properties,
                    &self.present_queue,
                );
            self.resize_dependent_component_rebuild_needed = false;
        }

        unsafe {
            self.device
                .wait_for_fences(&[self.draw_commands_reuse_fence], true, u64::MAX)
                .unwrap()
        };

        let next_image_result = unsafe {
            self.swapchain_loader.acquire_next_image(
                self.resize_dependent_components
                    .swapchain_components
                    .swapchain,
                u64::MAX,
                self.present_complete_semaphore,
                vk::Fence::null(),
            )
        };

        let present_index = match next_image_result {
            Ok((present_index, suboptimal)) => {
                if suboptimal {
                    self.resize_dependent_component_rebuild_needed = true;
                }
                present_index
            }
            Err(e) => {
                if e == vk::Result::ERROR_OUT_OF_DATE_KHR {
                    self.resize_dependent_component_rebuild_needed = true;
                    return;
                }
                panic!("Failed to acquire next image: {:?}", e);
            }
        } as usize;

        if self.descriptor_components.uniform_buffer_mappings.is_some() {
            let mappings = self
                .descriptor_components
                .uniform_buffer_mappings
                .as_mut()
                .unwrap();
            mappings[present_index].copy_from_slice(&[UniformBufferObject {
                model_matrix: camera::MODEL_MATRIX,
                view_matrix: camera.view_matrix(),
                projection_matrix: camera.projection_matrix(),
            }]);
        }
        dbg!(camera);
        let test_transformed_vertex = 
            camera.projection_matrix()
            * camera.view_matrix()
            * camera::MODEL_MATRIX
            * Vector4::new(
                vertex_buffer_components::VERTICES[0].position[0],
                vertex_buffer_components::VERTICES[0].position[1],
                vertex_buffer_components::VERTICES[0].position[2],
                1.0,
            );
        dbg!(test_transformed_vertex);

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .image_view(
                self.resize_dependent_components
                    .swapchain_components
                    .present_image_views[present_index],
            );

        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .clear_value(ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 }
            })
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .image_view(
                self.resize_dependent_components
                    .depth_image_components
                    .depth_image_view,
            );

        let color_attachments = &[color_attachment];
        let rendering_info = vk::RenderingInfo::default()
             .depth_attachment(&depth_attachment)
            .color_attachments(color_attachments)
            .layer_count(1)
            .render_area(
                self.resize_dependent_components
                    .swapchain_components
                    .surface_resolution
                    .into(),
            );

        record_submit_commandbuffer(
            &self.device,
            self.draw_command_buffer,
            self.draw_commands_reuse_fence,
            self.present_queue,
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
            &[self.present_complete_semaphore],
            &[self.rendering_complete_semaphore],
            |device, draw_command_buffer| {
                unsafe {
                    // dynamic rendering image layout transiton. see https://lesleylai.info/en/vk-khr-dynamic-rendering/
                    let image_subresource_range = ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1);
                    let image_memory_barrier = vk::ImageMemoryBarrier::default()
                        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .image(
                            self.resize_dependent_components
                                .swapchain_components
                                .present_images[present_index],
                        )
                        .subresource_range(image_subresource_range);
                    device.cmd_pipeline_barrier(
                        draw_command_buffer,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[image_memory_barrier],
                    );

                    // rendering
                    device.cmd_begin_rendering(self.draw_command_buffer, &rendering_info);
                    device.cmd_bind_pipeline(
                        draw_command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.graphics_pipeline_components.graphics_pipelines[0],
                    );
                    device.cmd_set_scissor(
                        draw_command_buffer,
                        0,
                        &self.resize_dependent_components.scissors,
                    );
                    device.cmd_set_viewport(
                        draw_command_buffer,
                        0,
                        &self.resize_dependent_components.viewports,
                    );
                    device.cmd_bind_vertex_buffers(
                        draw_command_buffer,
                        0,
                        &[self.vertex_buffer_components.vertex_buffer],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        draw_command_buffer,
                        self.index_buffer_components.index_buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_bind_descriptor_sets(
                        draw_command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.graphics_pipeline_components.pipeline_layout,
                        0,
                        &[self.descriptor_components.descriptor_sets[present_index]],
                        &[],
                    );
                    device.cmd_draw_indexed(
                        draw_command_buffer,
                        index_buffer_components::INDICES.len() as u32,
                        1,
                        0,
                        0,
                        1,
                    );
                    device.cmd_end_rendering(draw_command_buffer);

                    // dynamic rendering image layout transiton. see https://lesleylai.info/en/vk-khr-dynamic-rendering/
                    let image_subresource_range = ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1);
                    let image_memory_barrier = vk::ImageMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(
                            self.resize_dependent_components
                                .swapchain_components
                                .present_images[present_index],
                        )
                        .subresource_range(image_subresource_range);
                    device.cmd_pipeline_barrier(
                        draw_command_buffer,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[image_memory_barrier],
                    );
                };
            },
        );

        let wait_semaphores = [self.rendering_complete_semaphore];

        let swapchains = [self
            .resize_dependent_components
            .swapchain_components
            .swapchain];

        let image_indices = [present_index as u32];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let present_result = unsafe {
            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
        };

        match present_result {
            Err(e) => {
                if e == vk::Result::ERROR_OUT_OF_DATE_KHR || e == vk::Result::SUBOPTIMAL_KHR {
                    self.resize_dependent_component_rebuild_needed = true;
                } else {
                    panic!("Failed to present image {:?}", e);
                }
            }
            _ => (),
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.graphics_pipeline_components.cleanup(&self.device);
            self.shader_components.cleanup(&self.device);
            self.index_buffer_components.cleanup(&self.device);
            self.vertex_buffer_components.cleanup(&self.device);
            self.descriptor_components.cleanup(&self.device);
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_commands_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_commands_reuse_fence, None);
            self.resize_dependent_components
                .cleanup(&self.device, &self.swapchain_loader);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            #[cfg(debug_assertions)]
            self.debug_components.cleanup();
            self.instance.destroy_instance(None);
        }
    }
}

fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _memory_type)| index as _)
}

fn record_submit_commandbuffer<F: FnOnce(&ash::Device, vk::CommandBuffer)>(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    command_buffer_reuse_fence: vk::Fence,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .wait_for_fences(&[command_buffer_reuse_fence], true, u64::MAX)
            .expect("Wait for fence failed.");

        device
            .reset_fences(&[command_buffer_reuse_fence])
            .expect("Reset fences failed.");

        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer failed.");

        f(device, command_buffer);

        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer failed.");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
            .expect("queue submit failed.");
    }
}
