use crate::imp::{InstanceInner, SurfaceInner};
use crate::Surface;

use ash::vk;

use parking_lot::Mutex;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;

impl SurfaceInner {
    pub fn new_win32(instance: Arc<InstanceInner>, hwnd: *const c_void) -> Result<SurfaceInner, vk::Result> {
        let create_info = vk::Win32SurfaceCreateInfoKHR::builder().hwnd(hwnd);
        let handle = unsafe {
            instance
                .raw_ext
                .surface_win32
                .create_win32_surface(&create_info, None)?
        };
        let supported_formats = Mutex::new(HashMap::default());
        Ok(SurfaceInner {
            instance,
            handle,
            supported_formats,
        })
    }

    /// Recipe: _Selecting a format of swapchain images_ (page `101`)
    pub fn is_supported_format(
        &self,
        physical_device: vk::PhysicalDevice,
        requested_format: vk::SurfaceFormatKHR,
    ) -> Result<bool, vk::Result> {
        // Querying the supported formats is slow and causes swapchain re-creation to stutter,
        // so we cache these after initial lookup
        let mut supported_formats_guard = self.supported_formats.lock();
        if !supported_formats_guard.contains_key(&physical_device) {
            supported_formats_guard.insert(physical_device, unsafe {
                self.instance
                    .raw_ext
                    .surface
                    .get_physical_device_surface_formats(physical_device, self.handle)
            }?);
        }
        let supported_formats = &supported_formats_guard[&physical_device];
        let valid_formats = [requested_format.format, vk::Format::UNDEFINED];
        for format in supported_formats.iter().cloned() {
            if valid_formats.contains(&format.format) && requested_format.color_space == format.color_space {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Into<Surface> for SurfaceInner {
    fn into(self) -> Surface {
        Surface { inner: Arc::new(self) }
    }
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe {
            self.instance.raw_ext.surface.destroy_surface(self.handle, None);
        }
    }
}
