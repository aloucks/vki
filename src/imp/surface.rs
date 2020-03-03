use std::collections::HashMap;
use std::sync::Arc;

use ash::vk;
use parking_lot::Mutex;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::error::Error;
use crate::imp::{InstanceInner, SurfaceInner};
use crate::Surface;

impl SurfaceInner {
    pub fn from_raw_window_handle(
        instance: Arc<InstanceInner>,
        handle: RawWindowHandle,
    ) -> Result<SurfaceInner, Error> {
        match handle {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Windows(raw) => SurfaceInner::new(
                instance,
                &SurfaceDescriptorWin32 {
                    hwnd: raw.hwnd,
                    hinstance: raw.hinstance,
                },
            ),
            #[cfg(all(unix, target_os = "macos"))]
            RawWindowHandle::MacOS(raw) => SurfaceInner::new(instance, &SurfaceDescriptorMacOS { nsview: raw.ns_view }),
            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            RawWindowHandle::Xlib(raw) => SurfaceInner::new(
                instance,
                &SurfaceDescriptorUnix {
                    xlib_window: Some(raw.window),
                    xlib_display: Some(raw.display),
                    xcb_window: None,
                    xcb_connection: None,
                    wayland_surface: None,
                    wayland_display: None,
                },
            ),
            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            RawWindowHandle::Xcb(raw) => SurfaceInner::new(
                instance,
                &SurfaceDescriptorUnix {
                    xlib_window: None,
                    xlib_display: None,
                    xcb_window: Some(raw.window as _),
                    xcb_connection: Some(raw.connection),
                    wayland_surface: None,
                    wayland_display: None,
                },
            ),
            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            RawWindowHandle::Wayland(raw) => SurfaceInner::new(
                instance,
                &SurfaceDescriptorUnix {
                    xlib_window: None,
                    xlib_display: None,
                    xcb_window: None,
                    xcb_connection: None,
                    wayland_surface: Some(raw.surface),
                    wayland_display: Some(raw.display),
                },
            ),
            _ => unimplemented!(),
        }
    }

    #[cfg(target_os = "windows")]
    fn new(instance: Arc<InstanceInner>, descriptor: &SurfaceDescriptorWin32) -> Result<SurfaceInner, Error> {
        let create_info = vk::Win32SurfaceCreateInfoKHR {
            hwnd: descriptor.hwnd,
            hinstance: descriptor.hinstance,
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

    #[cfg(all(unix, target_os = "macos"))]
    fn new(instance: Arc<InstanceInner>, descriptor: &SurfaceDescriptorMacOS) -> Result<SurfaceInner, Error> {
        let create_info = vk::MacOSSurfaceCreateInfoMVK {
            p_view: descriptor.nsview,
            ..Default::default()
        };

        // TODO: SurfaceDescriptorMacOS
        // https://github.com/gfx-rs/gfx/blob/416d9ff65cd4559dcda5e640d7baf79e606be4e8/src/backend/metal/src/lib.rs#L245
        // https://github.com/glfw/glfw/blob/d834f01ca43c0f5ddd31b00a7fc2f48abbafa3da/src/cocoa_window.m#L1694

        let handle = unsafe {
            instance
                .raw_ext
                .surface_macos
                .create_mac_os_surface_mvk(&create_info, None)?
        };

        let supported_formats = Mutex::new(HashMap::default());

        Ok(SurfaceInner {
            instance,
            handle,
            supported_formats,
        })
    }

    #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
    fn new(instance: Arc<InstanceInner>, descriptor: &SurfaceDescriptorUnix) -> Result<SurfaceInner, Error> {
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct SurfaceDescriptorWin32 {
    pub hwnd: *const std::ffi::c_void,
    pub hinstance: *const std::ffi::c_void,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct SurfaceDescriptorMacOS {
    pub nsview: *const std::ffi::c_void,
}

unsafe impl Send for SurfaceDescriptorWin32 {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct SurfaceDescriptorUnix {
    pub xlib_window: Option<std::os::raw::c_ulong>,
    pub xlib_display: Option<*mut std::ffi::c_void>,
    pub xcb_window: Option<std::os::raw::c_ulong>,
    pub xcb_connection: Option<*mut std::ffi::c_void>,
    pub wayland_surface: Option<*mut std::ffi::c_void>,
    pub wayland_display: Option<*mut std::ffi::c_void>,
}

unsafe impl Send for SurfaceDescriptorUnix {}

#[cfg(windows)]
type SurfaceDescriptor = SurfaceDescriptorWin32;

#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
type SurfaceDescriptor = SurfaceDescriptorUnix;

#[cfg(all(unix, target_os = "macos"))]
type SurfaceDescriptor = SurfaceDescriptorMacOS;

impl SurfaceDescriptor {
    pub fn from_window<W: HasRawWindowHandle>(window: &W) -> SurfaceDescriptor {
        window.raw_window_handle().into()
    }
}

impl From<RawWindowHandle> for SurfaceDescriptor {
    fn from(handle: RawWindowHandle) -> SurfaceDescriptor {
        match handle {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Windows(raw) => SurfaceDescriptorWin32 {
                hwnd: raw.hwnd,
                hinstance: raw.hinstance,
            },
            #[cfg(all(unix, target_os = "macos"))]
            RawWindowHandle::MacOS(raw) => SurfaceDescriptorMacOS { nsview: raw.ns_view },
            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            RawWindowHandle::Xlib(raw) => SurfaceDescriptorUnix {
                xlib_window: Some(raw.window),
                xlib_display: Some(raw.display),
                xcb_window: None,
                xcb_connection: None,
                wayland_surface: None,
                wayland_display: None,
            },
            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            RawWindowHandle::Xcb(raw) => SurfaceDescriptorUnix {
                xlib_window: None,
                xlib_display: None,
                xcb_window: Some(raw.window as _),
                xcb_connection: Some(raw.connection),
                wayland_surface: None,
                wayland_display: None,
            },
            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            RawWindowHandle::Wayland(raw) => SurfaceDescriptorUnix {
                xlib_window: None,
                xlib_display: None,
                xcb_window: None,
                xcb_connection: None,
                wayland_surface: Some(raw.surface),
                wayland_display: Some(raw.display),
            },
            _ => panic!("unsupported window handle: {:?}", handle),
        }
    }
}
