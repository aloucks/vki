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
            hwnd: $window.get_hwnd(),
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
            xlib_window: $window.get_xlib_window(),
            xlib_display: $window.get_xlib_display(),
            xcb_connection: $window.get_xcb_connection(),
            xcb_window: $window.get_xlib_window(),
            wayland_surface: $window.get_wayland_surface(),
            wayland_display: $window.get_wayland_display(),
        }
    }};
}
