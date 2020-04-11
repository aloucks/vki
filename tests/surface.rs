#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use vki::{AdapterOptions, DeviceDescriptor, Instance};

pub mod support;

#[test]
fn winit_surface() {
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;
        let adapter = instance.request_adapter(AdapterOptions::default())?;

        let event_loop = support::new_event_loop();
        let window = winit::window::WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize {
                width: 1024 as _,
                height: 768 as _,
            })
            .with_visible(false)
            .build(&event_loop)?;

        let surface = instance.create_surface(&window)?;
        let _device = adapter.create_device(DeviceDescriptor::default().with_surface_support(&surface))?;

        Ok(instance)
    });
}

#[test]
fn glfw_surface() {
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;
        let adapter = instance.request_adapter(AdapterOptions::default())?;

        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
        glfw.window_hint(glfw::WindowHint::Visible(false));

        let (window, _receiver) = glfw
            .create_window(800, 600, "GLFW", glfw::WindowMode::Windowed)
            .unwrap();

        let surface = instance.create_surface(&window)?;
        let _device = adapter.create_device(DeviceDescriptor::default().with_surface_support(&surface))?;

        Ok(instance)
    });
}
