use ash::version::DeviceV1_0;
use ash::vk;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::util;
use crate::imp::{DeviceInner, TextureInner, TextureViewInner};
use crate::{
    Extent3D, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

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
        TextureDimension::D1 => vk::ImageType::TYPE_1D,
        TextureDimension::D2 => vk::ImageType::TYPE_2D,
        TextureDimension::D3 => vk::ImageType::TYPE_3D,
    }
}

pub fn image_view_type(descriptor: &TextureViewDescriptor) -> vk::ImageViewType {
    // TODO: arrays?
    match descriptor.dimension {
        TextureViewDimension::D1 => vk::ImageViewType::TYPE_1D,
        TextureViewDimension::D2 => vk::ImageViewType::TYPE_2D,
        TextureViewDimension::D3 => vk::ImageViewType::TYPE_3D,
        TextureViewDimension::Cube => vk::ImageViewType::CUBE,
    }
}

pub fn image_usage(usage: TextureUsageFlags, format: TextureFormat) -> vk::ImageUsageFlags {
    let mut flags = vk::ImageUsageFlags::empty();

    if usage.intersects(TextureUsageFlags::TRANSFER_SRC) {
        flags |= vk::ImageUsageFlags::TRANSFER_SRC;
    }

    if usage.intersects(TextureUsageFlags::TRANSFER_DST) {
        flags |= vk::ImageUsageFlags::TRANSFER_DST;
    }

    if usage.intersects(TextureUsageFlags::SAMPLED) {
        flags |= vk::ImageUsageFlags::SAMPLED;
    }

    if usage.intersects(TextureUsageFlags::STORAGE) {
        flags |= vk::ImageUsageFlags::STORAGE;
    }

    if usage.intersects(TextureUsageFlags::OUTPUT_ATTACHMENT) {
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
        TextureFormat::R8Unorm => vk::Format::R8_UNORM,
        TextureFormat::R8UnormSRGB => vk::Format::R8_SRGB,
        TextureFormat::R8Snorm => vk::Format::R8_SNORM,
        TextureFormat::R8Uint => vk::Format::R8_UINT,
        TextureFormat::R8Sint => vk::Format::R8_SINT,

        TextureFormat::R8G8Unorm => vk::Format::R8G8_UNORM,
        TextureFormat::R8G8Uint => vk::Format::R8G8_UINT,

        TextureFormat::R8G8B8A8Unorm => vk::Format::R8G8B8A8_UNORM,
        TextureFormat::R8G8B8A8Uint => vk::Format::R8G8B8A8_UINT,
        TextureFormat::B8G8R8A8Unorm => vk::Format::B8G8R8A8_UNORM,
        TextureFormat::B8G8R8A8UnormSRGB => vk::Format::B8G8R8A8_SRGB,

        TextureFormat::D32Float => vk::Format::D32_SFLOAT,
        TextureFormat::D32FloatS8Uint => vk::Format::D32_SFLOAT_S8_UINT,
    }
}

pub fn texture_format(format: vk::Format) -> TextureFormat {
    match format {
        vk::Format::B8G8R8A8_SRGB => TextureFormat::B8G8R8A8UnormSRGB,
        vk::Format::B8G8R8A8_UNORM => TextureFormat::B8G8R8A8Unorm,
        _ => unimplemented!("todo: missing format conversion: {:?}", format), // TODO
    }
}

#[rustfmt::skip]
pub fn pixel_size(format: TextureFormat) -> u32 {
    match format {
        TextureFormat::R8Unorm |
        TextureFormat::R8UnormSRGB |
        TextureFormat::R8Snorm |
        TextureFormat::R8Uint |
        TextureFormat::R8Sint
        => 1,
        TextureFormat::R8G8Unorm |
        TextureFormat::R8G8Uint
        => 2,
        TextureFormat::R8G8B8A8Unorm |
        TextureFormat::R8G8B8A8Uint |
        TextureFormat::B8G8R8A8Unorm |
        TextureFormat::B8G8R8A8UnormSRGB
        => 4,
        TextureFormat::D32Float
        => 8,
        // TODO: D32FloatS8Uint
        // Dawn has this as "8", but the Vulkan spec states:
        //
        //  VK_FORMAT_D32_SFLOAT_S8_UINT specifies a two-component format that has 32 signed float
        //  bits in the depth component and 8 unsigned integer bits in the stencil component. There
        //  are optionally: 24-bits that are unused.
        //
        // This sounds like 64 bits total?
        TextureFormat::D32FloatS8Uint
        => 16
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

    if usage.intersects(TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::TRANSFER_DST) {
        flags |= vk::PipelineStageFlags::TRANSFER;
    }

    if usage.intersects(TextureUsageFlags::SAMPLED | TextureUsageFlags::STORAGE) {
        flags |= vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
    }

    if usage.intersects(TextureUsageFlags::OUTPUT_ATTACHMENT) {
        if is_depth_or_stencil(format) {
            flags |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
        // TODO: Depth write? FRAGMENT_SHADER stage ?
        } else {
            flags |= vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        }
    }

    if usage.intersects(TextureUsageFlags::PRESENT) {
        // Dawn uses BOTTOM_OF_PIPE but notes that TOP_OF_PIPE has potential to block less
        flags |= vk::PipelineStageFlags::BOTTOM_OF_PIPE
    }

    flags
}

/// https://www.khronos.org/registry/vulkan/specs/1.1-extensions/html/vkspec.html#synchronization-access-types-supported
pub fn access_flags(usage: TextureUsageFlags, format: TextureFormat) -> vk::AccessFlags {
    let mut flags = vk::AccessFlags::empty();

    if usage.intersects(TextureUsageFlags::TRANSFER_SRC) {
        flags |= vk::AccessFlags::TRANSFER_READ;
    }

    if usage.intersects(TextureUsageFlags::TRANSFER_DST) {
        flags |= vk::AccessFlags::TRANSFER_WRITE;
    }

    if usage.intersects(TextureUsageFlags::SAMPLED) {
        flags |= vk::AccessFlags::SHADER_READ;
    }

    if usage.intersects(TextureUsageFlags::STORAGE) {
        flags |= vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;
    }

    if usage.intersects(TextureUsageFlags::OUTPUT_ATTACHMENT) {
        if is_depth_or_stencil(format) {
            flags |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        } else {
            flags |= vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        }
    }

    if usage.intersects(TextureUsageFlags::PRESENT) {
        flags |= vk::AccessFlags::empty();
    }

    flags
}

pub fn image_layout(usage: TextureUsageFlags, format: TextureFormat) -> vk::ImageLayout {
    if usage == TextureUsageFlags::NONE {
        return vk::ImageLayout::UNDEFINED;
    }

    if !util::has_zero_or_one_bits(usage.bits()) {
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

pub fn default_texture_view_descriptor(texture: &TextureInner) -> TextureViewDescriptor {
    let aspect_flags = aspect_mask(texture.descriptor.format);
    let aspect = unsafe { std::mem::transmute(aspect_flags) };

    //    let dimension = if texture.descriptor.array_layer_count >= 6
    //        && texture.descriptor.size.width == texture.descriptor.size.height
    //    {
    //        TextureViewDimension::Cube
    //    } else {
    //        match texture.descriptor.dimension {
    //            TextureDimension::D1 => TextureViewDimension::D1,
    //            TextureDimension::D2 => TextureViewDimension::D2,
    //            TextureDimension::D3 => TextureViewDimension::D3,
    //        }
    //    };

    let dimension = match texture.descriptor.dimension {
        TextureDimension::D1 => TextureViewDimension::D1,
        TextureDimension::D2 => TextureViewDimension::D2,
        TextureDimension::D3 => TextureViewDimension::D3,
    };

    TextureViewDescriptor {
        array_layer_count: texture.descriptor.array_layer_count,
        format: texture.descriptor.format,
        mip_level_count: texture.descriptor.mip_level_count,
        dimension,
        base_array_layer: 0,
        base_mip_level: 0,
        aspect,
    }
}

impl Texture {
    pub fn create_view(&self, descriptor: TextureViewDescriptor) -> Result<TextureView, vk::Result> {
        let texture_view = TextureViewInner::new(self.inner.clone(), descriptor)?;
        Ok(texture_view.into())
    }

    pub fn create_default_view(&self) -> Result<TextureView, vk::Result> {
        let descriptor = default_texture_view_descriptor(&self.inner);
        log::trace!("default image_view descriptor: {:?}", descriptor);
        self.create_view(descriptor)
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
            extent: util::extent_3d(descriptor.size),
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
            if let vk_mem::ErrorKind::Vulkan(vk::Result::ERROR_VALIDATION_FAILED_EXT) = e.kind() {
                unsafe {
                    let dummy = device.raw.create_image(&create_info, None)?;
                    device.raw.destroy_image(dummy, None);
                    return Err(vk::Result::ERROR_VALIDATION_FAILED_EXT);
                }
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

    pub fn transition_usage_now(
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
        let dst_stage_mask = pipeline_stage(usage, format);

        let src_access_mask = access_flags(*last_usage, format);
        let dst_access_mask = access_flags(usage, format);

        let old_layout = image_layout(*last_usage, format);
        let new_layout = image_layout(usage, format);

        log::trace!(
            "usage: {:?}, last_usage: {:?}, src_stage_mask: {}, src_access_mask: {}, old_layout: {}",
            usage,
            *last_usage,
            src_stage_mask,
            src_access_mask,
            old_layout
        );
        log::trace!(
            "usage: {:?}, last_usage: {:?}, dst_stage_mask: {}, dst_access_mask: {}, new_layout: {}",
            usage,
            *last_usage,
            dst_stage_mask,
            dst_access_mask,
            new_layout
        );

        let image = self.handle;

        let aspect_mask = aspect_mask(format);
        let base_mip_level = 0;
        let level_count = self.descriptor.mip_level_count;
        let base_array_layer = 0;
        let layer_count = self.descriptor.array_layer_count;

        let image_memory_barrier = vk::ImageMemoryBarrier {
            src_access_mask,
            dst_access_mask,
            old_layout,
            new_layout,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level,
                level_count,
                base_array_layer,
                layer_count,
            },
            ..Default::default()
        };

        let dependency_flags = vk::DependencyFlags::empty();
        let memory_barriers = &[];
        let buffer_memory_barriers = &[];
        let image_memory_barriers = &[image_memory_barrier];

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

impl TextureViewInner {
    pub fn new(texture: Arc<TextureInner>, descriptor: TextureViewDescriptor) -> Result<TextureViewInner, vk::Result> {
        let aspect_mask = unsafe { std::mem::transmute(descriptor.aspect) };
        let base_mip_level = descriptor.base_mip_level;
        let level_count = descriptor.mip_level_count;
        let base_array_layer = descriptor.base_array_layer;
        let layer_count = descriptor.array_layer_count;
        let view_type = image_view_type(&descriptor);

        let create_info = vk::ImageViewCreateInfo {
            format: image_format(descriptor.format),
            flags: vk::ImageViewCreateFlags::empty(),
            image: texture.handle,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level,
                level_count,
                base_array_layer,
                layer_count,
            },
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A,
            },
            view_type,
            ..Default::default()
        };

        log::trace!("image_view create_info: {:?}", create_info);

        let image_view = unsafe { texture.device.raw.create_image_view(&create_info, None)? };

        log::trace!("created image_view: {:?}", image_view);

        Ok(TextureViewInner {
            descriptor,
            handle: image_view,
            texture,
        })
    }

    /// Returns the sample count and the size of the texture,
    /// adjusted by the base mipmap level.
    pub fn get_sample_count_and_mipmap_size(&self) -> (u32, Extent3D) {
        let base_mip_level = self.descriptor.base_mip_level;
        let sample_count = self.texture.descriptor.sample_count;
        let width = self.texture.descriptor.size.width >> base_mip_level;
        let height = self.texture.descriptor.size.height >> base_mip_level;
        let depth = self.texture.descriptor.size.depth >> base_mip_level;

        (sample_count, Extent3D { width, height, depth })
    }
}

impl Into<TextureView> for TextureViewInner {
    fn into(self) -> TextureView {
        TextureView { inner: Arc::new(self) }
    }
}

impl Drop for TextureViewInner {
    fn drop(&mut self) {
        let mut state = self.texture.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}
