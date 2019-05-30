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
