use ash::version::DeviceV1_0;
use ash::vk;

use crate::imp::serial::{Serial, SerialQueue};
use crate::imp::DeviceInner;

use std::fmt::Debug;

#[derive(Default, Debug)]
pub struct FencedDeleter {
    swapchains_to_delete: SerialQueue<vk::SwapchainKHR>,
    semaphores_to_delete: SerialQueue<vk::Semaphore>,
}

impl FencedDeleter {
    pub fn tick(&mut self, last_completed_serial: Serial, device: &DeviceInner) {
        log::trace!("swapchains_to_delete.len: {}", self.swapchains_to_delete.len());
        log::trace!("semaphores_to_delete.len: {}", self.semaphores_to_delete.len());

        for (swapchain, serial) in self.swapchains_to_delete.drain_up_to(last_completed_serial) {
            log::debug!("destroying swapchain: {:?}, completed_serial: {:?}", swapchain, serial);
            unsafe {
                device.raw_ext.swapchain.destroy_swapchain(swapchain, None);
            }
        }

        for (semaphore, serial) in self.semaphores_to_delete.drain_up_to(last_completed_serial) {
            log::trace!("destroying semaphore: {:?}, completed_serial: {:?}", semaphore, serial);
            unsafe {
                device.raw.destroy_semaphore(semaphore, None);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.swapchains_to_delete.iter().count() == 0 && self.semaphores_to_delete.iter().count() == 0
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

impl DeleteWhenUnused<vk::SwapchainKHR> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::SwapchainKHR> {
        &mut self.swapchains_to_delete
    }
}

impl DeleteWhenUnused<vk::Semaphore> for FencedDeleter {
    fn get_serial_queue(&mut self) -> &mut SerialQueue<vk::Semaphore> {
        &mut self.semaphores_to_delete
    }
}
