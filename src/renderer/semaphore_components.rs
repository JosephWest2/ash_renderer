use ash::vk;

pub struct SemaphoreComponents {
    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,
}

impl SemaphoreComponents {
    pub fn new(device: &ash::Device) -> SemaphoreComponents {
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

        SemaphoreComponents {
            present_complete_semaphore,
            rendering_complete_semaphore,
        }
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.destroy_semaphore(self.present_complete_semaphore, None);
            device.destroy_semaphore(self.rendering_complete_semaphore, None);
        }
    }
}
