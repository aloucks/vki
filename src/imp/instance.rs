use ash::extensions::{ext, khr};
use ash::version::{EntryV1_0, InstanceV1_0};
use ash::{self, vk};

use lazy_static::lazy_static;
use parking_lot::{RwLock, RwLockReadGuard};

use std::ffi::CStr;
use std::fmt;
use std::mem;
use std::sync::Arc;

use crate::imp::{debug, AdapterInner, InstanceExt, InstanceInner, SurfaceInner};
use crate::{Adapter, AdapterOptions, Error, Instance, Surface, SurfaceDescriptor};

use raw_window_handle::HasRawWindowHandle;
use std::fmt::Debug;
use std::sync::atomic::Ordering;

lazy_static! {
    static ref ENTRY: RwLock<Result<ash::Entry, Error>> = {
        unsafe {
            extern "C" fn unload() {
                let mut entry_guard = ENTRY.write();
                *entry_guard = Err(Error::from(String::from("Vulkan library unloaded")));
            }
            libc::atexit(unload);
            RwLock::new(ash::Entry::new().map_err(Into::into))
        }
    };
}

impl Instance {
    pub fn new() -> Result<Instance, Error> {
        let inner = InstanceInner::new()?;
        Ok(inner.into())
    }

    pub fn get_adapter(&self, options: AdapterOptions) -> Result<Adapter, Error> {
        let adapter = AdapterInner::new(self.inner.clone(), options)?;
        Ok(adapter.into())
    }

    pub fn create_surface(&self, descriptor: &SurfaceDescriptor) -> Result<Surface, Error> {
        let surface = SurfaceInner::new(self.inner.clone(), descriptor)?;
        Ok(surface.into())
    }

    pub fn create_surface_raw<W: HasRawWindowHandle>(&self, window: &W) -> Result<Surface, Error> {
        let surface = SurfaceInner::from_raw_window_handle(self.inner.clone(), window.raw_window_handle())?;
        Ok(surface.into())
    }
}

impl InstanceInner {
    #[rustfmt::skip]
    fn new() -> Result<InstanceInner, Error> {
        let init_debug_report = debug::TEST_VALIDATION_HOOK.load(Ordering::Acquire);

        unsafe {
            let entry_guard: RwLockReadGuard<Result<ash::Entry, Error>> = ENTRY.read();
            let entry: &ash::Entry = entry_guard.as_ref()?;

            let mut extension_names = vec![];

            let extension_properties = entry.enumerate_instance_extension_properties()?;

            for p in extension_properties.iter() {
                let mut include_extension = false;
                let name = CStr::from_ptr(p.extension_name.as_ptr());
                let name_cow = name.to_string_lossy();
                log::trace!("found instance extension: {}", name_cow);
                if name_cow.ends_with("surface") {
                    include_extension = true;
                }
                if name_cow == "VK_EXT_debug_report" && init_debug_report {
                    include_extension = true;
                }
                if name_cow == "VK_EXT_debug_utils" {
                    include_extension = true;
                }
                if include_extension {
                    log::debug!("requesting extension support: {}", name_cow);
                    extension_names.push(name.to_owned());
                }
            }

            let instance_layer_properties = entry.enumerate_instance_layer_properties()?;

            for p in instance_layer_properties.iter() {
                let name = CStr::from_ptr(p.layer_name.as_ptr());
                log::trace!("found instance layer: {}", name.to_string_lossy());
            }

            let app_info = vk::ApplicationInfo::builder()
                .api_version(ash::vk_make_version!(1, 0, 0));

            let layer_names = vec![
                #[cfg(debug_assertions)]
                c_str!("VK_LAYER_LUNARG_standard_validation")
            ];

            for layer_name in layer_names.iter() {
                let requested_layer_name = CStr::from_ptr(*layer_name);
                let is_available = instance_layer_properties.iter().any(|p| {
                    let name = CStr::from_ptr(p.layer_name.as_ptr());
                    name == requested_layer_name
                });
                if !is_available {
                    log::error!("requested layer unavailable: {:?}", requested_layer_name.to_string_lossy());
                }
            }

            let extension_names_ptrs: Vec<_> = extension_names.iter().map(|name| name.as_ptr()).collect();

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&extension_names_ptrs)
                .enabled_layer_names(&layer_names);

            let raw = entry.create_instance(&create_info, None)?;

            let surface = khr::Surface::new(entry, &raw);

            #[cfg(windows)]
            let surface_win32 = khr::Win32Surface::new(entry, &raw);

            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            let surface_xlib = khr::XlibSurface::new(entry, &raw);

            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            let surface_xcb = khr::XcbSurface::new(entry, &raw);

            #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
            let surface_wayland = khr::WaylandSurface::new(entry, &raw);

            #[cfg(all(unix, target_os = "macos"))]
            let surface_macos= ash::extensions::mvk::MacOSSurface::new(entry, &raw);

            let debug_utils = ext::DebugUtils::new(entry, &raw);
            let debug_report = ext::DebugReport::new(entry, &raw);
            let debug_report_callback = if init_debug_report {
                let debug_report_create_info = vk::DebugReportCallbackCreateInfoEXT::builder()
                    .flags(vk::DebugReportFlagsEXT::ERROR | vk::DebugReportFlagsEXT::WARNING | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING)
                    .user_data(mem::transmute(raw.handle()))
                    .pfn_callback(Some(debug::debug_report_callback_test));
                Some(debug_report.create_debug_report_callback(&debug_report_create_info, None)?)
            } else {
                None
            };

            let raw_ext = InstanceExt {
                surface,

                #[cfg(windows)]
                surface_win32,

                #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
                surface_xlib,

                #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
                surface_xcb,

                #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
                surface_wayland,

                #[cfg(all(unix, target_os = "macos"))]
                surface_macos,

                debug_utils,
                debug_report,
            };

            Ok(InstanceInner { raw, raw_ext, extension_properties, debug_report_callback })
        }
    }

    pub fn has_extension(&self, name: &str) -> bool {
        for extension_properties in self.extension_properties.iter() {
            let ext_name = unsafe { CStr::from_ptr(extension_properties.extension_name.as_ptr()) };
            if name == ext_name.to_str().unwrap() {
                return true;
            }
        }
        false
    }
}

impl Into<Instance> for InstanceInner {
    fn into(self) -> Instance {
        Instance { inner: Arc::new(self) }
    }
}

impl Debug for InstanceInner {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self.raw.handle())
    }
}

impl Drop for InstanceInner {
    fn drop(&mut self) {
        unsafe {
            if let Some(debug_report_callback) = self.debug_report_callback {
                self.raw_ext
                    .debug_report
                    .destroy_debug_report_callback(debug_report_callback, None);
            }
            self.raw.destroy_instance(None);
        }
    }
}
