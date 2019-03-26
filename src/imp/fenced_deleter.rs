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
}

impl FencedDeleter {
    pub fn tick(&mut self, last_completed_serial: Serial, device: &DeviceInner, allocator: &mut Allocator) {
        log::trace!("swapchains_to_delete.len: {}", self.swapchains_to_delete.len());
        log::trace!("semaphores_to_delete.len: {}", self.semaphores_to_delete.len());
        log::trace!("buffers_to_delete.len: {}", self.semaphores_to_delete.len());
        log::trace!("images_to_delete.len: {}", self.semaphores_to_delete.len());

        for ((swapchain, surface), serial) in self.swapchains_to_delete.drain_up_to(last_completed_serial) {
            log::debug!("destroying swapchain: {:?}, completed_serial: {:?}", swapchain, serial);
            unsafe {
                device.raw_ext.swapchain.destroy_swapchain(swapchain, None);
            }
            drop(surface); // the surface must kept alive at least as long as the swapchain
        }

        for (semaphore, serial) in self.semaphores_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroying semaphore: {:?}, completed_serial: {:?}", semaphore, serial);
            unsafe {
                device.raw.destroy_semaphore(semaphore, None);
            }
        }

        for ((buffer, allocation), serial) in self.buffers_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroying buffer: {:?}, completed_serial: {:?}", buffer, serial);
            if let Err(e) = allocator.destroy_buffer(buffer, &allocation) {
                log::warn!("buffer destruction failed; buffer: {:?}, error: {:?}", buffer, e);
            }
        }

        for ((image, allocation), serial) in self.images_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroying image: {:?}, completed_serial: {:?}", image, serial);
            if let Err(e) = allocator.destroy_image(image, &allocation) {
                log::warn!("image destruction failed; buffer: {:?}, error: {:?}", image, e);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.swapchains_to_delete.len() == 0
            && self.semaphores_to_delete.len() == 0
            && self.buffers_to_delete.len() == 0
        && self.images_to_delete.len() == 0
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
