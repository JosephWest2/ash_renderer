use std::{
    borrow::Cow,
    ffi::{c_char, CStr},
    sync::Arc,
};

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk::{self, PhysicalDeviceType},
};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowAttributes,
};

pub struct TestRenderer {
    window: Arc<winit::window::Window>,
    pub instance: ash::Instance,
    pub device: ash::Device,
    entry: ash::Entry,
}

impl TestRenderer {
    pub fn new(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .expect("Failed to create winit window"),
        );

        let entry = unsafe { ash::Entry::load().unwrap() };

        let mut extension_names =
            ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())
                .unwrap()
                .to_vec();
        extension_names.push(ash::ext::debug_utils::NAME.as_ptr());

        let application_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_3);

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&application_info)
            .enabled_extension_names(&extension_names);

        let instance = unsafe { entry.create_instance(&instance_create_info, None).unwrap() };

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

        unsafe { device.destroy_device(None) };
        eprintln!("DESTROYED");

        Self {
            window,
            instance,
            device,
            entry,
        }
    }
}
