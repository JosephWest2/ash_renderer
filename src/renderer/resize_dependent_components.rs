use ash::{
    khr::{surface, swapchain},
    vk,
};
use depth_image_components::DepthImageComponents;
use swapchain_components::SwapchainComponents;

pub mod depth_image_components;
pub mod swapchain_components;

pub struct ResizeDependentComponents {
    pub swapchain_components: swapchain_components::SwapchainComponents,
    pub depth_image_components: depth_image_components::DepthImageComponents,
    pub scissors: [vk::Rect2D; 1],
    pub viewports: [vk::Viewport; 1],
}

impl ResizeDependentComponents {
    pub fn new(
        device: &ash::Device,
        window: &winit::window::Window,
        surface: &vk::SurfaceKHR,
        surface_loader: &surface::Instance,
        swapchain_loader: &swapchain::Device,
        physical_device: &vk::PhysicalDevice,
        setup_command_buffer: &vk::CommandBuffer,
        setup_commands_reuse_fence: &vk::Fence,
        device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
        present_queue: &vk::Queue,
    ) -> Self {
        let swapchain_components = SwapchainComponents::new(
            device,
            window,
            surface,
            surface_loader,
            swapchain_loader,
            physical_device,
        );

        let depth_image_components = DepthImageComponents::new(
            device,
            device_memory_properties,
            &swapchain_components.surface_resolution,
            setup_command_buffer,
            setup_commands_reuse_fence,
            present_queue,
        );

        let scissors = [swapchain_components.surface_resolution.into()];
        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: swapchain_components.surface_resolution.width as f32,
            height: swapchain_components.surface_resolution.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];

        ResizeDependentComponents {
            swapchain_components,
            depth_image_components,
            scissors,
            viewports,
        }
    }
    pub fn cleanup(&self, device: &ash::Device, swapchain_loader: &swapchain::Device) {
        self.depth_image_components.cleanup(device);
        self.swapchain_components.cleanup(device, swapchain_loader);
    }
    pub fn aspect_ratio(&self) -> f32 {
        self.swapchain_components.surface_resolution.width as f32
            / self.swapchain_components.surface_resolution.height as f32
    }
}
