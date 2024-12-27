use ash::vk;
use image::{GenericImageView, ImageReader};

use super::find_memorytype_index;

pub fn create_texture(
    device: &ash::Device,
    physical_device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
) {
    let img = ImageReader::open("../../static/textures/texture.jpg")
        .unwrap()
        .decode()
        .unwrap();
    let dimensions = img.dimensions();
    let extent = vk::Extent3D {
        width: dimensions.0,
        height: dimensions.1,
        depth: 1,
    };
    let image_create_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .extent(extent)
        .mip_levels(1)
        .format(vk::Format::R8G8B8A8_SRGB)
        .tiling(vk::ImageTiling::OPTIMAL)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .samples(vk::SampleCountFlags::TYPE_1)
        .usage(vk::ImageUsageFlags::SAMPLED)
        .array_layers(1);

    let image = unsafe { device.create_image(&image_create_info, None).unwrap() };

    let memory_reqs = unsafe { device.get_image_memory_requirements(image) };

    let memtype_index = find_memorytype_index(
        &memory_reqs,
        physical_device_memory_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .expect("failed to find memtype index");

    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(memory_reqs.size)
        .memory_type_index(memtype_index);

    let memory = unsafe { device.allocate_memory(&allocate_info, None).unwrap() };

    unsafe { device.bind_image_memory(image, memory, 0).unwrap() };
}
