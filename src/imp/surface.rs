use crate::error::Error;
use crate::imp::{InstanceInner, SurfaceInner};
use crate::Surface;

use ash::vk;

use parking_lot::Mutex;

use std::collections::HashMap;
use std::sync::Arc;

impl SurfaceInner {
    #[cfg(target_os = "windows")]
    pub fn new(
        instance: Arc<InstanceInner>,
        descriptor: &crate::SurfaceDescriptorWin32,
    ) -> Result<SurfaceInner, Error> {
        let create_info = vk::Win32SurfaceCreateInfoKHR {
            hwnd: descriptor.hwnd,
            ..Default::default()
        };

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

    #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
    pub fn new(instance: Arc<InstanceInner>, descriptor: &crate::SurfaceDescriptorUnix) -> Result<SurfaceInner, Error> {
        let supported_formats = Mutex::new(HashMap::default());

        if let (Some(xlib_window), Some(xlib_display)) = (descriptor.xlib_window, descriptor.xlib_display) {
            let create_info = vk::XlibSurfaceCreateInfoKHR {
                window: xlib_window as _,
                dpy: xlib_display as _,
                ..Default::default()
            };

            let handle = unsafe { instance.raw_ext.surface_xlib.create_xlib_surface(&create_info, None)? };

            return Ok(SurfaceInner {
                instance,
                handle,
                supported_formats,
            });
        }

        if let (Some(xcb_window), Some(xcb_connection)) = (descriptor.xcb_window, descriptor.xcb_connection) {
            let create_info = vk::XcbSurfaceCreateInfoKHR {
                window: xcb_window as _,
                connection: xcb_connection as _,
                ..Default::default()
            };

            let handle = unsafe { instance.raw_ext.surface_xcb.create_xcb_surface(&create_info, None)? };

            return Ok(SurfaceInner {
                instance,
                handle,
                supported_formats,
            });
        }

        if let (Some(wayland_display), Some(wayland_surface)) = (descriptor.wayland_display, descriptor.wayland_surface)
        {
            let create_info = vk::WaylandSurfaceCreateInfoKHR {
                surface: wayland_surface as _,
                display: wayland_display as _,
                ..Default::default()
            };

            let handle = unsafe {
                instance
                    .raw_ext
                    .surface_wayland
                    .create_wayland_surface(&create_info, None)?
            };

            return Ok(SurfaceInner {
                instance,
                handle,
                supported_formats,
            });
        }

        log::error!("invalid surface descriptor: {:?}", descriptor);
        Err(Error::from("Invalid surface descriptor"))
    }

    /// Recipe: _Selecting a format of swapchain images_ (page `101`)
    pub fn is_supported_format(
        &self,
        physical_device: vk::PhysicalDevice,
        requested_format: vk::SurfaceFormatKHR,
    ) -> Result<bool, Error> {
        let supported_formats = self.get_physical_device_surface_formats(physical_device)?;
        let valid_formats = [requested_format.format, vk::Format::UNDEFINED];
        for format in supported_formats.iter().cloned() {
            if valid_formats.contains(&format.format) && requested_format.color_space == format.color_space {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn get_physical_device_surface_formats(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Vec<vk::SurfaceFormatKHR>, Error> {
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
        Ok(supported_formats_guard[&physical_device].clone())
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
