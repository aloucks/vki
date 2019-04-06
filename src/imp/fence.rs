use crate::imp::{DeviceInner, FenceInner};
use crate::Fence;
use ash::vk;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::imp::serial::Serial;

// TODO: The GPUWeb definition of a Fence seems to be more akin to a Vulkan event, but this
//       implementation doesn't match GPUWeb anyway. Also, MokenVK doesn't support events
//       so we'll use this gimmick implementation for now.

impl Fence {
    pub fn reset(&self) -> Result<(), vk::Result> {
        *self.inner.serial.lock() = get_serial_to_wait_for(&self.inner.device);
        Ok(())
    }

    pub fn wait(&self, timeout: Duration) -> Result<(), vk::Result> {
        let timeout = Instant::now() + timeout;
        let serial = *self.inner.serial.lock();
        while serial > self.inner.device.state.lock().get_last_completed_serial() {
            self.inner.device.tick()?;
            if Instant::now() >= timeout {
                return Err(vk::Result::TIMEOUT);
            }
        }
        Ok(())
    }
}

impl FenceInner {
    pub fn new(device: Arc<DeviceInner>) -> Result<FenceInner, vk::Result> {
        let serial = {
            Mutex::new(get_serial_to_wait_for(&device))
        };
        Ok(FenceInner { serial, device })
    }
}

fn get_serial_to_wait_for(device: &DeviceInner) -> Serial {
    let state = device.state.lock();
    state.get_next_pending_serial()
}

impl Into<Fence> for FenceInner {
    fn into(self) -> Fence {
        Fence { inner: self }
    }
}
