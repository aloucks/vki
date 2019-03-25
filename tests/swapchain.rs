use vki::{DeviceDescriptor, Instance, RequestAdapterOptions, SwapchainDescriptor, TextureFormat, TextureUsageFlags};

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;
use winit::platform::desktop::EventLoopExtDesktop;
use winit::platform::windows::WindowExtWindows;

mod support;

#[test]
fn create_swapchain() {
    let _ = pretty_env_logger::try_init();
    vki::validate(|| {
        let instance = Instance::new()?;

        let event_loop = winit::event_loop::EventLoop::new();
        let window = winit::window::WindowBuilder::new()
            .with_dimensions(LogicalSize {
                width: 1024 as _,
                height: 768 as _,
            })
            .with_visibility(false)
            .build(&event_loop)?;

        let surface = instance.create_surface_win32(window.get_hwnd())?;
        let adapter = instance.request_adaptor(RequestAdapterOptions::default())?;
        let device = adapter.create_device(DeviceDescriptor::default().with_surface_support(&surface))?;

        let swapchain_descriptor = SwapchainDescriptor {
            surface: &surface,
            format: TextureFormat::B8G8R8A8UnormSRGB,
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
        };

        let _swapchain = device.create_swapchain(swapchain_descriptor, None)?;

        Ok(instance)
    });
}

#[test]
fn acquire_next_image() {
    vki::validate(|| {
        let (_event_loop, _window, instance, _surface, _adapter, _device, swapchain) = support::init()?;
        let _frame = swapchain.acquire_next_image()?;

        Ok(instance)
    });
}

#[test]
fn present() {
    vki::validate(|| {
        let (_event_loop, _window, instance, _surface, _adapter, device, swapchain) = support::init()?;
        let frame = swapchain.acquire_next_image()?;

        let queue = device.get_queue();
        queue.present(frame)?;

        Ok(instance)
    });
}

#[test]
fn recreate_after_resize() {
    vki::validate(|| {
        let (mut event_loop, window, instance, surface, _adapter, device, mut swapchain) = support::init()?;

        let frame = swapchain.acquire_next_image()?;
        let queue = device.get_queue();
        queue.present(frame)?;

        let mut resized = false;
        window.set_inner_size(LogicalSize {
            width: 800 as _,
            height: 600 as _,
        });
        event_loop.run_return(|event, _target, control_flow| match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                let swapchain_descriptor = support::swapchain_descriptor(&surface);
                swapchain = device
                    .create_swapchain(swapchain_descriptor, Some(&swapchain))
                    .expect("Failed to re-create swapchain");
                resized = true;
            }
            Event::EventsCleared => *control_flow = ControlFlow::Exit,
            _ => {}
        });
        assert_eq!(true, resized);

        let frame = swapchain.acquire_next_image()?;
        let queue = device.get_queue();
        queue.present(frame)?;

        Ok(instance)
    });
}

#[test]
fn keep_surface_alive() {
    vki::validate(|| {
        let (mut event_loop, window, instance, surface, _adapter, device, swapchain) = support::init()?;

        // Assert that the swapchain keeps the surface alive until after the swapchain is destroyed
        drop(surface);

        let frame = swapchain.acquire_next_image()?;
        let queue = device.get_queue();
        queue.present(frame)?;

        Ok(instance)
    });
}
