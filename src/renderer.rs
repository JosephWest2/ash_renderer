use std::{
    borrow::Cow,
    ffi::{c_char, CStr},
    mem::offset_of,
};

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk::{self, Handle, PhysicalDeviceType},
    Instance,
};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowAttributes,
};

use crate::shaders;

pub struct Renderer {
    instance: Instance,
    device: ash::Device,
    surface_loader: surface::Instance,
    swapchain_loader: swapchain::Device,
    debug_utils_loader: debug_utils::Instance,
    debug_callback: vk::DebugUtilsMessengerEXT,

    physical_device: vk::PhysicalDevice,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    queue_family_index: u32,
    present_queue: vk::Queue,

    pub window: winit::window::Window,
    surface: vk::SurfaceKHR,
    surface_format: vk::SurfaceFormatKHR,
    surface_resolution: vk::Extent2D,

    swapchain: vk::SwapchainKHR,
    present_images: Vec<vk::Image>,
    present_image_views: Vec<vk::ImageView>,

    pool: vk::CommandPool,
    draw_command_buffer: vk::CommandBuffer,
    setup_command_buffer: vk::CommandBuffer,

    depth_image: vk::Image,
    depth_image_view: vk::ImageView,
    depth_image_memory: vk::DeviceMemory,

    present_complete_semaphore: vk::Semaphore,
    rendering_complete_semaphore: vk::Semaphore,

    draw_commands_reuse_fence: vk::Fence,
    setup_commands_reuse_fence: vk::Fence,

    graphics_pipeline: vk::Pipeline,
    scissors: [vk::Rect2D; 1],
    vertex_input_buffer: vk::Buffer,
    index_buffer: vk::Buffer,
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

        let surface_format = unsafe {
            surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .unwrap()[0]
        };

        let surface_capabilities = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .unwrap()
        };

        let mut desired_image_count = surface_capabilities.min_image_count + 1;

        if surface_capabilities.max_image_count > 0
            && desired_image_count > surface_capabilities.max_image_count
        {
            desired_image_count = surface_capabilities.max_image_count;
        }

        let surface_resolution = match surface_capabilities.current_extent.width {
            u32::MAX => vk::Extent2D {
                width: window.inner_size().width,
                height: window.inner_size().height,
            },
            _ => surface_capabilities.current_extent,
        };

        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };

        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .unwrap()
        };

        let present_mode = present_modes
            .iter()
            .cloned()
            .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);
        let swapchain_loader = swapchain::Device::new(&instance, &device);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(desired_image_count)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(surface_resolution)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1);

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap()
        };

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

        let present_images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() };

        let present_image_views: Vec<vk::ImageView> = present_images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::R,
                        g: vk::ComponentSwizzle::G,
                        b: vk::ComponentSwizzle::B,
                        a: vk::ComponentSwizzle::A,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);
                unsafe { device.create_image_view(&create_view_info, None).unwrap() }
            })
            .collect();

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let depth_image_create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D16_UNORM)
            .extent(surface_resolution.into())
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_4)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let depth_image = unsafe { device.create_image(&depth_image_create_info, None).unwrap() };

        let depth_image_memory_reqs = unsafe { device.get_image_memory_requirements(depth_image) };

        let depth_image_memory_index = find_memorytype_index(
            &depth_image_memory_reqs,
            &device_memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .expect("Cannot find suitable memory index for depth image");

        let depth_image_allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(depth_image_memory_reqs.size)
            .memory_type_index(depth_image_memory_index);

        let depth_image_memory = unsafe {
            device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap()
        };

        unsafe {
            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("Failed to bind depth image memory")
        };

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

        record_submit_commandbuffer(
            &device,
            setup_command_buffer,
            setup_commands_reuse_fence,
            present_queue,
            &[],
            &[],
            &[],
            |device, setup_command_buffer| {
                let layout_transition_barrier = vk::ImageMemoryBarrier::default()
                    .image(depth_image)
                    .dst_access_mask(
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .layer_count(1)
                            .level_count(1),
                    );
                unsafe {
                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barrier],
                    )
                };
            },
        );

        let depth_image_view_info = vk::ImageViewCreateInfo::default()
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .level_count(1)
                    .layer_count(1),
            )
            .image(depth_image)
            .format(depth_image_create_info.format)
            .view_type(vk::ImageViewType::TYPE_2D);

        let depth_image_view = unsafe {
            device
                .create_image_view(&depth_image_view_info, None)
                .unwrap()
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

        let noop_stencil_state = vk::StencilOpState::default()
            .fail_op(vk::StencilOp::KEEP)
            .pass_op(vk::StencilOp::KEEP)
            .depth_fail_op(vk::StencilOp::KEEP)
            .compare_op(vk::CompareOp::ALWAYS);

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .front(noop_stencil_state)
            .back(noop_stencil_state)
            .max_depth_bounds(1.0);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::SRC_COLOR)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_DST_COLOR)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states);

        let layout_create_info = vk::PipelineLayoutCreateInfo::default();
        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&layout_create_info, None)
                .expect("Failed to create pipeline layout")
        };

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0)
            .polygon_mode(vk::PolygonMode::FILL);

        let scissors = [surface_resolution.into()];
        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: surface_resolution.width as f32,
            height: surface_resolution.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .scissors(&scissors)
            .viewports(&viewports);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)];

        let vertex_input_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: offset_of!(Vertex, position) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: offset_of!(Vertex, color) as u32,
            },
        ];

        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_attribute_descriptions(&vertex_input_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_input_binding_descriptions);

        let vertex_input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let color_attachment_formats = &[surface_format.format];
        let mut pipeline_rendering_create_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(color_attachment_formats)
            .depth_attachment_format(depth_image_view_info.format);

        let graphics_pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
            .push_next(&mut pipeline_rendering_create_info)
            .stages(&pipeline_shader_stage_infos)
            .dynamic_state(&dynamic_state_info)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .layout(pipeline_layout)
            .rasterization_state(&rasterization_state)
            .viewport_state(&viewport_state)
            .input_assembly_state(&vertex_input_assembly_state)
            .vertex_input_state(&vertex_input_state)
            .depth_stencil_state(&depth_stencil_state);

        let graphics_pipelines = unsafe {
            device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[graphics_pipeline_create_info],
                    None,
                )
                .expect("Failed to create graphics pipelines")
        };

        let graphics_pipeline = graphics_pipelines[0];

        Self {
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
            surface_format,
            surface_resolution,
            swapchain,
            present_images,
            present_image_views,
            pool,
            draw_command_buffer,
            setup_command_buffer,
            depth_image,
            depth_image_view,
            depth_image_memory,
            present_complete_semaphore,
            rendering_complete_semaphore,
            draw_commands_reuse_fence,
            setup_commands_reuse_fence,
            graphics_pipeline,
            scissors,
            vertex_input_buffer,
            index_buffer,
        }
    }

    pub fn draw_frame(&mut self) {
        let (present_index, _) = unsafe {
            self.swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    self.present_complete_semaphore,
                    vk::Fence::null(),
                )
                .unwrap()
        };

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .image_view(self.present_image_views[present_index as usize]);

        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .image_view(self.depth_image_view);

        let color_attachments = &[color_attachment];
        let rendering_info = vk::RenderingInfo::default()
            .depth_attachment(&depth_attachment)
            .color_attachments(color_attachments)
            .layer_count(1);

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
                    device.cmd_begin_rendering(self.draw_command_buffer, &rendering_info);
                    device.cmd_set_scissor(draw_command_buffer, 0, &self.scissors);
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
                };
            },
        );

        let wait_semaphores = [self.rendering_complete_semaphore];
        let swapchains = [self.swapchain];
        let image_indices = [present_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
                .expect("Swapchain loader failed to present")
        };
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
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_commands_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_commands_reuse_fence, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
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
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

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
