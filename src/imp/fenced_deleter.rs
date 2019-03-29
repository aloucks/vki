use ash::version::DeviceV1_0;
use ash::vk;

use vk_mem::{Allocation, Allocator};

use crate::imp::serial::{Serial, SerialQueue};
use crate::imp::{DeviceInner, SurfaceInner};

use std::fmt::Debug;
use std::sync::Arc;

#[derive(Default, Debug)]
pub struct FencedDeleter {
    swapchains_to_delete: SerialQueue<(vk::SwapchainKHR, Arc<SurfaceInner>)>,
    semaphores_to_delete: SerialQueue<vk::Semaphore>,
    buffers_to_delete: SerialQueue<(vk::Buffer, Allocation)>,
    images_to_delete: SerialQueue<(vk::Image, Allocation)>,
    image_views_to_delete: SerialQueue<vk::ImageView>,
    samplers_to_delete: SerialQueue<vk::Sampler>,
    descriptor_set_layouts_to_delete: SerialQueue<vk::DescriptorSetLayout>,
    descriptor_pools_to_delete: SerialQueue<vk::DescriptorPool>,
}

impl FencedDeleter {
    pub fn tick(&mut self, last_completed_serial: Serial, device: &DeviceInner, allocator: &mut Allocator) {
        log::trace!("last_completed_serial: {:?}", last_completed_serial);
        log::trace!("swapchains_to_delete:             {}", self.swapchains_to_delete.len());
        log::trace!("semaphores_to_delete:             {}", self.semaphores_to_delete.len());
        log::trace!("buffers_to_delete:                {}", self.buffers_to_delete.len());
        log::trace!("images_to_delete:                 {}", self.images_to_delete.len());
        log::trace!("image_views_to_delete:            {}", self.image_views_to_delete.len());
        log::trace!("descriptor_set_layouts_to_delete: {}", self.image_views_to_delete.len());

        for ((handle, surface), serial) in self.swapchains_to_delete.drain_up_to(last_completed_serial) {
            log::debug!("destroy swapchain: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw_ext.swapchain.destroy_swapchain(handle, None);
            }
            drop(surface); // the surface must kept alive at least as long as the swapchain
        }

        for (handle, serial) in self.semaphores_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy semaphore: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_semaphore(handle, None);
            }
        }

        for ((handle, allocation), serial) in self.buffers_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy buffer: {:?}, completed: {:?}", handle, serial);
            if let Err(e) = allocator.destroy_buffer(handle, &allocation) {
                log::warn!("buffer destruction failed; buffer: {:?}, error: {:?}", handle, e);
            }
        }

        for ((handle, allocation), serial) in self.images_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy image: {:?}, completed: {:?}", handle, serial);
            if let Err(e) = allocator.destroy_image(handle, &allocation) {
                log::warn!("image destruction failed; buffer: {:?}, error: {:?}", handle, e);
            }
        }

        for (handle, serial) in self.image_views_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy image_view: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_image_view(handle, None);
            }
        }

        for (handle, serial) in self.samplers_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy sampler: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_sampler(handle, None);
            }
        }

        for (handle, serial) in self.descriptor_set_layouts_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy descriptor set layout: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_descriptor_set_layout(handle, None);
            }
        }

        for (handle, serial) in self.descriptor_pools_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroy descriptor pool: {:?}, completed: {:?}", handle, serial);
            unsafe {
                device.raw.destroy_descriptor_pool(handle, None);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.swapchains_to_delete.len() == 0
            && self.semaphores_to_delete.len() == 0
            && self.buffers_to_delete.len() == 0
            && self.images_to_delete.len() == 0
            && self.samplers_to_delete.len() == 0
            && self.descriptor_set_layouts_to_delete.len() == 0
            && self.descriptor_pools_to_delete.len() == 0
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
        &mut self.swapchains_to_delete
    }
}

impl DeleteWhenUnused<vk::Semaphore> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Semaphore> {
        &mut self.semaphores_to_delete
    }
}

impl DeleteWhenUnused<(vk::Buffer, Allocation)> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<(vk::Buffer, Allocation)> {
        &mut self.buffers_to_delete
    }
}

impl DeleteWhenUnused<(vk::Image, Allocation)> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<(vk::Image, Allocation)> {
        &mut self.images_to_delete
    }
}

impl DeleteWhenUnused<vk::ImageView> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::ImageView> {
        &mut self.image_views_to_delete
    }
}

impl DeleteWhenUnused<vk::Sampler> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Sampler> {
        &mut self.samplers_to_delete
    }
}

impl DeleteWhenUnused<vk::DescriptorSetLayout> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::DescriptorSetLayout> {
        &mut self.descriptor_set_layouts_to_delete
    }
}

impl DeleteWhenUnused<vk::DescriptorPool> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::DescriptorPool> {
        &mut self.descriptor_pools_to_delete
    }
}
