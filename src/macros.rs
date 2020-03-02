macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const std::os::raw::c_char
    };
}

/// Create a `SurfaceDescriptor` from a Winit `Window`.
///
/// # Winit Note
///
/// Winit is in the process of a major API refactor. If using winit `v0.19`, the `winit-eventloop-2`
/// feature needs to be disabled.
#[macro_export]
#[doc(hidden)]
#[deprecated(note = "Use SurfaceDescriptor::from_window")]
macro_rules! winit_surface_descriptor {
    ($window:expr) => {{
        use raw_window_handle::HasRawWindowHandle;
        $crate::SurfaceDescriptor::from_window($window)
    }};
}

/// Create a `SurfaceDescriptor` from a GLFW `Window`.
///
/// # MacOS Note
///
/// The `objc` crate must be added to `Cargo.toml` in addition to `glfw`. If compile errors
/// arise on the `sel!` macro from the `objc` crate, try importing it with rust 2015 syntax:
///
/// ```
/// #[cfg(target_os = "macos")]
/// #[macro_use]
/// extern crate objc;
/// ```
#[macro_export]
#[doc(hidden)]
#[deprecated(note = "Use SurfaceDescriptor::from_window")]
macro_rules! glfw_surface_descriptor (
    ($window:expr) => {{
        use raw_window_handle::HasRawWindowHandle;
        $crate::SurfaceDescriptor::from_window($window)
    }};
);
