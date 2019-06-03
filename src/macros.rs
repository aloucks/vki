macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const std::os::raw::c_char
    };
}

#[macro_export]
#[cfg(target_os = "windows")]
macro_rules! winit_surface_descriptor (
    ($window:expr) => ({
        #[cfg(feature="winit-eventloop-2")]
        use winit::platform::windows::WindowExtWindows;

        #[cfg(not(feature="winit-eventloop-2"))]
        use winit::os::windows::WindowExt;

        $crate::SurfaceDescriptorWin32 {
            hwnd: $window.hwnd(),
        }
    });
);

#[macro_export]
#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
macro_rules! winit_surface_descriptor {
    ($window:expr) => {{
        #[cfg(feature = "winit-eventloop-2")]
        use winit::platform::unix::WindowExtUnix;

        #[cfg(not(feature = "winit-eventloop-2"))]
        use winit::os::unix::WindowExt;

        $crate::SurfaceDescriptorUnix {
            xlib_window: $window.xlib_window(),
            xlib_display: $window.xlib_display(),
            xcb_connection: $window.xcb_connection(),
            xcb_window: $window.xlib_window(),
            wayland_surface: $window.wayland_surface(),
            wayland_display: $window.wayland_display(),
        }
    }};
}

#[macro_export]
#[cfg(target_os = "windows")]
macro_rules! glfw_surface_descriptor (
    ($window:expr) => ({
        $crate::SurfaceDescriptorWin32 {
            hwnd: $window.get_win32_window() as _,
        }
    });
);

#[macro_export]
#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
macro_rules! glfw_surface_descriptor {
    ($window:expr) => {{
        let xlib_display = unsafe {
            glfw::ffi::glfwGetX11Display()
        };
        $crate::SurfaceDescriptorUnix {
            xlib_window: Some($window.get_x11_window() as _),
            xlib_display: Some(xlib_display as _),
            xcb_connection: None,
            xcb_window: None,
            wayland_surface: None,
            wayland_display: None,
        }
    }};
}

#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = std::mem::uninitialized();
            let offset = (&b.$field as *const _ as isize) - (&b as *const _ as isize);
            std::mem::forget(b);
            offset as _
        }
    }};
}