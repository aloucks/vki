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
        {
            use winit::platform::windows::WindowExtWindows;

            $crate::SurfaceDescriptorWin32 {
                hwnd: $window.hwnd(),
            }
        }

        #[cfg(not(feature="winit-eventloop-2"))]
        {
            use winit::os::windows::WindowExt;

            $crate::SurfaceDescriptorWin32 {
                hwnd: $window.get_hwnd(),
            }
        }
    });
);

#[macro_export]
#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
macro_rules! winit_surface_descriptor {
    ($window:expr) => {{
        #[cfg(feature = "winit-eventloop-2")]
        {
            use winit::platform::unix::WindowExtUnix;
            $crate::SurfaceDescriptorUnix {
                xlib_window: $window.xlib_window(),
                xlib_display: $window.xlib_display(),
                xcb_connection: $window.xcb_connection(),
                xcb_window: $window.xlib_window(),
                wayland_surface: $window.wayland_surface(),
                wayland_display: $window.wayland_display(),
            }
        }

        #[cfg(not(feature = "winit-eventloop-2"))]
        {
            use winit::os::unix::WindowExt;

            $crate::SurfaceDescriptorUnix {
                xlib_window: $window.get_xlib_window(),
                xlib_display: $window.get_xlib_display(),
                xcb_connection: $window.get_xcb_connection(),
                xcb_window: $window.get_xlib_window(),
                wayland_surface: $window.get_wayland_surface(),
                wayland_display: $window.get_wayland_display(),
            }
        }
    }};
}

#[macro_export]
#[cfg(all(unix, target_os = "macos"))]
macro_rules! winit_surface_descriptor {
    ($window:expr) => {{
        #[cfg(feature = "winit-eventloop-2")]
        {
            use winit::platform::macos::WindowExtMacOS;
            $crate::SurfaceDescriptorMacOS {
                nsview: $window.nsview(),
            }
        }

        #[cfg(not(feature = "winit-eventloop-2"))]
        {
            use winit::os::macos::WindowExt;

            $crate::SurfaceDescriptorMacOS {
                nsview: $window.get_nsview(),
            }
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
        let xlib_display = unsafe { glfw::ffi::glfwGetX11Display() };
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
#[cfg(all(unix, target_os = "macos"))]
macro_rules! glfw_surface_descriptor (
    ($window:expr) => {{
        // https://stackoverflow.com/questions/7566882/how-to-get-current-nsview-in-cocoa
        // TODO: Verify that this works!
        let ns_object: *mut $crate::objc::runtime::Object = $window.get_cocoa_window() as *mut _;
        let ns_view: *mut $crate::objc::runtime::Object = $crate::objc::msg_send![ns_object, contentView];
        assert_ne!(ns_view, std::ptr::null_mut());
        $crate::SurfaceDescriptorMacOS {
            nsview: ns_view as *const _,
        }
    }};
);

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
