use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{texture, AdapterInner, SurfaceInner, TextureViewInner};
use crate::imp::{DeviceInner, InstanceInner, SwapchainInner, TextureInner};
use crate::{
    Error, Extent3D, PowerPreference, Swapchain, SwapchainDescriptor, SwapchainError, SwapchainImage, Texture,
    TextureDescriptor, TextureDimension, TextureUsageFlags, TextureView,
};

use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::vk::StructureType;
use parking_lot::Mutex;

use crate::imp::texture::SubresourceUsageTracker;
use std::sync::Arc;
use std::time::Duration;

// The gpuweb spec doesn't mention anything about color space for the swapchain
// creation, so we'll hardcode one that should be available everywhere.
// Supported formats should only be advertised for this color space.
pub const COLOR_SPACE: vk::ColorSpaceKHR = vk::ColorSpaceKHR::SRGB_NONLINEAR;

impl Swapchain {
    pub fn acquire_next_image(&self) -> Result<SwapchainImage, SwapchainError> {
        let image_index = self.inner.acquire_next_image_index()?;
        Ok(SwapchainImage {
            swapchain: Arc::clone(&self.inner),
            texture: Texture {
                inner: Arc::clone(&self.inner.textures[image_index as usize]),
            },
            view: TextureView {
                inner: Arc::clone(&self.inner.views[image_index as usize]),
            },
            image_index,
        })
    }
}

impl SwapchainInner {
    /// Recipe: _Creating a swapchain_ (page `105`)
    pub fn new(
        device: Arc<DeviceInner>,
        descriptor: SwapchainDescriptor,
        old_swapchain: Option<&SwapchainInner>,
    ) -> Result<SwapchainInner, Error> {
        unsafe {
            let instance = &device.adapter.instance;
            let physical_device = device.adapter.physical_device;
            let surface_handle = descriptor.surface.inner.handle;
            let surface_caps = device
                .adapter
                .instance
                .raw_ext
                .surface
                .get_physical_device_surface_capabilities(physical_device, surface_handle)?;

            let dimensions = surface_caps.current_extent;

            let surface_format = vk::SurfaceFormatKHR {
                format: texture::image_format(descriptor.format),
                color_space: COLOR_SPACE,
            };
            let surface_image_transform = vk::SurfaceTransformFlagsKHR::IDENTITY;
            let surface_image_usage = texture::image_usage(descriptor.usage, descriptor.format);
            let surface_image_count = surface_image_count(&surface_caps);
            let surface_image_extent = surface_image_extent(&surface_caps, dimensions);
            let surface_present_mode = surface_present_mode(instance, &device.adapter, surface_handle)?;

            surface_format_check(&descriptor.surface.inner, physical_device, surface_format)?;
            surface_image_usage_check(&surface_caps, surface_image_usage)?;
            surface_image_transform_check(&surface_caps, surface_image_transform)?;

            let old_swapchain_handle = old_swapchain.map(|s| s.handle).unwrap_or_else(vk::SwapchainKHR::null);

            let create_info = vk::SwapchainCreateInfoKHR {
                s_type: StructureType::SWAPCHAIN_CREATE_INFO_KHR,
                flags: vk::SwapchainCreateFlagsKHR::empty(),
                surface: surface_handle,
                min_image_count: surface_image_count,
                image_format: surface_format.format,
                image_color_space: surface_format.color_space,
                image_extent: surface_image_extent,
                image_array_layers: 1,
                image_usage: surface_image_usage,
                image_sharing_mode: vk::SharingMode::EXCLUSIVE,
                p_queue_family_indices: std::ptr::null(),
                queue_family_index_count: 0,
                pre_transform: surface_image_transform,
                present_mode: surface_present_mode,
                composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
                clipped: vk::TRUE,
                old_swapchain: old_swapchain_handle,
                p_next: std::ptr::null(),
            };

            // TODO: Attempting to create a new swapchain for a surface will fail unless:
            //       1) No swapchain has ever been created for this surface
            //       2) The previous swapchain was dropped (and will be purged here)
            //       2) The previous swapchain was passed in as `old_swapchain`
            if old_swapchain.is_none() {
                let mut state = device.state.lock();
                state.get_fenced_deleter().purge_swapchains(&device);
            }

            let swapchain = device.raw_ext.swapchain.create_swapchain(&create_info, None)?;
            log::debug!("created swapchain: {:?}", swapchain);

            let images = device.raw_ext.swapchain.get_swapchain_images(swapchain)?;

            let texture_descriptor = TextureDescriptor {
                size: Extent3D {
                    width: surface_image_extent.width,
                    height: surface_image_extent.height,
                    depth: 1,
                },
                array_layer_count: create_info.image_array_layers,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: descriptor.format,
                usage: TextureUsageFlags::PRESENT,
            };

            let textures = images.iter().cloned().map(|handle| {
                let subresource_usage = SubresourceUsageTracker::new(1, 1, descriptor.format);
                Arc::new(TextureInner {
                    handle,
                    device: device.clone(),
                    allocation: None,
                    allocation_info: None,
                    subresource_usage: Mutex::new(subresource_usage),
                    descriptor: texture_descriptor,
                })
            });
            let textures: Vec<_> = textures.collect();

            let mut views = Vec::with_capacity(textures.len());

            for texture in textures.iter() {
                let view = TextureViewInner::new(texture.clone(), texture::default_texture_view_descriptor(&texture))?;
                views.push(Arc::new(view));
            }

            // initial transition
            let mut state = device.state.lock();
            let command_buffer = state.get_pending_command_buffer(&device)?;
            for texture in textures.iter() {
                texture.transition_usage_now(command_buffer, texture.descriptor.usage, None)?;
            }
            drop(state);

            Ok(SwapchainInner {
                handle: swapchain,
                textures,
                views,
                device,
                surface: descriptor.surface.inner.clone(),
            })
        }
    }

    fn acquire_next_image_index(&self) -> Result<u32, SwapchainError> {
        unsafe {
            let timeout = Duration::from_millis(100);
            let timeout = timeout.as_nanos() as u64;
            let fence = vk::Fence::null();
            let create_info = vk::SemaphoreCreateInfo::builder();
            let semaphore = self.device.raw.create_semaphore(&create_info, None)?;
            let result = self
                .device
                .raw_ext
                .swapchain
                .acquire_next_image(self.handle, timeout, semaphore, fence);

            loop {
                match result {
                    Ok((index, false)) => {
                        let mut state = self.device.state.lock();
                        state.add_wait_semaphore(semaphore);
                        return Ok(index);
                    }
                    Ok((index, true)) => {
                        let mut state = self.device.state.lock();
                        state.add_wait_semaphore(semaphore);
                        log::warn!("acquire_next_image_index: suboptimal");
                        return Ok(index);
                    }
                    Err(vk::Result::TIMEOUT) => {
                        log::warn!("acquire_next_image_index: timeout");
                        continue;
                    }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        let mut state = self.device.state.lock();
                        let serial = state.get_next_pending_serial();
                        state.get_fenced_deleter().delete_when_unused(semaphore, serial);
                        return Err(SwapchainError::OutOfDate);
                    }
                    Err(err) => {
                        let mut state = self.device.state.lock();
                        let serial = state.get_next_pending_serial();
                        state.get_fenced_deleter().delete_when_unused(semaphore, serial);
                        return Err(SwapchainError::Other(Error::from(err)));
                    }
                }
            }
        }
    }
}

impl Into<Swapchain> for SwapchainInner {
    fn into(self) -> Swapchain {
        Swapchain { inner: Arc::new(self) }
    }
}

impl Drop for SwapchainInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let next_pending_serial = state.get_next_pending_serial();
        state
            .get_fenced_deleter()
            .delete_when_unused((self.handle, self.surface.clone()), next_pending_serial);
    }
}

/// Recipe: _Selecting a desired presentation mode_ (page `86`)
///
/// Selects `Mailbox` mode is available, otherwise defaults to `Fifo`.
fn surface_present_mode(
    instance: &InstanceInner,
    adapter: &AdapterInner,
    surface: vk::SurfaceKHR,
) -> VkResult<vk::PresentModeKHR> {
    let present_mode = match adapter.options.power_preference {
        PowerPreference::LowPower => vk::PresentModeKHR::FIFO,
        PowerPreference::HighPerformance => {
            let physical_device = adapter.physical_device;
            unsafe {
                instance
                    .raw_ext
                    .surface
                    .get_physical_device_surface_present_modes(physical_device, surface)?
                    .iter()
                    .cloned()
                    .find(|mode| *mode == vk::PresentModeKHR::MAILBOX)
                    .unwrap_or(vk::PresentModeKHR::FIFO)
            }
        }
    };
    log::debug!("selected present mode: {:?}", present_mode);
    Ok(present_mode)
}

/// Recipe: _Selecting the number of swapchain images_ (page `94`)
pub fn surface_image_count(surface_caps: &vk::SurfaceCapabilitiesKHR) -> u32 {
    let image_count = surface_caps.min_image_count + 1;
    let count = match surface_caps.max_image_count {
        0 => image_count,
        max_image_count => {
            if image_count > max_image_count {
                max_image_count
            } else {
                image_count
            }
        }
    };
    log::debug!("selected image count: {:?}", count);
    count
}

/// Recipe: _Choosing a size of swapchain images_ (page `96`)
pub fn surface_image_extent(
    surface_caps: &vk::SurfaceCapabilitiesKHR,
    requested_dimensions: vk::Extent2D,
) -> vk::Extent2D {
    let extent = match surface_caps.current_extent {
        vk::Extent2D { width: 0, height: 0 } => {
            let mut width = requested_dimensions.width;
            let mut height = requested_dimensions.height;
            if surface_caps.min_image_extent.width > width {
                width = surface_caps.min_image_extent.width;
            }
            if surface_caps.min_image_extent.height > height {
                height = surface_caps.min_image_extent.height;
            }
            if surface_caps.max_image_extent.width < width {
                width = surface_caps.max_image_extent.width;
            }
            if surface_caps.max_image_extent.height < height {
                height = surface_caps.max_image_extent.height;
            }
            vk::Extent2D { width, height }
        }
        current_extent => current_extent,
    };
    log::debug!("selected image extent: {:?}", extent);
    extent
}

/// Recipe: _Selecting desired usage scenarios of swapchain images_ (page `98`)
pub fn surface_image_usage_check(
    surface_caps: &vk::SurfaceCapabilitiesKHR,
    usage_flags: vk::ImageUsageFlags,
) -> Result<(), Error> {
    if surface_caps.supported_usage_flags.contains(usage_flags) {
        log::debug!("selected image usage: {:?}", usage_flags);
        Ok(())
    } else {
        Err(Error::from(format!(
            "Unsupported surface image usage flags: {:?}",
            usage_flags
        )))
    }
}

/// Recipe: _Selecting a transformation of swapchain images_ (page `100`)
pub fn surface_image_transform_check(
    surface_caps: &vk::SurfaceCapabilitiesKHR,
    transform_flags: vk::SurfaceTransformFlagsKHR,
) -> Result<(), Error> {
    if surface_caps.supported_transforms.contains(transform_flags) {
        log::debug!("selected image transform: {:?}", transform_flags);
        Ok(())
    } else {
        Err(Error::from(format!(
            "Unsupported surface image transform flags: {:?}",
            transform_flags
        )))
    }
}

fn surface_format_check(
    surface: &SurfaceInner,
    physical_device: vk::PhysicalDevice,
    requested_format: vk::SurfaceFormatKHR,
) -> Result<(), Error> {
    if surface.is_supported_format(physical_device, requested_format)? {
        log::debug!(
            "selected format: {:?}, color_space: {:?}",
            requested_format.format,
            requested_format.color_space
        );
        Ok(())
    } else {
        Err(Error::from(format!(
            "Unsupported surface format: {:?}",
            requested_format
        )))
    }
}
