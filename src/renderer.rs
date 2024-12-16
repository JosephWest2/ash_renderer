use std::{
    borrow::Cow,
    ffi::{c_char, CStr},
    mem::offset_of,
};

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk::{self, ImageSubresourceRange, PhysicalDeviceType},
};
use graphics_pipeline_components::GraphicsPipelineComponents;
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowAttributes,
};

use crate::shaders;
mod graphics_pipeline_components;
mod resize_dependent_components;

// Assume unused variables are required for persistence
#[allow(unused)]
pub struct Renderer {
    entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,
    debug_utils_loader: debug_utils::Instance,
    debug_callback: vk::DebugUtilsMessengerEXT,

    surface_loader: surface::Instance,
    swapchain_loader: swapchain::Device,

    physical_device: vk::PhysicalDevice,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    queue_family_index: u32,
    present_queue: vk::Queue,

    pub window: winit::window::Window,
    surface: vk::SurfaceKHR,

    resize_dependent_components: resize_dependent_components::ResizeDependentComponents,

    pool: vk::CommandPool,
    draw_command_buffer: vk::CommandBuffer,
    setup_command_buffer: vk::CommandBuffer,

    present_complete_semaphore: vk::Semaphore,
    rendering_complete_semaphore: vk::Semaphore,

    draw_commands_reuse_fence: vk::Fence,
    setup_commands_reuse_fence: vk::Fence,

    vertex_shader_module: vk::ShaderModule,
    fragment_shader_module: vk::ShaderModule,

    graphics_pipeline_components: graphics_pipeline_components::GraphicsPipelineComponents,

    vertex_input_buffer_memory: vk::DeviceMemory,
    vertex_input_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
    index_buffer: vk::Buffer,

    pub resize_dependent_component_rebuild_needed: bool,
}

impl Renderer {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let window = event_loop
            .create_window(WindowAttributes::default())
            .expect("Failed to create winit window");

        let validation_layer_names =
            [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];

        let validation_layer_names_raw: Vec<*const c_char> = validation_layer_names
            .iter()
            .map(|name| name.as_ptr())
            .collect();

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

        let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        let debug_utils_loader = debug_utils::Instance::new(&entry, &instance);

        let debug_callback = unsafe {
            debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap()
        };

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

        let pool = unsafe { device.create_command_pool(&pool_create_info, None).unwrap() };

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_buffer_count(2)
            .command_pool(pool)
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

        let vertex_shader_code = shaders::compile(
            &include_str!("../shaders/vertex_shader.glsl"),
            shaderc::ShaderKind::Vertex,
            "vertex_shader.glsl",
            "main",
        );

        let vertex_shader_info =
            vk::ShaderModuleCreateInfo::default().code(&vertex_shader_code.as_binary());

        let vertex_shader_module = unsafe {
            device
                .create_shader_module(&vertex_shader_info, None)
                .expect("Failed to create vertex shader module")
        };

        let fragment_shader_code = shaders::compile(
            &include_str!("../shaders/fragment_shader.glsl"),
            shaderc::ShaderKind::Fragment,
            "fragment_shader.glsl",
            "main",
        );

        let fragment_shader_info =
            vk::ShaderModuleCreateInfo::default().code(&fragment_shader_code.as_binary());

        let fragment_shader_module = unsafe {
            device
                .create_shader_module(&fragment_shader_info, None)
                .expect("Failed to create fragment shader module")
        };

        let pipeline_shader_stage_infos = [
            vk::PipelineShaderStageCreateInfo {
                module: vertex_shader_module,
                p_name: c"main".as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                module: fragment_shader_module,
                p_name: c"main".as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];

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

        let graphics_pipeline_components = GraphicsPipelineComponents::new(
            &device,
            &resize_dependent_components
                .swapchain_components
                .surface_format,
            &pipeline_shader_stage_infos,
            &resize_dependent_components.scissors,
            &resize_dependent_components.viewports,
        );

        eprintln!("Renderer Created");

        Self {
            entry,
            instance,
            device,
            surface_loader,
            swapchain_loader,
            debug_utils_loader,
            debug_callback,
            physical_device,
            device_memory_properties,
            window,
            queue_family_index,
            present_queue,
            surface,
            resize_dependent_components,
            pool,
            draw_command_buffer,
            setup_command_buffer,
            present_complete_semaphore,
            rendering_complete_semaphore,
            draw_commands_reuse_fence,
            setup_commands_reuse_fence,
            graphics_pipeline_components,
            vertex_input_buffer,
            index_buffer,
            vertex_shader_module,
            fragment_shader_module,
            vertex_input_buffer_memory,
            index_buffer_memory,
            resize_dependent_component_rebuild_needed: false,
        }
    }

    pub fn resize(&mut self) {}

    pub fn draw_frame(&mut self) {
        if self.resize_dependent_component_rebuild_needed {
            unsafe { self.device.device_wait_idle().unwrap() };
            resize_dependent_components::cleanup_resize_dependent_components(
                &self.device,
                &self.swapchain_loader,
                &self.resize_dependent_components,
            );
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
        };
        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .image_view(
                self.resize_dependent_components
                    .swapchain_components
                    .present_image_views[present_index as usize],
            );

        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
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
                                .present_images[present_index as usize],
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
                        &[self.vertex_input_buffer],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        draw_command_buffer,
                        self.index_buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_draw_indexed(draw_command_buffer, INDICES.len() as u32, 1, 0, 0, 1);
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
                                .present_images[present_index as usize],
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
        let image_indices = [present_index];
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

#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 4],
    color: [f32; 4],
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
const INDICES: [u32; 3] = [0, 1, 2];

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            graphics_pipeline_components::cleanup_graphics_pipeline_components(
                &self.device,
                &self.graphics_pipeline_components,
            );
            self.device
                .destroy_shader_module(self.vertex_shader_module, None);
            self.device
                .destroy_shader_module(self.fragment_shader_module, None);
            self.device
                .free_memory(self.vertex_input_buffer_memory, None);
            self.device.destroy_buffer(self.vertex_input_buffer, None);
            self.device.free_memory(self.index_buffer_memory, None);
            self.device.destroy_buffer(self.index_buffer, None);
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_commands_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_commands_reuse_fence, None);
            resize_dependent_components::cleanup_resize_dependent_components(
                &self.device,
                &self.swapchain_loader,
                &self.resize_dependent_components,
            );
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_callback, None);
            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
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
