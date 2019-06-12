// TODO: Figure out a better way to have shared code for the integration tests. Currently, this
//       module is declared repeatedly and any unused functions for a particular test suite are
//       reported as unused.
#![allow(dead_code)]

use vki::{winit_surface_descriptor, PowerPreference};
use vki::{
    Adapter, AdapterOptions, Device, DeviceDescriptor, Instance, Surface, Swapchain, SwapchainDescriptor,
    TextureFormat, TextureUsageFlags,
};

use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::Window;

/// Setup validation and logging. This is called automatically
/// by `init` and `init_with_window`.
pub fn init_environment() {
    // NOTE: This is currently *also* set via instance creation, but that should probably
    //       be configurable. However, we *always* want it enabled for tests.
    std::env::set_var("VK_INSTANCE_LAYERS", "VK_LAYER_LUNARG_standard_validation");

    let _ = pretty_env_logger::try_init();
}

fn select_power_preference() -> PowerPreference {
    use std::str::FromStr;
    let test_integrated_gpu = std::env::var("TEST_INTEGRATED_GPU").unwrap_or("false".to_owned());
    if bool::from_str(test_integrated_gpu.as_str()).unwrap_or(false) {
        PowerPreference::LowPower
    } else if i32::from_str(test_integrated_gpu.as_str()).unwrap_or(0) > 0 {
        PowerPreference::LowPower
    } else {
        PowerPreference::HighPerformance
    }
}

pub fn init() -> Result<(Instance, Adapter, Device), Box<dyn std::error::Error>> {
    init_environment();
    let power_preference = select_power_preference();
    log::debug!("power_preference: {:?}", power_preference);
    let instance = Instance::new()?;
    let adapter = instance.get_adapter(&AdapterOptions { power_preference })?;
    let device = adapter.create_device(&DeviceDescriptor::default())?;

    Ok((instance, adapter, device))
}

pub fn init_with_window(
    window: &Window,
) -> Result<(Instance, Adapter, Device, Surface, Swapchain), Box<dyn std::error::Error>> {
    init_environment();
    let instance = Instance::new()?;
    let adapter = instance.get_adapter(&AdapterOptions::default())?;
    let surface_descriptor = winit_surface_descriptor!(window);
    let surface = instance.create_surface(&surface_descriptor)?;
    let device = adapter.create_device(&DeviceDescriptor::default().with_surface_support(&surface))?;
    let swapchain_descriptor = swapchain_descriptor(&surface);
    let swapchain = device.create_swapchain(&swapchain_descriptor, None)?;

    Ok((instance, adapter, device, surface, swapchain))
}

pub fn swapchain_descriptor<'a>(surface: &'a Surface) -> SwapchainDescriptor<'a> {
    SwapchainDescriptor {
        surface,
        format: TextureFormat::B8G8R8A8Unorm,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    }
}

pub fn headless_window() -> Result<(EventLoop<()>, Window), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_inner_size(LogicalSize {
            width: 1024 as _,
            height: 768 as _,
        })
        .with_visible(false)
        .build(&event_loop)?;

    Ok((event_loop, window))
}
