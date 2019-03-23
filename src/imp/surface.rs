use crate::imp::{InstanceInner, SurfaceInner};
use crate::Surface;

use ash::vk;

use std::ffi::c_void;
use std::sync::Arc;

impl Surface {
    pub(crate) fn new_win32(instance: Arc<InstanceInner>, hwnd: *const c_void) -> Result<Surface, vk::Result> {
        unsafe {
            let create_info = vk::Win32SurfaceCreateInfoKHR::builder().hwnd(hwnd);
            let handle = instance
                .raw_ext
                .surface_win32
                .create_win32_surface(&create_info, None)?;
            let inner = SurfaceInner { instance, handle };

            Ok(Surface { inner: Arc::new(inner) })
        }
    }
}

impl Drop for SurfaceInner {
    fn drop(&mut self) {
        unsafe {
            self.instance.raw_ext.surface.destroy_surface(self.handle, None);
        }
    }
}
