use ash::version::DeviceV1_0;
use ash::vk;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{render_pass, util};
use crate::imp::{DeviceInner, TextureInner, TextureViewInner};
use crate::{
    Extent3D, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

use ash::vk::MemoryPropertyFlags;
use parking_lot::Mutex;
use smallvec::SmallVec;
use std::collections::HashMap;
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
        TextureFormat::D32Float => true,
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
        // Dawn notes that:
        //  "Depending on whether parts of the texture have been transitioned to only
        //   TransferSrc or a combination with something else, the texture could be in a
        //   combination of GENERAL and TRANSFER_SRC_OPTIMAL. This would be a problem, so we
        //   make TransferSrc use GENERAL."
        // However, this is causing performance validation warnings, so we'll use
        // TRANSFER_SRC_OPTIMAL for now.
        TextureUsageFlags::TRANSFER_SRC => vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        TextureUsageFlags::STORAGE => vk::ImageLayout::GENERAL,
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
            samples: render_pass::sample_count(descriptor.sample_count)?,
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

        let subresource_usage = SubresourceUsageTracker::new(
            descriptor.mip_level_count,
            descriptor.array_layer_count,
            descriptor.format,
        );

        Ok(TextureInner {
            handle: image,
            device: device.clone(),
            allocation: Some(allocation),
            allocation_info: Some(allocation_info),
            descriptor,
            subresource_usage: Mutex::new(subresource_usage),
        })
    }

    /// Transition the texture usage. A `subresource_range` of `None` indicates the whole texture.
    pub fn transition_usage_now(
        &self,
        command_buffer: vk::CommandBuffer,
        usage: TextureUsageFlags,
        subresource: Option<Subresource>,
    ) -> Result<(), vk::Result> {
        let format = self.descriptor.format;

        // log2(32768) + 1 = 16; enough barriers on the stack for a non-array image with mipmaps, up to 32768 x 32768
        let mut image_memory_barriers = SmallVec::<[vk::ImageMemoryBarrier; 16]>::new();

        let mut src_stage_mask = vk::PipelineStageFlags::empty();
        let dst_stage_mask = pipeline_stage(usage, format);

        let mut add_image_memory_barrier =
            |range: vk::ImageSubresourceRange, range_last_usage: &mut TextureUsageFlags| {
                // TODO: Add a version of this optimization back at the "whole texture" level.
                //       Example: If we're only repeatedly requesting that the image is SAMPLED,
                //       there's no need to iterate all of the subresources every time.

                let last_read_only = (*range_last_usage & read_only_texture_usage()) == *range_last_usage;
                if last_read_only && *range_last_usage == usage {
                    return;
                }

                src_stage_mask |= pipeline_stage(*range_last_usage, format);

                let src_access_mask = access_flags(*range_last_usage, format);
                let dst_access_mask = access_flags(usage, format);

                let old_layout = image_layout(*range_last_usage, format);
                let new_layout = image_layout(usage, format);

                // TODO: We should probably set old_layout to UNDEFINED as an optimization when
                //       new_layout is TRANSFER_DST_OPTIMAL

                let image_memory_barrier = vk::ImageMemoryBarrier {
                    src_access_mask,
                    dst_access_mask,
                    old_layout,
                    new_layout,
                    image: self.handle,
                    subresource_range: range,
                    src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                    dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                    ..Default::default()
                };

                log::trace!(
                    concat!(
                        "mip_level: {}, array_layer: {}, ",
                        "old_usage: {:?}, old_layout: {}, src_stage_mask: {}, src_access_mask: {}, ",
                        "new_usage: {:?}, new_layout: {}, dst_stage_mask: {}, dst_access_mask: {}"
                    ),
                    image_memory_barrier.subresource_range.base_mip_level,
                    image_memory_barrier.subresource_range.base_array_layer,
                    *range_last_usage,
                    old_layout,
                    src_stage_mask,
                    src_access_mask,
                    usage,
                    new_layout,
                    dst_stage_mask,
                    dst_access_mask,
                );

                *range_last_usage = usage;

                image_memory_barriers.push(image_memory_barrier);
            };

        match subresource {
            Some(subresource) => {
                let mut subresource_usage = self.subresource_usage.lock();
                let (range, range_last_usage) = subresource_usage.usage_mut(subresource);
                add_image_memory_barrier(range, range_last_usage);
            }
            None => {
                for (range, range_last_usage) in self.subresource_usage.lock().iter_mut() {
                    add_image_memory_barrier(range, range_last_usage);
                }
            }
        }

        if !image_memory_barriers.is_empty() {
            let dependency_flags = vk::DependencyFlags::empty();
            let memory_barriers = &[];
            let buffer_memory_barriers = &[];
            let image_memory_barriers: &[vk::ImageMemoryBarrier] = &*image_memory_barriers;
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
        }

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

#[derive(Debug)]
pub struct SubresourceUsageTracker {
    ranges: HashMap<Subresource, TextureUsageFlags>,
    aspect_mask: vk::ImageAspectFlags,
    mip_levels: u32,
    array_layers: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Subresource {
    pub mip_level: u32,
    pub array_layer: u32,
}

impl SubresourceUsageTracker {
    pub fn new(mip_levels: u32, array_layers: u32, format: TextureFormat) -> SubresourceUsageTracker {
        let none = TextureUsageFlags::NONE;
        let mut ranges = HashMap::with_capacity((mip_levels * array_layers) as usize);
        (0..array_layers).for_each(|array_layer| {
            (0..mip_levels).for_each(|mip_level| {
                let subresource = Subresource { mip_level, array_layer };
                ranges.insert(subresource, none);
            });
        });
        SubresourceUsageTracker {
            ranges,
            aspect_mask: aspect_mask(format),
            mip_levels,
            array_layers,
        }
    }

    fn usage_mut(&mut self, subresource: Subresource) -> (vk::ImageSubresourceRange, &mut TextureUsageFlags) {
        let aspect_mask = self.aspect_mask;
        let usage = self
            .ranges
            .get_mut(&subresource)
            .expect("invalid subresource mip_level or array_layer");
        let range = vk::ImageSubresourceRange {
            aspect_mask,
            base_mip_level: subresource.mip_level,
            level_count: 1,
            base_array_layer: subresource.array_layer,
            layer_count: 1,
        };
        (range, usage)
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = (vk::ImageSubresourceRange, &mut TextureUsageFlags)> {
        let aspect_mask = self.aspect_mask;
        self.ranges.iter_mut().map(move |(k, usage)| {
            let range = vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: k.mip_level,
                level_count: 1,
                base_array_layer: k.array_layer,
                layer_count: 1,
            };
            (range, usage)
        })
    }
}
