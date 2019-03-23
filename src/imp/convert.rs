use crate::{Extent3D, TextureFormat, TextureUsageFlags};
use ash::vk;

pub fn is_depth(format: TextureFormat) -> bool {
    match format {
        TextureFormat::D32FloatS8Uint => true,
        _ => false,
    }
}

pub fn is_stencil(format: TextureFormat) -> bool {
    match format {
        TextureFormat::D32FloatS8Uint => true,
        _ => false,
    }
}

pub fn is_depth_or_stencil(format: TextureFormat) -> bool {
    is_depth(format) || is_stencil(format)
}

pub fn image_usage(usage: TextureUsageFlags, format: TextureFormat) -> vk::ImageUsageFlags {
    let mut flags = vk::ImageUsageFlags::empty();

    if usage.contains(TextureUsageFlags::TRANSFER_SRC) {
        flags |= vk::ImageUsageFlags::TRANSFER_SRC;
    }

    if usage.contains(TextureUsageFlags::TRANSFER_DST) {
        flags |= vk::ImageUsageFlags::TRANSFER_DST;
    }

    if usage.contains(TextureUsageFlags::SAMPLED) {
        flags |= vk::ImageUsageFlags::SAMPLED;
    }

    if usage.contains(TextureUsageFlags::STORAGE) {
        flags |= vk::ImageUsageFlags::STORAGE;
    }

    if usage.contains(TextureUsageFlags::OUTPUT_ATTACHMENT) {
        if is_depth_or_stencil(format) {
            flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
        } else {
            flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
        }
    }

    flags
}

pub fn image_format(format: TextureFormat) -> vk::Format {
    match format {
        TextureFormat::B8G8R8A8UnormSRGB => vk::Format::B8G8R8A8_SRGB,
        TextureFormat::R8G8B8A8Unorm => vk::Format::R8G8B8A8_UNORM,
        TextureFormat::R8G8Unorm => vk::Format::R8G8_UNORM,
        TextureFormat::R8G8Uint => vk::Format::R8G8_UINT,
        TextureFormat::R8Unorm => vk::Format::R8_UNORM,
        TextureFormat::R8Uint => vk::Format::R8_UINT,
        TextureFormat::B8G8R8A8Unorm => vk::Format::B8G8R8A8_UNORM,
        TextureFormat::R8G8B8A8Uint => vk::Format::R8G8B8A8_UINT,
        TextureFormat::D32FloatS8Uint => vk::Format::D32_SFLOAT_S8_UINT,
    }
}

#[rustfmt::skip]
pub fn pixel_size(format: TextureFormat) -> u32 {
    match format {
        TextureFormat::R8Unorm |
        TextureFormat::R8Uint
            => 1,
        TextureFormat::R8G8Unorm |
        TextureFormat::R8G8Uint
            => 2,
        TextureFormat::R8G8B8A8Unorm |
        TextureFormat::R8G8B8A8Uint |
        TextureFormat::B8G8R8A8Unorm |
        TextureFormat::B8G8R8A8UnormSRGB
            => 4,
        TextureFormat::D32FloatS8Uint
            => 8
    }
}

pub fn aspect_mask(format: TextureFormat) -> vk::ImageAspectFlags {
    let mut flags = vk::ImageAspectFlags::empty();
    if is_depth(format) {
        flags |= vk::ImageAspectFlags::DEPTH;
    }
    if is_stencil(format) {
        flags |= vk::ImageAspectFlags::STENCIL;
    }
    if !flags.is_empty() {
        return flags;
    } else {
        vk::ImageAspectFlags::COLOR
    }
}

impl Into<vk::Extent3D> for Extent3D {
    fn into(self) -> vk::Extent3D {
        vk::Extent3D {
            width: self.width,
            height: self.height,
            depth: self.depth,
        }
    }
}
