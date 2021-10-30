use ash::vk;

use vk_mem::{Allocation, Allocator};

use crate::imp::serial::{Serial, SerialQueue};
use crate::imp::{DeviceInner, SurfaceInner};

use std::fmt::Debug;
use std::sync::Arc;

#[derive(Default, Debug)]
pub struct FencedDeleter {
    swapchains: SerialQueue<(vk::SwapchainKHR, Arc<SurfaceInner>)>,
    semaphores: SerialQueue<vk::Semaphore>,
    buffers: SerialQueue<(vk::Buffer, Allocation)>,
    buffer_views: SerialQueue<vk::BufferView>,
    images: SerialQueue<(vk::Image, Allocation)>,
    image_views: SerialQueue<vk::ImageView>,
    samplers: SerialQueue<vk::Sampler>,
    descriptor_set_layouts: SerialQueue<vk::DescriptorSetLayout>,
    descriptor_pools: SerialQueue<vk::DescriptorPool>,
    shader_modules: SerialQueue<vk::ShaderModule>,
    pipeline_layouts: SerialQueue<vk::PipelineLayout>,
    pipelines: SerialQueue<vk::Pipeline>,
    framebuffers: SerialQueue<vk::Framebuffer>,
    surface_keepalive: SerialQueue<Arc<SurfaceInner>>,
    // NOTE: Update is_empty(&self) when adding to this list
}

impl FencedDeleter {
    pub unsafe fn purge_swapchains(&mut self, device: &DeviceInner) {
        for ((handle, surface), serial) in self.swapchains.drain(..) {
            log::debug!("destroy swapchain (purged): {:?}, completed: {:?}", handle, serial);
            device.raw_ext.swapchain.destroy_swapchain(handle, None);
            drop(surface); // assert that the surface lives longer than the swapchain
        }
    }

    #[allow(clippy::cognitive_complexity)]
    pub fn tick(&mut self, last_completed_serial: Serial, device: &DeviceInner, allocator: &Allocator) {
        if log::log_enabled!(log::Level::Trace) {
            log::trace!("last_completed_serial:   {:?}", last_completed_serial);
            log::trace!(" swapchains:             {}", self.swapchains.len());
            log::trace!(" semaphores:             {}", self.semaphores.len());
            log::trace!(" buffers:                {}", self.buffers.len());
            log::trace!(" buffer_views:           {}", self.buffer_views.len());
            log::trace!(" images:                 {}", self.images.len());
            log::trace!(" image_views:            {}", self.image_views.len());
            log::trace!(" descriptor_set_layouts: {}", self.descriptor_set_layouts.len());
            log::trace!(" descriptor_pools:       {}", self.descriptor_pools.len());
            log::trace!(" shader_modules:         {}", self.shader_modules.len());
            log::trace!(" pipeline_layouts:       {}", self.pipeline_layouts.len());
            log::trace!(" pipelines:              {}", self.pipelines.len());
            log::trace!(" framebuffers:           {}", self.framebuffers.len());
        }

        for ((handle, surface), serial) in self.swapchains.drain_up_to(last_completed_serial) {
            log::debug!("destroy swapchain: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw_ext.swapchain.destroy_swapchain(handle, None);
            }
            drop(surface); // the surface must kept alive at least as long as the swapchain
        }

        for (handle, serial) in self.semaphores.drain_up_to(last_completed_serial) {
            log::trace!("destroy semaphore: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_semaphore(handle, None);
            }
        }

        for ((handle, allocation), serial) in self.buffers.drain_up_to(last_completed_serial) {
            log::trace!("destroy buffer: {:?}, completed: {:?}", handle, serial);
            allocator.destroy_buffer(handle, &allocation);
        }

        for ((handle, allocation), serial) in self.images.drain_up_to(last_completed_serial) {
            log::trace!("destroy image: {:?}, completed: {:?}", handle, serial);
            allocator.destroy_image(handle, &allocation);
        }

        for (handle, serial) in self.image_views.drain_up_to(last_completed_serial) {
            log::trace!("destroy image_view: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_image_view(handle, None);
            }
        }

        for (handle, serial) in self.buffer_views.drain_up_to(last_completed_serial) {
            log::trace!("destroy buffer_view: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_buffer_view(handle, None);
            }
        }

        for (handle, serial) in self.samplers.drain_up_to(last_completed_serial) {
            log::trace!("destroy sampler: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_sampler(handle, None);
            }
        }

        for (handle, serial) in self.descriptor_set_layouts.drain_up_to(last_completed_serial) {
            log::trace!("destroy descriptor set layout: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_descriptor_set_layout(handle, None);
            }
        }

        for (handle, serial) in self.descriptor_pools.drain_up_to(last_completed_serial) {
            log::trace!("destroy descriptor pool: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_descriptor_pool(handle, None);
            }
        }

        for (handle, serial) in self.shader_modules.drain_up_to(last_completed_serial) {
            log::trace!("destroy shader module: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_shader_module(handle, None);
            }
        }

        for (handle, serial) in self.pipeline_layouts.drain_up_to(last_completed_serial) {
            log::trace!("destroy pipeline layout: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_pipeline_layout(handle, None);
            }
        }

        for (handle, serial) in self.pipelines.drain_up_to(last_completed_serial) {
            log::trace!("destroy pipeline: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_pipeline(handle, None);
            }
        }

        for (handle, serial) in self.framebuffers.drain_up_to(last_completed_serial) {
            log::trace!("destroy framebuffers: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_framebuffer(handle, None);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.swapchains.is_empty()
            && self.semaphores.is_empty()
            && self.buffers.is_empty()
            && self.buffer_views.is_empty()
            && self.images.is_empty()
            && self.image_views.is_empty()
            && self.samplers.is_empty()
            && self.descriptor_set_layouts.is_empty()
            && self.descriptor_pools.is_empty()
            && self.shader_modules.is_empty()
            && self.pipeline_layouts.is_empty()
            && self.pipelines.is_empty()
            && self.framebuffers.is_empty()
            && self.surface_keepalive.is_empty()
    }
}

impl Drop for FencedDeleter {
    fn drop(&mut self) {
        if !self.is_empty() {
            log::error!("FencedDeleter dropped while objects are live")
        }
    }
}

pub trait DeleteWhenUnused<T: Debug> {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<T>;
    fn delete_when_unused(&mut self, resource: T, after_completed_serial: Serial) {
        log::trace!(
            "delete_when_unused: {:?}, after_completed_serial: {:?}",
            resource,
            after_completed_serial
        );
        self.get_serial_queue().enqueue(resource, after_completed_serial);
    }
}

impl DeleteWhenUnused<(vk::SwapchainKHR, Arc<SurfaceInner>)> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<(vk::SwapchainKHR, Arc<SurfaceInner>)> {
        &mut self.swapchains
    }
}

impl DeleteWhenUnused<vk::Semaphore> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Semaphore> {
        &mut self.semaphores
    }
}

impl DeleteWhenUnused<(vk::Buffer, Allocation)> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<(vk::Buffer, Allocation)> {
        &mut self.buffers
    }
}

impl DeleteWhenUnused<vk::BufferView> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::BufferView> {
        &mut self.buffer_views
    }
}

impl DeleteWhenUnused<(vk::Image, Allocation)> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<(vk::Image, Allocation)> {
        &mut self.images
    }
}

impl DeleteWhenUnused<vk::ImageView> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::ImageView> {
        &mut self.image_views
    }
}

impl DeleteWhenUnused<vk::Sampler> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Sampler> {
        &mut self.samplers
    }
}

impl DeleteWhenUnused<vk::DescriptorSetLayout> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::DescriptorSetLayout> {
        &mut self.descriptor_set_layouts
    }
}

impl DeleteWhenUnused<vk::DescriptorPool> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::DescriptorPool> {
        &mut self.descriptor_pools
    }
}

impl DeleteWhenUnused<vk::ShaderModule> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::ShaderModule> {
        &mut self.shader_modules
    }
}

impl DeleteWhenUnused<vk::PipelineLayout> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::PipelineLayout> {
        &mut self.pipeline_layouts
    }
}

impl DeleteWhenUnused<vk::Pipeline> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Pipeline> {
        &mut self.pipelines
    }
}

impl DeleteWhenUnused<vk::Framebuffer> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Framebuffer> {
        &mut self.framebuffers
    }
}
