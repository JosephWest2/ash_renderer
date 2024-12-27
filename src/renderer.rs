use std::ffi::{c_char, CStr};

use ash::{
    khr,
    vk::{self, ClearValue, ImageSubresourceRange},
};
use command_buffer_components::{record_submit_commandbuffer, CommandBufferComponents};
use descriptor_components::{DescriptorComponents, UniformBuffers};
use graphics_pipeline_components::GraphicsPipelineComponents;
use index_buffer_components::{IndexBufferComponents, INDICES};
use resize_dependent_components::ResizeDependentComponents;
use semaphore_components::SemaphoreComponents;
use vertex_buffer_components::{VertexBufferComponents, VERTICES};
use winit::{
    event_loop::ActiveEventLoop,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowAttributes,
};

mod buffer;
pub mod camera;
mod command_buffer_components;
mod debug_components;
mod descriptor_components;
mod graphics_pipeline_components;
mod index_buffer_components;
mod resize_dependent_components;
mod select_physical_device;
mod semaphore_components;
mod shaders;
mod textures;
mod vertex_buffer_components;

pub struct UserSettings {
    pub preferred_physical_device_id: Option<u32>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            preferred_physical_device_id: None,
        }
    }
}

// Assume all unused variables are required for persistence
#[allow(dead_code)]
pub struct Renderer {
    sic: SettingsIndependentComponents,
    sdc: SettingsDependentComponents,
    pub resize_dependent_component_rebuild_needed: bool,
}

impl Renderer {
    pub fn new(event_loop: &ActiveEventLoop, user_settings: &UserSettings) -> Self {
        let sic = SettingsIndependentComponents::new(event_loop);
        let sdc = SettingsDependentComponents::new(&sic, user_settings);

        Self {
            sdc,
            sic,
            resize_dependent_component_rebuild_needed: false,
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.sdc.cleanup();
        self.sic.cleanup();
    }
}

#[allow(dead_code)]
struct SettingsIndependentComponents {
    entry: ash::Entry,
    instance: ash::Instance,
    #[cfg(debug_assertions)]
    debug_components: debug_components::DebugComponents,
    window: winit::window::Window,
    surface: vk::SurfaceKHR,
    surface_loader: khr::surface::Instance,
}
impl SettingsIndependentComponents {
    pub fn new(event_loop: &ActiveEventLoop) -> SettingsIndependentComponents {
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

        let surface_loader = khr::surface::Instance::new(&entry, &instance);

        SettingsIndependentComponents {
            window,
            entry,
            instance,
            #[cfg(debug_assertions)]
            debug_components,
            surface,
            surface_loader,
        }
    }
    pub fn cleanup(&mut self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
            #[cfg(debug_assertions)]
            self.debug_components.cleanup();
            self.instance.destroy_instance(None);
        }
    }
}

#[allow(dead_code)]
struct SettingsDependentComponents {
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    graphics_queue: vk::Queue,
    transfer_queue: Option<vk::Queue>,
    swapchain_loader: khr::swapchain::Device,
    physical_device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    semaphore_components: SemaphoreComponents,
    command_buffer_components: CommandBufferComponents,
    vertex_buffer_components: VertexBufferComponents,
    index_buffer_components: IndexBufferComponents,
    shaders: shaders::Shaders,
    rdc: ResizeDependentComponents,
    descriptor_components: DescriptorComponents,
    graphics_pipeline_components: GraphicsPipelineComponents,
}
impl SettingsDependentComponents {
    fn new(
        settings_independent_components: &SettingsIndependentComponents,
        user_settings: &UserSettings,
    ) -> SettingsDependentComponents {
        let physical_device_selection = select_physical_device(
            &settings_independent_components.instance,
            user_settings.preferred_physical_device_id,
        );
        let graphics_queue_family_index =
            physical_device_selection.graphics_queue_family_index as u32;
        let transfer_queue_family_index = physical_device_selection.transfer_queue_family_index;
        let physical_device = physical_device_selection.physical_device;

        let device_extension_names_raw = [khr::swapchain::NAME.as_ptr()];

        let features = vk::PhysicalDeviceFeatures::default().shader_clip_distance(true);

        let mut dynamic_rendering_features =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);

        let priorities = [1.0];

        let graphics_queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_queue_family_index)
            .queue_priorities(&priorities);
        let queue_infos = match transfer_queue_family_index {
            Some(i) => {
                let transfer_queue_create_info = vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(i as u32)
                    .queue_priorities(&priorities);
                vec![graphics_queue_create_info, transfer_queue_create_info]
            }
            None => vec![graphics_queue_create_info],
        };

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extension_names_raw)
            .push_next(&mut dynamic_rendering_features)
            .enabled_features(&features);

        let device = unsafe {
            settings_independent_components
                .instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap()
        };

        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_family_index, 0) };

        let transfer_queue = match transfer_queue_family_index {
            Some(i) => Some(unsafe { device.get_device_queue(i as u32, 0) }),
            None => None,
        };

        let swapchain_loader =
            khr::swapchain::Device::new(&settings_independent_components.instance, &device);

        let physical_device_memory_properties = unsafe {
            settings_independent_components
                .instance
                .get_physical_device_memory_properties(physical_device)
        };

        let semaphore_components = SemaphoreComponents::new(&device);

        let command_buffer_components =
            CommandBufferComponents::new(graphics_queue_family_index, &device);

        let mut index_buffer_components =
            IndexBufferComponents::new_unintiailized(&device, &physical_device_memory_properties);
        index_buffer_components.update_indices(
            &device,
            &INDICES,
            command_buffer_components.setup_command_buffer,
            command_buffer_components.setup_commands_reuse_fence,
            graphics_queue,
        );

        let mut vertex_buffer_components =
            VertexBufferComponents::new_unintialized(&device, &physical_device_memory_properties);
        vertex_buffer_components.update_vertices(
            &device,
            &VERTICES,
            command_buffer_components.setup_command_buffer,
            command_buffer_components.setup_commands_reuse_fence,
            graphics_queue,
        );

        let shaders = shaders::Shaders::new(&device);

        let rdc = resize_dependent_components::ResizeDependentComponents::new(
            &device,
            &settings_independent_components.window,
            settings_independent_components.surface,
            &settings_independent_components.surface_loader,
            &swapchain_loader,
            physical_device,
            command_buffer_components.setup_command_buffer,
            command_buffer_components.setup_commands_reuse_fence,
            &physical_device_memory_properties,
            graphics_queue,
        );

        let descriptor_components = DescriptorComponents::new(
            &device,
            &physical_device_memory_properties,
            rdc.swapchain_components.present_images.len() as u32,
        );

        let graphics_pipeline_components = GraphicsPipelineComponents::new(
            &device,
            &rdc.swapchain_components.surface_format,
            &shaders.shader_stage_infos(),
            &[descriptor_components.uniform_buffer_descriptor_set_layout],
            &rdc.scissors,
            &rdc.viewports,
        );

        SettingsDependentComponents {
            physical_device,
            device,
            graphics_queue,
            transfer_queue,
            swapchain_loader,
            physical_device_memory_properties,
            shaders,
            rdc,
            command_buffer_components,
            semaphore_components,
            index_buffer_components,
            vertex_buffer_components,
            descriptor_components,
            graphics_pipeline_components,
        }
    }

    pub fn cleanup(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.graphics_pipeline_components.cleanup(&self.device);
            self.shaders.cleanup(&self.device);
            self.index_buffer_components.cleanup(&self.device);
            self.vertex_buffer_components.cleanup(&self.device);
            self.descriptor_components.cleanup(&self.device);
            self.semaphore_components.cleanup(&self.device);
            self.command_buffer_components.cleanup(&self.device);
            self.rdc.cleanup(&self.device, &self.swapchain_loader);
            self.device.destroy_device(None);
        }
    }
}

#[derive(Clone, Copy)]
struct PhysicalDeviceSelection {
    pub graphics_queue_family_index: usize,
    pub transfer_queue_family_index: Option<usize>,
    pub physical_device: vk::PhysicalDevice,
}
fn select_physical_device(
    instance: &ash::Instance,
    preferred_physical_device_id: Option<u32>,
) -> PhysicalDeviceSelection {
    let physical_devices = unsafe { instance.enumerate_physical_devices().unwrap() };
    let mut qualified_devices = Vec::new();
    for physical_device in physical_devices.iter() {
        let properties =
            unsafe { instance.get_physical_device_queue_family_properties(*physical_device) };
        let mut graphics_queue_family_index = None;
        let mut transfer_queue_family_index = None;
        for i in 0..properties.len() {
            let property = properties[i];
            if property.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics_queue_family_index = Some(i);
            } else if property.queue_flags.contains(vk::QueueFlags::TRANSFER) {
                transfer_queue_family_index = Some(i);
            }
        }
        if graphics_queue_family_index.is_some() {
            qualified_devices.push(PhysicalDeviceSelection {
                graphics_queue_family_index: graphics_queue_family_index.unwrap(),
                transfer_queue_family_index,
                physical_device: *physical_device,
            })
        }
    }
    if qualified_devices.is_empty() {
        panic!("No supported physical device found");
    }
    let mut selection_index = 0;
    let mut scores = vec![0; qualified_devices.len()];
    for i in 0..qualified_devices.len() {
        let physical_device = qualified_devices[i].physical_device;
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        if preferred_physical_device_id.is_some_and(|id| id == properties.device_id) {
            return qualified_devices[i];
        }
        let mut score = 0;
        match properties.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => score += 1000,
            vk::PhysicalDeviceType::INTEGRATED_GPU => score += 100,
            vk::PhysicalDeviceType::VIRTUAL_GPU => score += 10,
            vk::PhysicalDeviceType::CPU => score += 1,
            _ => (),
        }
        score += properties.limits.max_image_dimension2_d;
        scores[i] = score;
    }
    for i in 0..scores.len() {
        if scores[i] >= scores[selection_index] {
            selection_index = i;
        }
    }
    qualified_devices[selection_index]
}
impl Renderer {
    pub fn draw_frame(&mut self, camera: &camera::Camera) {
        if self.resize_dependent_component_rebuild_needed {
            self.handle_window_resize();
            self.resize_dependent_component_rebuild_needed = false;
        }

        unsafe {
            self.sdc
                .device
                .wait_for_fences(
                    &[self.sdc.command_buffer_components.draw_commands_reuse_fence],
                    true,
                    u64::MAX,
                )
                .unwrap()
        };

        let next_image_result = unsafe {
            self.sdc.swapchain_loader.acquire_next_image(
                self.sdc.rdc.swapchain_components.swapchain,
                u64::MAX,
                self.sdc.semaphore_components.present_complete_semaphore,
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

        self.sdc.descriptor_components.uniform_buffers[present_index].write_data_direct(
            &self.sdc.device,
            &[UniformBuffers {
                model_matrix: camera::MODEL_MATRIX,
                view_matrix: camera.view_matrix(),
                projection_matrix: camera
                    .projection_matrix(self.sdc.rdc.swapchain_components.get_aspect_ratio()),
            }],
        );

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .image_view(self.sdc.rdc.swapchain_components.present_image_views[present_index]);

        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .clear_value(ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            })
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .image_view(self.sdc.rdc.depth_image_components.depth_image_view);

        let color_attachments = &[color_attachment];
        let rendering_info = vk::RenderingInfo::default()
            .depth_attachment(&depth_attachment)
            .color_attachments(color_attachments)
            .layer_count(1)
            .render_area(self.sdc.rdc.swapchain_components.surface_resolution.into());

        record_submit_commandbuffer(
            &self.sdc.device,
            self.sdc.graphics_queue,
            self.sdc.command_buffer_components.draw_command_buffer,
            self.sdc.command_buffer_components.draw_commands_reuse_fence,
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
            &[self.sdc.semaphore_components.present_complete_semaphore],
            &[self.sdc.semaphore_components.rendering_complete_semaphore],
            |device, draw_command_buffer| {
                unsafe {
                    // dynamic rendering image layout transiton. see https://lesleylai.info/en/vk-khr-dynamic-rendering/
                    let image_memory_barrier = vk::ImageMemoryBarrier::default()
                        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .image(self.sdc.rdc.swapchain_components.present_images[present_index])
                        .subresource_range(
                            ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(1),
                        );
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
                    device.cmd_begin_rendering(draw_command_buffer, &rendering_info);
                    device.cmd_bind_pipeline(
                        draw_command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.sdc.graphics_pipeline_components.graphics_pipelines
                            [self.sdc.graphics_pipeline_components.render_pipeline_index],
                    );
                    device.cmd_set_scissor(draw_command_buffer, 0, &self.sdc.rdc.scissors);
                    device.cmd_set_viewport(draw_command_buffer, 0, &self.sdc.rdc.viewports);
                    device.cmd_bind_vertex_buffers(
                        draw_command_buffer,
                        0,
                        &[self.sdc.vertex_buffer_components.vertex_buffer.buffer],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        draw_command_buffer,
                        self.sdc.index_buffer_components.index_buffer.buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_bind_descriptor_sets(
                        draw_command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.sdc.graphics_pipeline_components.render_pipeline_layout,
                        0,
                        &[self
                            .sdc
                            .descriptor_components
                            .uniform_buffer_descriptor_sets[present_index]],
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
                    let image_memory_barrier = vk::ImageMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(self.sdc.rdc.swapchain_components.present_images[present_index])
                        .subresource_range(
                            ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(1),
                        );
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

        let wait_semaphores = [self.sdc.semaphore_components.rendering_complete_semaphore];

        let swapchains = [self.sdc.rdc.swapchain_components.swapchain];

        let image_indices = [present_index as u32];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let present_result = unsafe {
            self.sdc
                .swapchain_loader
                .queue_present(self.sdc.graphics_queue, &present_info)
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

impl Renderer {
    fn handle_window_resize(&mut self) {
        unsafe { self.sdc.device.device_wait_idle().unwrap() };
        self.sdc
            .rdc
            .cleanup(&self.sdc.device, &self.sdc.swapchain_loader);
        self.sdc.rdc = ResizeDependentComponents::new(
            &self.sdc.device,
            &self.sic.window,
            self.sic.surface,
            &self.sic.surface_loader,
            &self.sdc.swapchain_loader,
            self.sdc.physical_device,
            self.sdc.command_buffer_components.setup_command_buffer,
            self.sdc
                .command_buffer_components
                .setup_commands_reuse_fence,
            &self.sdc.physical_device_memory_properties,
            self.sdc.graphics_queue,
        )
    }
    pub fn request_redraw(&self) {
        self.sic.window.request_redraw();
    }
    pub fn update_user_settings(&mut self, new_user_settings: &UserSettings) {
        unsafe { self.sdc.device.device_wait_idle().unwrap() };
        self.sdc = SettingsDependentComponents::new(&self.sic, new_user_settings);
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
