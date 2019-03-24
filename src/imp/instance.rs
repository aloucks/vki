use ash::extensions::{ext, khr};
use ash::version::{EntryV1_0, InstanceV1_0};
use ash::{self, vk};

use lazy_static::lazy_static;
use parking_lot::{RwLock, RwLockReadGuard};

use std::ffi::{c_void, CStr};
use std::fmt;
use std::mem;
use std::sync::Arc;

use crate::imp::{debug, AdapterInner, InstanceExt, InstanceInner, SurfaceInner};
use crate::{Adapter, InitError, Instance, RequestAdapterOptions, Surface};

use std::fmt::Debug;
use std::sync::atomic::Ordering;

lazy_static! {
    static ref ENTRY: RwLock<Result<ash::Entry, InitError>> = {
        unsafe {
            extern "C" fn unload() {
                let mut entry_guard = ENTRY.write();
                *entry_guard = Err(InitError::Library(String::from("unload")));
            }
            libc::atexit(unload);
            RwLock::new(ash::Entry::new().map_err(|e| e.into()))
        }
    };
}

impl Instance {
    pub fn new() -> Result<Instance, InitError> {
        let inner = InstanceInner::new()?;
        Ok(inner.into())
    }

    pub fn request_adaptor(&self, options: RequestAdapterOptions) -> Result<Adapter, vk::Result> {
        let adapter = AdapterInner::new(self.inner.clone(), options)?;
        Ok(adapter.into())
    }

    pub fn create_surface_win32(&self, hwnd: *const c_void) -> Result<Surface, vk::Result> {
        let surface = SurfaceInner::new_win32(self.inner.clone(), hwnd)?;
        Ok(surface.into())
    }
}

impl InstanceInner {
    #[rustfmt::skip]
    fn new() -> Result<InstanceInner, InitError> {
        unsafe {
            let entry_guard: RwLockReadGuard<Result<ash::Entry, InitError>> = ENTRY.read();
            let entry: &ash::Entry = entry_guard.as_ref()?;

            for p in entry.enumerate_instance_extension_properties()?.iter() {
                let name = CStr::from_ptr(p.extension_name.as_ptr());
                log::debug!("found instance extension: {}", name.to_string_lossy());
            }

            for p in entry.enumerate_instance_layer_properties()?.iter() {
                let name = CStr::from_ptr(p.layer_name.as_ptr());
                log::debug!("found instance layer: {}", name.to_string_lossy());
            }

            let app_info = vk::ApplicationInfo::builder()
                .api_version(ash::vk_make_version!(1, 0, 0));

            let extension_names = [
                c_str!("VK_KHR_surface"),
                #[cfg(windows)]
                c_str!("VK_KHR_win32_surface"),
                c_str!("VK_EXT_debug_report"),
            ];

            let layer_names = [
                #[cfg(debug_assertions)]
                c_str!("VK_LAYER_LUNARG_standard_validation")
            ];

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&extension_names)
                .enabled_layer_names(&layer_names);

            let raw = entry.create_instance(&create_info, None)?;

            let surface = khr::Surface::new(entry, &raw);

            #[cfg(windows)]
            let surface_win32 = khr::Win32Surface::new(entry, &raw);

            let debug_utils = ext::DebugUtils::new(entry, &raw);
            let debug_report = ext::DebugReport::new(entry, &raw);

            let init_debug_report = debug::TEST_VALIDATION_HOOK.load(Ordering::Acquire);
            let debug_report_callback = if init_debug_report {
                let debug_report_create_info = vk::DebugReportCallbackCreateInfoEXT::builder()
                    .flags(vk::DebugReportFlagsEXT::ERROR)
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
                debug_utils,
                debug_report,
            };

            Ok(InstanceInner { raw, raw_ext, debug_report_callback })
        }
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
