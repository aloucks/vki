use crate::imp::{InstanceInner, SurfaceInner};
use crate::Surface;

use ash::vk;

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
        Ok(SurfaceInner { instance, handle })
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
