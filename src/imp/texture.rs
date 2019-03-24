use ash::vk;
use ash::version::DeviceV1_0;

use crate::imp::TextureInner;
use crate::imp::{has_zero_or_one_bits};
use crate::{TextureFormat, TextureUsageFlags};

fn read_only_texture_usage() -> TextureUsageFlags {
    TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::SAMPLED | TextureUsageFlags::PRESENT
}

fn writable_texture_usages() -> TextureUsageFlags {
    TextureUsageFlags::TRANSFER_DST | TextureUsageFlags::STORAGE | TextureUsageFlags::OUTPUT_ATTACHMENT
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

pub fn pipeline_stage(usage: TextureUsageFlags, format: TextureFormat) -> vk::PipelineStageFlags {
    const NONE: TextureUsageFlags = TextureUsageFlags::NONE;

    let mut flags = vk::PipelineStageFlags::empty();

    if usage == TextureUsageFlags::NONE {
        return vk::PipelineStageFlags::TOP_OF_PIPE;
    }

    if usage & (TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::TRANSFER_DST) > NONE {
        flags |= vk::PipelineStageFlags::TRANSFER;
    }

    if usage & (TextureUsageFlags::SAMPLED | TextureUsageFlags::STORAGE) > NONE {
        flags |= vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
    }

    if usage & TextureUsageFlags::OUTPUT_ATTACHMENT > NONE {
        if is_depth_or_stencil(format) {
            flags |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
        // TODO: Depth write? FRAGMENT_SHADER stage ?
        } else {
            flags |= vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        }
    }

    if usage & TextureUsageFlags::PRESENT > NONE {
        // Dawn uses BOTTOM_OF_PIPE but notes that TOP_OF_PIPE has potential to block less
        flags |= vk::PipelineStageFlags::BOTTOM_OF_PIPE
    }

    flags
}

pub fn access_flags(usage: TextureUsageFlags, format: TextureFormat) -> vk::AccessFlags {
    const NONE: TextureUsageFlags = TextureUsageFlags::NONE;

    let mut flags = vk::AccessFlags::empty();

    if usage & TextureUsageFlags::TRANSFER_SRC > NONE {
        flags |= vk::AccessFlags::TRANSFER_READ;
    }

    if usage & TextureUsageFlags::TRANSFER_DST > NONE {
        flags |= vk::AccessFlags::TRANSFER_WRITE;
    }

    if usage & TextureUsageFlags::SAMPLED > NONE {
        flags |= vk::AccessFlags::SHADER_READ;
    }

    if usage & TextureUsageFlags::STORAGE > NONE {
        flags |= vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;
    }

    if usage & TextureUsageFlags::OUTPUT_ATTACHMENT > NONE {
        if is_depth_or_stencil(format) {
            flags |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        } else {
            flags |= vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        }
    }

    if usage & TextureUsageFlags::PRESENT > NONE {
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
                image_memory_barriers
            );
        }

        *last_usage = usage;

        Ok(())
    }
}
