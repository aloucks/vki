use vki::{DeviceDescriptor, Instance, RequestAdapterOptions, SwapchainDescriptor, TextureFormat, TextureUsageFlags};

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::desktop::EventLoopExtDesktop;
use winit::platform::windows::WindowExtWindows;

use std::time::{Duration, Instant};

fn main() -> Result<(), Box<std::error::Error>> {
    std::env::set_var("VK_INSTANCE_LAYERS", "VK_LAYER_LUNARG_standard_validation");

    let _ = pretty_env_logger::try_init();

    let mut event_loop = EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("swapchain.rs")
        .with_dimensions(LogicalSize::new(1024 as _, 768 as _))
        .build(&event_loop)?;

    let hwnd = window.get_hwnd();

    let instance = Instance::new()?;

    let adapter = instance.request_adaptor(RequestAdapterOptions::default())?;
    println!("{:?}", adapter);

    let surface = instance.create_surface_win32(hwnd)?;

    let device_desc = DeviceDescriptor::default().with_surface_support(&surface);

    let device = adapter.create_device(device_desc)?;
    println!("{:?}", device);

    let swapchain_desc = SwapchainDescriptor {
        surface: &surface,
        format: TextureFormat::B8G8R8A8UnormSRGB,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    };

    let mut swapchain = device.create_swapchain(swapchain_desc, None)?;

    event_loop.run_return(|event, _target, control_flow| {
        let mut do_event = || {
            match event {
                Event::NewEvents(_) => {
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    ..
                } => {
                    swapchain = device.create_swapchain(swapchain_desc, Some(&swapchain))?;
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    let frame = swapchain.acquire_next_image()?;
                    //println!("new frame; time: {:?}", Instant::now());
                    let queue = device.get_queue();
                    queue.present(frame)?;
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16));
                }
                _ => {}
            }
            Ok(())
        };
        let result: Result<(), Box<std::error::Error>> = do_event();
        result.unwrap();
    });

    Ok(())
}
