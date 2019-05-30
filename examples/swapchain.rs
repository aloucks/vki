use vki::{
    AdapterOptions, DeviceDescriptor, Instance, SwapchainDescriptor, SwapchainError, TextureFormat, TextureUsageFlags,
};

use winit::dpi::LogicalSize;
use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::desktop::EventLoopExtDesktop;

use std::time::{Duration, Instant};

fn main() -> Result<(), Box<std::error::Error>> {
    if std::env::var("VK_INSTANCE_LAYERS").is_err() {
        std::env::set_var("VK_INSTANCE_LAYERS", "VK_LAYER_LUNARG_standard_validation");
    }

    let _ = pretty_env_logger::try_init();

    let mut event_loop = EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("swapchain.rs")
        .with_inner_size(LogicalSize::from((800, 600)))
        .with_visibility(false)
        .build(&event_loop)?;

    let instance = Instance::new()?;
    let adapter_options = AdapterOptions::default();
    let adapter = instance.get_adapter(adapter_options)?;
    println!("Adapter: {:#?}", adapter.properties());

    // The winit_surface_descriptor macro is optional. It creates the platform specific
    // descriptor without vki requiring a dependency on winit (or a specific version of winit).
    //
    // On windows, this evaluates to:
    //
    // SurfaceDescriptorWin32 {
    //    hwnd: window.get_hwnd()
    // }
    let surface_descriptor = vki::winit_surface_descriptor!(&window);

    let surface = instance.create_surface(&surface_descriptor)?;

    let device_desc = DeviceDescriptor::default().with_surface_support(&surface);
    let device = adapter.create_device(device_desc)?;

    let formats = device.get_supported_swapchain_formats(&surface)?;
    println!("Supported swapchain formats: {:?}", formats);

    let swapchain_format = TextureFormat::B8G8R8A8Unorm;
    assert!(formats.contains(&swapchain_format), "Unsupported swapchain format");

    let swapchain_desc = SwapchainDescriptor {
        surface: &surface,
        format: swapchain_format,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    };

    let mut swapchain = device.create_swapchain(swapchain_desc, None)?;

    window.set_visible(true);

    event_loop.run_return(|event, _target, control_flow| {
        let mut handle_event = || {
            match event {
                Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                    const TARGET_FRAMERATE: u64 = 60;
                    let max_wait_millis = 1000 / TARGET_FRAMERATE;
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(max_wait_millis));
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
                    // Note that the swapchain should not be re-created when the window width
                    // or height is equal to zero. This can happen during a resize or when
                    // the window is minimized. We do not handle this case here, but it is
                    // handled in the examples framework. Minimizing this window should result
                    // in a validation error.
                    swapchain = device.create_swapchain(swapchain_desc, Some(&swapchain))?;
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // println!("new frame; time: {:?}", Instant::now());

                    // End the frame early if the swapchain is out of date and hasn't been re-created yet
                    let frame = match swapchain.acquire_next_image() {
                        Ok(frame) => frame,
                        Err(SwapchainError::OutOfDate) => return Ok(()),
                        Err(e) => return Err(e)?,
                    };

                    // Record drawing commands here!

                    let queue = device.get_queue();

                    // Submit drawing commands here!

                    // Drop the frame if the swapchain is out of date and hasn't been re-created yet
                    match queue.present(frame) {
                        Ok(()) => {}
                        Err(SwapchainError::OutOfDate) => return Ok(()),
                        Err(e) => return Err(e)?,
                    }
                }
                _ => {}
            }
            Ok(())
        };
        let result: Result<(), Box<std::error::Error>> = handle_event();
        result.expect("event loop error");
    });

    Ok(())
}
