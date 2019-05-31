use vki::winit_surface_descriptor;
use vki::{AdapterOptions, DeviceDescriptor, Instance};

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;
use winit::platform::desktop::EventLoopExtDesktop;

pub mod support;

// TODO: Concurrent EventLoop creation hangs or segfaults in winit on x11
#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    static ref LOCK: std::sync::Mutex::<()> = std::sync::Mutex::new(());
}

#[test]
fn create_swapchain() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        support::init_environment();
        let (_event_loop, window) = support::headless_window()?;
        let instance = Instance::new()?;
        let adapter = instance.get_adapter(AdapterOptions::default())?;
        let surface_descriptor = winit_surface_descriptor!(window);
        let surface = instance.create_surface(&surface_descriptor)?;
        let device = adapter.create_device(DeviceDescriptor::default().with_surface_support(&surface))?;
        let swapchain_descriptor = support::swapchain_descriptor(&surface);

        let _swapchain = device.create_swapchain(swapchain_descriptor, None)?;

        Ok(instance)
    });
}

#[test]
fn recreate_swapchain_without_old() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        support::init_environment();
        let (_event_loop, window) = support::headless_window()?;
        let instance = Instance::new()?;
        let adapter = instance.get_adapter(AdapterOptions::default())?;
        let surface_descriptor = winit_surface_descriptor!(window);
        let surface = instance.create_surface(&surface_descriptor)?;
        let device = adapter.create_device(DeviceDescriptor::default().with_surface_support(&surface))?;
        let swapchain_descriptor = support::swapchain_descriptor(&surface);

        let swapchain = device.create_swapchain(swapchain_descriptor, None)?;

        // TODO: This test explicitly invalidates the previous swapchain, There currently
        //       isn't any way to have that previous swapchain *implicitly* invalidated,
        //       when passing `None` as "old_swapchain". Without this drop, the re-creation
        //       below may fail (horribly with a segmentation fault).
        drop(swapchain);

        let _swapchain = device.create_swapchain(swapchain_descriptor, None)?;

        Ok(instance)
    });
}

#[test]
fn acquire_next_image() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (_event_loop, window) = support::headless_window()?;
        let (instance, _adapter, _device, _surface, swapchain) = support::init_with_window(&window)?;

        let _frame = swapchain.acquire_next_image()?;

        Ok(instance)
    });
}

#[test]
fn present() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (_event_loop, window) = support::headless_window()?;
        let (instance, _adapter, device, _surface, swapchain) = support::init_with_window(&window)?;

        let frame = swapchain.acquire_next_image()?;

        let queue = device.get_queue();
        queue.present(frame)?;

        Ok(instance)
    });
}

#[test]
#[cfg_attr(target_os = "linux", ignore)] // TODO: winit eventloop-2.0 is eating events right now
fn recreate_after_resize() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (mut event_loop, window) = support::headless_window()?;
        let (instance, _adapter, device, surface, mut swapchain) = support::init_with_window(&window)?;

        let queue = device.get_queue();

        let frame = swapchain.acquire_next_image()?;

        queue.present(frame)?;

        let new_size = LogicalSize {
            width: 800 as _,
            height: 600 as _,
        };

        let old_size = window.inner_size();
        assert_ne!(new_size, old_size);

        let mut resized = false;
        window.set_inner_size(new_size);
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

        queue.present(frame)?;

        Ok(instance)
    });
}

#[test]
fn keep_surface_alive() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (_event_loop, window) = support::headless_window()?;
        let (instance, _adapter, device, surface, swapchain) = support::init_with_window(&window)?;

        // Assert that the swapchain keeps the surface alive until after the swapchain is destroyed
        drop(surface);

        let frame = swapchain.acquire_next_image()?;
        let queue = device.get_queue();
        queue.present(frame)?;

        Ok(instance)
    });
}
