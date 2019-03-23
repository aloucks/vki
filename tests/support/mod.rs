use vki::{
    Adapter, Device, DeviceDescriptor, Instance, RequestAdapterOptions, Surface, Swapchain, SwapchainDescriptor,
    TextureFormat, TextureUsageFlags,
};

use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::platform::windows::WindowExtWindows;
use winit::window::Window;

pub fn init() -> Result<(EventLoop<()>, Window, Instance, Surface, Adapter, Device, Swapchain), Box<std::error::Error>>
{
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_dimensions(LogicalSize {
            width: 1024 as _,
            height: 768 as _,
        })
        .with_visibility(false)
        .build(&event_loop)?;

    let instance = Instance::new()?;
    let surface = instance.create_surface_win32(window.get_hwnd())?;
    let adapter = instance.request_adaptor(RequestAdapterOptions::default())?;
    let device = adapter.create_device(DeviceDescriptor::default().with_surface_support(&surface))?;

    let swapchain_descriptor = swapchain_descriptor(&surface);

    let swapchain = device.create_swapchain(swapchain_descriptor, None)?;

    Ok((event_loop, window, instance, surface, adapter, device, swapchain))
}

pub fn swapchain_descriptor<'a>(surface: &'a Surface) -> SwapchainDescriptor<'a> {
    SwapchainDescriptor {
        surface,
        format: TextureFormat::B8G8R8A8UnormSRGB,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    }
}
