use ash::version::DeviceV1_0;
use ash::vk;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::TextureInner;
use crate::imp::{extent_3d, has_zero_or_one_bits, DeviceInner};
use crate::{Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags};

use ash::vk::MemoryPropertyFlags;
use parking_lot::Mutex;
use std::sync::Arc;
use vk_mem::{AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

fn read_only_texture_usage() -> TextureUsageFlags {
    TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::SAMPLED | TextureUsageFlags::PRESENT
}

fn writable_texture_usages() -> TextureUsageFlags {
    TextureUsageFlags::TRANSFER_DST | TextureUsageFlags::STORAGE | TextureUsageFlags::OUTPUT_ATTACHMENT
}

pub fn memory_usage(_usage: TextureUsageFlags) -> MemoryUsage {
    MemoryUsage::GpuOnly
}

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

pub fn image_type(dimension: TextureDimension) -> vk::ImageType {
    // TODO: arrays?
    match dimension {
        TextureDimension::Texture1D => vk::ImageType::TYPE_1D,
        TextureDimension::Texture2D => vk::ImageType::TYPE_2D,
        TextureDimension::Texture3D => vk::ImageType::TYPE_3D,
    }
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
        flags
    } else {
        vk::ImageAspectFlags::COLOR
    }
}

pub fn pipeline_stage(usage: TextureUsageFlags, format: TextureFormat) -> vk::PipelineStageFlags {
    const NONE: TextureUsageFlags = TextureUsageFlags::NONE;

    let mut flags = vk::PipelineStageFlags::empty();

    if usage == TextureUsageFlags::NONE {
        return vk::PipelineStageFlags::TOP_OF_PIPE;
    }

    if usage.contains(TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::TRANSFER_DST) {
        flags |= vk::PipelineStageFlags::TRANSFER;
    }

    if usage.contains(TextureUsageFlags::SAMPLED | TextureUsageFlags::STORAGE) {
        flags |= vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
    }

    if usage.contains(TextureUsageFlags::OUTPUT_ATTACHMENT) {
        if is_depth_or_stencil(format) {
            flags |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
        // TODO: Depth write? FRAGMENT_SHADER stage ?
        } else {
            flags |= vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        }
    }

    if usage.contains(TextureUsageFlags::PRESENT) {
        // Dawn uses BOTTOM_OF_PIPE but notes that TOP_OF_PIPE has potential to block less
        flags |= vk::PipelineStageFlags::BOTTOM_OF_PIPE
    }

    flags
}

pub fn access_flags(usage: TextureUsageFlags, format: TextureFormat) -> vk::AccessFlags {
    let mut flags = vk::AccessFlags::empty();

    if usage.contains(TextureUsageFlags::TRANSFER_SRC) {
        flags |= vk::AccessFlags::TRANSFER_READ;
    }

    if usage.contains(TextureUsageFlags::TRANSFER_DST) {
        flags |= vk::AccessFlags::TRANSFER_WRITE;
    }

    if usage.contains(TextureUsageFlags::SAMPLED) {
        flags |= vk::AccessFlags::SHADER_READ;
    }

    if usage.contains(TextureUsageFlags::STORAGE) {
        flags |= vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;
    }

    if usage.contains(TextureUsageFlags::OUTPUT_ATTACHMENT) {
        if is_depth_or_stencil(format) {
            flags |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        } else {
            flags |= vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        }
    }

    if usage.contains(TextureUsageFlags::PRESENT) {
        flags |= vk::AccessFlags::empty();
    }

    flags
}

pub fn image_layout(usage: TextureUsageFlags, format: TextureFormat) -> vk::ImageLayout {
    if usage == TextureUsageFlags::NONE {
        return vk::ImageLayout::UNDEFINED;
    }

    if !has_zero_or_one_bits(usage.bits()) {
        return vk::ImageLayout::GENERAL;
    }

    // Only a single usage flag is set

    match usage {
        TextureUsageFlags::TRANSFER_DST => vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        TextureUsageFlags::SAMPLED => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::STORAGE => vk::ImageLayout::GENERAL,
        TextureUsageFlags::OUTPUT_ATTACHMENT => {
            if is_depth_or_stencil(format) {
                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
            } else {
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
            }
        }
        TextureUsageFlags::PRESENT => vk::ImageLayout::PRESENT_SRC_KHR,
        _ => unreachable!(),
    }
}

impl TextureInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: TextureDescriptor) -> Result<TextureInner, vk::Result> {
        let flags = if descriptor.array_layer_count >= 6 && descriptor.size.width == descriptor.size.height {
            vk::ImageCreateFlags::CUBE_COMPATIBLE
        } else {
            vk::ImageCreateFlags::empty()
        };

        let create_info = vk::ImageCreateInfo {
            flags,
            image_type: image_type(descriptor.dimension),
            format: image_format(descriptor.format),
            extent: extent_3d(descriptor.size),
            mip_levels: descriptor.mip_level_count,
            array_layers: descriptor.array_layer_count,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: image_usage(descriptor.usage, descriptor.format),
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            ..Default::default()
        };

        let mut state = device.state.lock();
        let allocator = state.allocator_mut();
        let allocation_create_info = AllocationCreateInfo {
            pool: None,
            user_data: None,
            required_flags: MemoryPropertyFlags::empty(),
            preferred_flags: MemoryPropertyFlags::empty(),
            flags: AllocationCreateFlags::NONE,
            memory_type_bits: 0,
            usage: memory_usage(descriptor.usage),
        };

        log::trace!(
            "image create_info: {:?}, allocation_create_info: {:?}",
            create_info,
            allocation_create_info
        );

        let result = allocator.create_image(&create_info, &allocation_create_info);

        // VMA does some basic pre-checks on the input, but doesn't provide any info around why the
        // input was invalid and just returns `VK_ERROR_VALIDAITON_FAILED_EXT` without triggering
        // any validation messages (it doesn't call `vkCreateImage` if it's pre-checks fail).
        // So, in the event of a validation error, we'll create a dummy image that triggers the
        // validation message (so we can see what exactly the problem was) and then destroy it
        // immediately.
        if let Err(ref e) = &result {
            match e.kind() {
                &vk_mem::ErrorKind::Vulkan(vk::Result::ERROR_VALIDATION_FAILED_EXT) => unsafe {
                    let dummy = device.raw.create_image(&create_info, None)?;
                    device.raw.destroy_image(dummy, None);
                    return Err(vk::Result::ERROR_VALIDATION_FAILED_EXT);
                },
                _ => {}
            }
        }

        let (image, allocation, allocation_info) = result.expect("failed to create image"); // TODO

        log::trace!("created image: {:?}, allocation_info: {:?}", image, allocation_info);

        Ok(TextureInner {
            handle: image,
            device: device.clone(),
            allocation: Some(allocation),
            allocation_info: Some(allocation_info),
            descriptor,
            last_usage: Mutex::new(TextureUsageFlags::NONE),
        })
    }

    pub fn transition_usage(
        &self,
        command_buffer: vk::CommandBuffer,
        usage: TextureUsageFlags,
    ) -> Result<(), vk::Result> {
        let mut last_usage = self.last_usage.lock();
        let format = self.descriptor.format;

        let last_read_only = (*last_usage & read_only_texture_usage()) == *last_usage;
        if last_read_only && *last_usage == usage {
            return Ok(());
        }

        let src_stage_mask = pipeline_stage(*last_usage, format);
        let dst_stage_mask = pipeline_stage(*last_usage, format);

        let src_access_mask = access_flags(*last_usage, format);
        let dst_access_mask = access_flags(usage, format);

        let old_layout = image_layout(*last_usage, format);
        let new_layout = image_layout(usage, format);

        let image = self.handle;

        let aspect_mask = aspect_mask(format);
        let base_mip_level = 0;
        let level_count = self.descriptor.mip_level_count;
        let base_array_layer = 0;
        let layer_count = self.descriptor.array_layer_count;

        let image_memory_barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(src_access_mask)
            .dst_access_mask(dst_access_mask)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level,
                level_count,
                base_array_layer,
                layer_count,
            });

        let dependency_flags = vk::DependencyFlags::empty();
        let memory_barriers = &[];
        let buffer_memory_barriers = &[];
        let image_memory_barriers = &[*image_memory_barrier];

        unsafe {
            self.device.raw.cmd_pipeline_barrier(
                command_buffer,
                src_stage_mask,
                dst_stage_mask,
                dependency_flags,
                memory_barriers,
                buffer_memory_barriers,
                image_memory_barriers,
            );
        }

        *last_usage = usage;

        Ok(())
    }
}

impl Into<Texture> for TextureInner {
    fn into(self) -> Texture {
        Texture { inner: Arc::new(self) }
    }
}

impl Drop for TextureInner {
    fn drop(&mut self) {
        if let Some(allocation) = self.allocation.as_ref() {
            let mut state = self.device.state.lock();
            let serial = state.get_next_pending_serial();
            state
                .get_fenced_deleter()
                .delete_when_unused((self.handle, allocation.clone()), serial);
        }
    }
}
