use vki::{DeviceDescriptor, Instance, RequestAdapterOptions, SwapchainDescriptor, TextureFormat, TextureUsageFlags};

use winit::dpi::LogicalSize;
use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::desktop::EventLoopExtDesktop;
use winit::platform::windows::WindowExtWindows;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<std::error::Error>> {
    std::env::set_var("VK_INSTANCE_LAYERS", "VK_LAYER_LUNARG_standard_validation");

    let _ = pretty_env_logger::try_init();

    let mut event_loop = EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("swapchain.rs")
        .with_dimensions(LogicalSize::new(1024 as _, 768 as _))
        .with_visibility(false)
        .build(&event_loop)?;

    let instance = Instance::new()?;
    let adapter_options = RequestAdapterOptions::default();
    let adapter = instance.request_adaptor(adapter_options)?;
    println!("{:?}", adapter);

    let hwnd = window.get_hwnd();
    let surface = instance.create_surface_win32(hwnd)?;

    let device_desc = DeviceDescriptor::default().with_surface_support(&surface);
    let device = adapter.create_device(device_desc)?;
    println!("{:?}", device);

    let swapchain_desc = SwapchainDescriptor {
        surface: &surface,
        format: TextureFormat::B8G8R8A8UnormSRGB,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    };

    let mut swapchain = Some(device.create_swapchain(swapchain_desc, None)?);
    let mut last_frame_time = Instant::now();
    window.show();

    use std::sync::mpsc;
    use std::thread;

    let (tx1, rx1) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();

    let running = AtomicBool::new(true);
    let swapchain_in_flight = AtomicBool::new(false);

    let mut rebuild_swapchain = false;

    let device_clone = device.clone();
    let surface_clone = surface.clone();
    let join_handle = thread::spawn(move || {
        let swapchain_desc = SwapchainDescriptor {
            surface: &surface_clone,
            format: TextureFormat::B8G8R8A8UnormSRGB,
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
        };
        loop {
            match rx1.recv() {
                Err(_) => break,
                Ok(old_swapchain) => {
                    println!("re-recreating swapchain");
                    let swapchain = device_clone
                        .create_swapchain(swapchain_desc, Some(&old_swapchain))
                        .expect("swapchain creation failed");
                    tx2.send(swapchain);
                }
            }
        }
    });

    event_loop.run_return(|event, _target, control_flow| {
        let mut handle_event = || {
            match event {
                Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
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
                    //let old_swapchain = swapchain.take();
                    //swapchain = Some(device.create_swapchain(swapchain_desc, old_swapchain.as_ref())?);
                    //rebuild_swapchain = true;
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    let frame_time = Instant::now();

                    if rebuild_swapchain {
                        if let Some(old_swapchain) = swapchain.take() {
                            println!("requesting new swapchain");
                            swapchain_in_flight.store(true, Ordering::SeqCst);
                            tx1.send(old_swapchain)?;
                        }
                        rebuild_swapchain = false;
                    }

                    if let Ok(new_swapchain) = rx2.try_recv() {
                        println!("received new swapchain");
                        swapchain_in_flight.store(false, Ordering::SeqCst);
                        swapchain = Some(new_swapchain);
                    }

                    use ash::vk;

                    if let Some(ref swapchain) = swapchain {
                        let frame = match swapchain.acquire_next_image() {
                            Ok(frame) => Some(frame),
                            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                                rebuild_swapchain = true;
                                None
                            }
                            Err(e) => panic!("swapchain.acquire_next_image: {:?}", e),
                        };

                        if let Some(frame) = frame {
                            //let frame = swapchain.acquire_next_image()?;
                            println!("new frame; time: {:?}", frame_time);
                            let queue = device.get_queue();
                            match queue.present(frame) {
                                Ok(()) => {}
                                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                                    rebuild_swapchain = true;
                                }
                                Err(e) => panic!("queue.present: {:?}", e),
                            }
                        }
                    }

                    *control_flow = ControlFlow::WaitUntil(last_frame_time + Duration::from_millis(16));
                    last_frame_time = frame_time;
                }
                _ => {}
            }
            Ok(())
        };
        let result: Result<(), Box<std::error::Error>> = handle_event();
        result.expect("event loop error");
    });

    drop(tx1);
    drop(rx2);

    join_handle.join().unwrap();

    Ok(())
}
