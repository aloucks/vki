macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const std::os::raw::c_char
    };
}

#[macro_export]
#[cfg(target_os = "windows")]
macro_rules! winit_surface_descriptor (
    ($window:expr) => ({
        use winit::platform::windows::WindowExtWindows;
        let hwnd = $window.get_hwnd();
        $crate::SurfaceDescriptorWin32 {
            hwnd
        }
    });
);
