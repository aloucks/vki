use cgmath::prelude::*;
use cgmath::{Deg, Matrix4, Point3, Vector3};

pub mod shape;

use ash::vk;

use std::slice;

use vki::{
    Adapter, AdapterOptions, Buffer, BufferCopyView, BufferDescriptor, BufferUsageFlags, CommandEncoder, Device,
    DeviceDescriptor, Error, Extensions, Extent3D, FilterMode, Instance, Origin3D, PowerPreference, Surface, Swapchain,
    SwapchainDescriptor, Texture, TextureBlitView, TextureCopyView, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsageFlags, TextureView,
};

use std::time::{Duration, Instant};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{
    DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent,
};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

pub const DEFAULT_DEPTH_FORMAT: TextureFormat = TextureFormat::D32FloatS8Uint;
pub const DEFAULT_COLOR_FORMAT: TextureFormat = TextureFormat::B8G8R8A8Unorm;

#[allow(unused_macros)]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = std::mem::uninitialized();
            let offset = (&b.$field as *const _ as isize) - (&b as *const _ as isize);
            std::mem::forget(b);
            offset as _
        }
    }};
}

fn create_swapchain_and_depth_view_and_color_view(
    device: &Device,
    surface: &Surface,
    sample_count: u32,
    width: u32,
    height: u32,
    old_swapchain: Option<&Swapchain>,
) -> Result<(Swapchain, TextureView, TextureView), Error> {
    let swap_chain = device
        .create_swapchain(
            SwapchainDescriptor {
                surface,
                usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
                format: DEFAULT_COLOR_FORMAT,
            },
            old_swapchain,
        )
        .map_err(|e| {
            log::error!("failed to create swapchain: {:?}", e);
            vk::Result::ERROR_INITIALIZATION_FAILED
        })?;

    let depth_texture = device.create_texture(TextureDescriptor {
        size: Extent3D {
            width,
            height,
            depth: 1,
        },
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count,
        dimension: TextureDimension::D2,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
        format: DEFAULT_DEPTH_FORMAT,
    })?;

    let depth_view = depth_texture.create_default_view()?;

    let color_texture = device.create_texture(TextureDescriptor {
        size: Extent3D {
            width,
            height,
            depth: 1,
        },
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count,
        dimension: TextureDimension::D2,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
        format: DEFAULT_COLOR_FORMAT,
    })?;

    let color_view = color_texture.create_default_view()?;

    Ok((swap_chain, depth_view, color_view))
}

pub enum EventHandlers<T> {
    Default,
    Custom(Vec<Box<dyn EventHandler<T>>>),
}

impl<T: 'static> EventHandlers<T> {
    pub fn default_event_handlers() -> Vec<Box<dyn EventHandler<T>>> {
        vec![
            Box::new(CloseRequestedHandler),
            Box::new(WindowResizedHandler::default()),
            Box::new(CameraViewportHandler),
            Box::new(ArcBallCameraControlHandler::default()),
        ]
    }
}

impl<T: 'static> Into<Vec<Box<dyn EventHandler<T>>>> for EventHandlers<T> {
    fn into(self) -> Vec<Box<dyn EventHandler<T>>> {
        match self {
            EventHandlers::Default => EventHandlers::default_event_handlers(),
            EventHandlers::Custom(event_handlers) => event_handlers,
        }
    }
}

enum WindowMode {
    Fullscreen {
        last_position: LogicalPosition,
        last_size: LogicalSize,
    },
    Windowed,
}

pub struct App<T> {
    pub instance: Instance,
    pub surface: Surface,
    pub adapter: Adapter,
    pub device: Device,
    pub swapchain: Swapchain,
    pub depth_view: TextureView,
    pub color_view: TextureView,
    pub window: Window,
    pub should_close: bool,
    pub camera: Camera,
    pub state: T,
    /// Throttle the frame rate to reduce CPU usage for low complexity scenes. Values greater than
    /// `1000` are interpreted as `unlimited`. The default is `60`.
    pub max_fps: u64,
    last_frame_time: Instant,
    window_mode: WindowMode,
    sample_count: u32,
    event_handlers: Option<Vec<Box<dyn EventHandler<T>>>>,
    event_loop: Option<EventLoop<()>>,
}

impl<T: 'static> App<T> {
    pub fn init(
        title: &str,
        window_width: u32,
        window_height: u32,
        event_handlers: EventHandlers<T>,
    ) -> Result<App<T>, Error>
    where
        T: Default,
    {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_inner_size(LogicalSize::from((window_width, window_height)))
            .with_title(title)
            .with_visibility(false)
            .build(&event_loop)
            .map_err(|e| {
                log::error!("Failed to initialize window: {:?}", e);
                vk::Result::ERROR_INITIALIZATION_FAILED
            })?;

        let instance = Instance::new().map_err(|e| {
            log::error!("Failed to initialize instance: {:?}", e);
            vk::Result::ERROR_INITIALIZATION_FAILED
        })?;

        let surface = instance.create_surface(&vki::winit_surface_descriptor!(&window))?;
        let adapter = instance.get_adapter(AdapterOptions {
            power_preference: PowerPreference::HighPerformance,
        })?;
        let device = adapter.create_device(DeviceDescriptor {
            surface_support: Some(&surface),
            extensions: Extensions {
                anisotropic_filtering: false,
            },
        })?;

        let sample_count = 1;

        let (swapchain, depth_view, color_view) = create_swapchain_and_depth_view_and_color_view(
            &device,
            &surface,
            sample_count,
            window_width,
            window_height,
            None,
        )?;

        let state = Default::default();
        let should_close = false;
        let event_loop = Some(event_loop);
        let event_handlers = Some(event_handlers.into());
        let camera = Camera::new(window_width, window_height);
        let window_mode = WindowMode::Windowed;
        let last_frame_time = Instant::now();
        let max_fps = if cfg!(target_os = "linux") {
            // Winit on X11 is eating events with eventloop-2.0.
            // Limiting the FPS seems to help mitigate the issue.
            println!("FIXME (Winit/X11): FPS Limit: 15");
            15
        } else {
            60
        };

        log::debug!("{:#?}", adapter);

        Ok(App {
            instance,
            surface,
            adapter,
            device,
            swapchain,
            depth_view,
            color_view,
            window,
            state,
            should_close,
            camera,
            event_loop,
            event_handlers,
            sample_count,
            window_mode,
            last_frame_time,
            max_fps,
        })
    }

    pub fn toggle_window_mode(&mut self) {
        match self.window_mode {
            WindowMode::Windowed => {
                let last_position = self.window.outer_position().unwrap();
                let last_size = self.window.inner_size();
                let monitor = self.window.current_monitor();

                // Use windowed mode on windows and support fullscreen
                // across multiple monitors. This assumes the monitors
                // are all the same size.
                if cfg!(target_os="windows") {
                    let dpi_factor = monitor.hidpi_factor();
                    let x = monitor.position().x as _;

                    // If the window is wider than the current monitor, stretch it across all monitors
                    let mut inner_physical_size = monitor.dimensions();
                    if last_size.to_physical(dpi_factor).width > inner_physical_size.width {
                        let monitor_count = self.window.available_monitors().count();
                        inner_physical_size.width *= monitor_count as f64;
                    }

                    self.window.set_visible(false);
                    self.window.set_decorations(false);
                    self.window.set_outer_position(LogicalPosition::from((x, 0)));
                    self.window.set_inner_size(inner_physical_size.to_logical(dpi_factor));
                    self.window.set_visible(true);
                } else {
                    self.window.set_fullscreen(Some(monitor));
                }

                self.window_mode = WindowMode::Fullscreen {
                    last_position,
                    last_size,
                };
            }
            WindowMode::Fullscreen {
                last_position,
                last_size,
            } => {
                if cfg!(target_os = "windows") {
                    self.window.set_visible(false);
                    self.window.set_decorations(true);
                    self.window.set_inner_size(last_size);
                    self.window.set_outer_position(last_position);
                    self.window.set_visible(true);
                } else {
                    self.window.set_fullscreen(None);
                }

                self.window_mode = WindowMode::Windowed;
            }
        }
    }

    pub fn set_sample_count(&mut self, sample_count: u32) -> Result<(), Error> {
        let (window_width, window_height): (u32, u32) = self.window.inner_size().into();
        if self.sample_count != sample_count {
            let (swapchain, depth_view, color_view) = create_swapchain_and_depth_view_and_color_view(
                &self.device,
                &self.surface,
                sample_count,
                window_width,
                window_height,
                Some(&self.swapchain),
            )?;
            self.swapchain = swapchain;
            self.depth_view = depth_view;
            self.color_view = color_view;
        }
        self.sample_count = sample_count;
        Ok(())
    }

    pub fn get_sample_count(&self) -> u32 {
        self.sample_count
    }

    pub fn run<F>(mut self, mut on_frame: F) -> !
    where
        F: 'static + FnMut(&mut App<T>) -> Result<(), Box<dyn std::error::Error>>,
    {
        let event_loop = self.event_loop.take().unwrap();
        let mut event_handlers = self.event_handlers.take().unwrap();

        self.camera.update_view_matrix();
        self.camera.update_projection_matrix();

        self.window.set_visible(true);

        event_loop.run(move |event, _, control_flow| {
            for event_handler in event_handlers.iter_mut() {
                let consume = event_handler.on_event(&mut self, &event);
                if consume {
                    break;
                }
            }

            let min_duration = Duration::from_millis(1000 / self.max_fps);

            match event {
                Event::EventsCleared => {
                    if Instant::now() - min_duration >= self.last_frame_time {
                        self.window.request_redraw();
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    self.last_frame_time = Instant::now();

                    *control_flow = ControlFlow::WaitUntil(self.last_frame_time + min_duration);

                    for event_handler in event_handlers.iter_mut() {
                        event_handler.on_frame(&mut self);
                    }

                    on_frame(&mut self).expect("on_frame error");
                }
                _ => {}
            }

            if self.should_close {
                *control_flow = ControlFlow::Exit;
            }
        })
    }
}

/// Convenience function for submitting a command buffer and creating a new encoder
pub fn submit(device: &Device, encoder: CommandEncoder) -> Result<CommandEncoder, vki::Error> {
    device.get_queue().submit(&[encoder.finish()?])?;
    Ok(device.create_command_encoder()?)
}

/// Creates a new buffer with the given data. If the `usage` flag contains `BufferUsageFlags::MAP_WRITE`,
/// the data is mapped and copied directly. Otherwise, a staging buffer is used to copy the data
pub fn create_buffer_with_data<U: Copy + 'static>(
    device: &Device,
    encoder: &mut CommandEncoder,
    mut usage: BufferUsageFlags,
    data: &[U],
) -> Result<Buffer, Error> {
    let size_bytes = self::byte_length(&data);
    let is_write_mapped = usage.contains(BufferUsageFlags::MAP_WRITE);

    if !is_write_mapped {
        usage |= BufferUsageFlags::TRANSFER_DST;
    }

    let descriptor = BufferDescriptor {
        usage,
        size: size_bytes,
    };

    if is_write_mapped {
        let mapped_buffer = device.create_buffer_mapped(descriptor)?;
        mapped_buffer.write(0, data)?;
        Ok(mapped_buffer.unmap())
    } else {
        let buffer = device.create_buffer(descriptor)?;
        copy_to_buffer(device, encoder, data, &buffer)?;
        Ok(buffer)
    }
}

pub fn create_staging_buffer<U: Copy + 'static>(device: &Device, data: &[U]) -> Result<Buffer, Error> {
    let descriptor = BufferDescriptor {
        usage: BufferUsageFlags::MAP_WRITE | BufferUsageFlags::TRANSFER_SRC,
        size: byte_length(data),
    };
    let mapped_buffer = device.create_buffer_mapped(descriptor)?;
    mapped_buffer.write(0, data)?;
    Ok(mapped_buffer.unmap())
}

/// Copies the data to the destination using a staging buffer
pub fn copy_to_buffer<T: Copy + 'static>(
    device: &Device,
    encoder: &mut CommandEncoder,
    data: &[T],
    destination: &Buffer,
) -> Result<(), Error> {
    let staging_buffer = create_staging_buffer(device, data)?;
    encoder.copy_buffer_to_buffer(&staging_buffer, 0, destination, 0, self::byte_length(data));
    Ok(())
}

/// If `has_mipmaps` is `false`, the `mip_level_count` is set to `1`. Otherwise, the
/// additional mipmaps should be generated or uploaded separately.
pub fn create_texture_with_data(
    device: &Device,
    encoder: &mut CommandEncoder,
    data: &[u8],
    has_mipmaps: bool,
    format: TextureFormat,
    width: u32,
    height: u32,
) -> Result<Texture, Error> {
    let size = Extent3D {
        width,
        height,
        depth: 1,
    };
    let mip_level_count = if has_mipmaps {
        (size.width.max(size.height) as f32).log2().floor() as u32 + 1
    } else {
        1
    };

    let descriptor = TextureDescriptor {
        mip_level_count,
        size,
        format,
        sample_count: 1,
        array_layer_count: 1,
        usage: TextureUsageFlags::SAMPLED | TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::TRANSFER_DST,
        dimension: TextureDimension::D2,
    };

    let texture = device.create_texture(descriptor)?;

    let buffer = create_staging_buffer(&device, data)?;

    encoder.copy_buffer_to_texture(
        BufferCopyView {
            offset: 0,
            buffer: &buffer,
            image_height: size.height,
            row_pitch: size.width,
        },
        TextureCopyView {
            texture: &texture,
            origin: Origin3D { x: 0, y: 0, z: 0 },
            mip_level: 0,
            array_layer: 0,
        },
        size,
    );

    Ok(texture)
}

/// If `has_mipmaps` is `false`, the `mip_level_count` is set to `1`. Otherwise, the
/// additional mipmaps should be generated or uploaded separately.
pub fn create_texture(
    device: &Device,
    encoder: &mut CommandEncoder,
    img: image::DynamicImage,
    has_mipmaps: bool,
) -> Result<Texture, Error> {
    use image::DynamicImage;
    use image::GenericImageView;

    let (width, height) = img.dimensions();

    let (format, data): (TextureFormat, &[u8]) = match &img {
        DynamicImage::ImageRgba8(img) => (TextureFormat::R8G8B8A8Unorm, &img),
        DynamicImage::ImageLuma8(img) => (TextureFormat::R8Unorm, &img),
        _ => unimplemented!(),
    };

    create_texture_with_data(device, encoder, data, has_mipmaps, format, width, height)
}

pub fn generate_mipmaps(encoder: &mut CommandEncoder, texture: &Texture) -> Result<(), Error> {
    let mut mip_width = texture.size().width;
    let mut mip_height = texture.size().height;

    let mip_level_count = texture.mip_level_count();

    for i in 1..mip_level_count {
        let src = TextureBlitView {
            texture: &texture,
            mip_level: i - 1,
            array_layer: 0,
            bounds: [
                Origin3D { x: 0, y: 0, z: 0 },
                Origin3D {
                    x: mip_width as i32,
                    y: mip_height as i32,
                    z: 1,
                },
            ],
        };

        if mip_width > 1 {
            mip_width = mip_width / 2;
        }

        if mip_height > 1 {
            mip_height = mip_height / 2;
        }

        let dst = TextureBlitView {
            texture: &texture,
            mip_level: i,
            array_layer: 0,
            bounds: [
                Origin3D { x: 0, y: 0, z: 0 },
                Origin3D {
                    x: mip_width as i32,
                    y: mip_height as i32,
                    z: 1,
                },
            ],
        };

        encoder.blit_texture_to_texture(src, dst, FilterMode::Linear);
    }

    Ok(())
}

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rect<S> {
    pub left: S,
    pub bottom: S,
    pub width: S,
    pub height: S,
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub fovy: Deg<f32>,
    pub near: f32,
    pub far: f32,

    pub projection: Matrix4<f32>,
    pub view: Matrix4<f32>,

    pub viewport: Rect<f32>,

    pub eye: Point3<f32>,
    pub center: Point3<f32>,
    pub up: Vector3<f32>,
}

impl Camera {
    pub fn new(window_width: u32, window_height: u32) -> Camera {
        let fovy = Deg(45.0);
        let near = 0.01;
        let far = 10000.0;

        let projection = Matrix4::identity();
        let viewport = Rect {
            left: 0.0,
            bottom: 0.0,
            width: window_width as f32,
            height: window_height as f32,
        };

        let eye = Point3 {
            x: 10.0,
            y: 10.0,
            z: 10.0,
        };
        let center = Point3::origin();
        let up = Vector3::unit_y();

        let view = Matrix4::look_at(eye, center, up);

        let mut camera = Camera {
            fovy,
            near,
            far,
            projection,
            viewport,
            eye,
            center,
            up,
            view,
        };

        camera.update_projection_matrix();

        camera
    }

    pub fn update_projection_matrix(&mut self) {
        let width = self.viewport.width - self.viewport.left;
        let height = self.viewport.height - self.viewport.bottom;
        let aspect = width as f32 / height as f32;

        let clip = clip_correction_matrix();

        self.projection = clip * cgmath::perspective(self.fovy, aspect, self.near, self.far);
    }

    pub fn update_view_matrix(&mut self) {
        self.view = Matrix4::look_at(self.eye, self.center, self.up);
    }
}

pub trait EventHandler<T> {
    /// Called once per event. Return `true` to consume the event.
    fn on_event(&mut self, app: &mut App<T>, event: &Event<()>) -> bool;

    /// Called once per frame after all events are processed
    fn on_frame(&mut self, _app: &mut App<T>) {}
}

pub struct CloseRequestedHandler;

impl<T> EventHandler<T> for CloseRequestedHandler {
    fn on_event(&mut self, app: &mut App<T>, event: &Event<()>) -> bool {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => app.should_close = true,
            _ => {}
        }

        false
    }
}

#[derive(Default)]
pub struct WindowResizedHandler {
    new_window_width: u32,
    new_window_height: u32,
    rebuild_swapchain_and_views: bool,
}

impl<T> EventHandler<T> for WindowResizedHandler {
    fn on_event(&mut self, _app: &mut App<T>, event: &Event<()>) -> bool {
        let mut consume = false;
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(logical_size),
                ..
            } => {
                let (width, height) = (*logical_size).into();
                self.new_window_width = width;
                self.new_window_height = height;
                self.rebuild_swapchain_and_views = true;
                std::thread::yield_now();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                if self.new_window_height <= 0 || self.new_window_width <= 0 {
                    consume = true;
                }
            }
            _ => {}
        }

        consume
    }

    fn on_frame(&mut self, app: &mut App<T>) {
        let ready_to_rebuild = self.new_window_height > 0 && self.new_window_width > 0;
        if self.rebuild_swapchain_and_views && ready_to_rebuild {
            let (swapchain, new_depth_view, new_color_view) = create_swapchain_and_depth_view_and_color_view(
                &app.device,
                &app.surface,
                app.sample_count,
                self.new_window_width,
                self.new_window_height,
                Some(&app.swapchain),
            )
            .expect("failed to re-create swapchain or textures views");
            app.swapchain = swapchain;
            app.depth_view = new_depth_view;
            app.color_view = new_color_view;
            self.rebuild_swapchain_and_views = false;
        }
    }
}

#[derive(Default, Debug, Clone)]
struct MouseState {
    motion_deltas: Vec<(f64, f64)>,
    button_down: bool,
    cursor_left: bool,
}

impl<T> EventHandler<T> for MouseState {
    fn on_event(&mut self, _: &mut App<T>, event: &Event<()>) -> bool {
        let mut consume = false;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CursorLeft { .. },
                ..
            } => {
                self.cursor_left = true;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorEntered { .. },
                ..
            } => {
                self.cursor_left = false;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { .. },
                ..
            } => {
                self.cursor_left = false;
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { button, state, .. },
                ..
            } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) => {
                    self.button_down = true;
                    consume = true;
                }
                (MouseButton::Left, ElementState::Released) => {
                    self.button_down = false;
                    consume = true;
                }
                _ => {}
            },
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta, .. },
                ..
            } => {
                if self.button_down && !self.cursor_left {
                    let (x, y) = delta;
                    self.motion_deltas.push((x.round() as _, y.round() as _));
                    consume = true;
                }
            }
            _ => {}
        }
        consume
    }
}

#[derive(Default, Debug, Clone)]
pub struct CameraViewportHandler;

impl<T> EventHandler<T> for CameraViewportHandler {
    fn on_event(&mut self, app: &mut App<T>, event: &Event<()>) -> bool {
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(logical_size),
                ..
            } => {
                let (new_width, new_height): (u32, u32) = (*logical_size).into();
                app.camera.viewport.width = new_width as f32;
                app.camera.viewport.height = new_height as f32;
                if new_height > 0 && new_width > 0 {
                    app.camera.update_projection_matrix();
                }
            }
            _ => {}
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct ArcBallCameraControlHandler {
    mouse_state: MouseState,
    show_cursor_position: LogicalPosition,
}

impl Default for ArcBallCameraControlHandler {
    fn default() -> ArcBallCameraControlHandler {
        ArcBallCameraControlHandler {
            mouse_state: Default::default(),
            show_cursor_position: LogicalPosition { x: 0.0, y: 0.0 },
        }
    }
}

impl<T: 'static> EventHandler<T> for ArcBallCameraControlHandler {
    fn on_event(&mut self, app: &mut App<T>, event: &Event<()>) -> bool {
        let consume = self.mouse_state.on_event(app, event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                if !self.mouse_state.button_down {
                    self.show_cursor_position = *position;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { button, state, .. },
                ..
            } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) => app.window.set_cursor_visible(false),
                (MouseButton::Left, ElementState::Released) => {
                    app.window
                        .set_cursor_position(self.show_cursor_position)
                        .expect("failed to set cursor position");
                    app.window.set_cursor_visible(true);
                }
                _ => {}
            },
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(virtual_keycode),
                                modifiers,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let transform = app.camera.view.invert().unwrap();
                let left = transform.transform_vector(Vector3::unit_x());
                let forward = -left.cross(Vector3::unit_y());

                match virtual_keycode {
                    VirtualKeyCode::W => {
                        app.camera.eye += forward;
                        app.camera.center += forward;
                    }
                    VirtualKeyCode::S => {
                        app.camera.eye -= forward;
                        app.camera.center -= forward;
                    }
                    VirtualKeyCode::C => {
                        app.camera.center = Point3::new(0.0, 0.0, 0.0);
                    }
                    VirtualKeyCode::A => {
                        app.camera.eye -= left;
                        app.camera.center -= left;
                    }
                    VirtualKeyCode::D => {
                        app.camera.eye += left;
                        app.camera.center += left;
                    }
                    VirtualKeyCode::PageUp => {
                        app.camera.eye += Vector3::unit_y();
                        if !modifiers.shift {
                            app.camera.center += Vector3::unit_y();
                        }
                    }
                    VirtualKeyCode::PageDown => {
                        app.camera.eye -= Vector3::unit_y();
                        if !modifiers.shift {
                            app.camera.center -= Vector3::unit_y();
                        }
                    }
                    VirtualKeyCode::F11 => {
                        app.toggle_window_mode();
                    }
                    _ => {}
                }

                app.camera.update_view_matrix();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        ..
                    },
                ..
            } => {
                let v: Vector3<f32> = app.camera.eye - app.camera.center;
                let y = *y;
                if v.magnitude() - y > 0.0 {
                    let dir: Vector3<f32> = v.normalize();
                    app.camera.eye += dir * -y;
                    app.camera.update_view_matrix();
                }
            }
            _ => {}
        }

        for (dx, dy) in self.mouse_state.motion_deltas.drain(..) {
            let (dx, dy) = (dx as f32, dy as f32);

            let transform = app.camera.view.invert().unwrap();
            let right = transform.transform_vector(Vector3::unit_x());
            let forward = transform.transform_vector(-Vector3::unit_z());
            let up = app.camera.up;

            let angle = Deg::from(forward.angle(up)).0 + dy;
            if angle < 179.0 && angle > 1.0 {
                let m1 = Matrix4::from_axis_angle(up, Deg(-dx));
                let m2 = Matrix4::from_axis_angle(right, Deg(-dy));

                let center_offset: Vector3<f32> = app.camera.center - Point3::origin();

                let mut eye = app.camera.eye - center_offset;

                eye = m1.transform_point(eye);
                eye = m2.transform_point(eye);

                app.camera.eye = eye + center_offset;

                app.camera.update_view_matrix();
            }
        }

        consume
    }
}

pub fn byte_stride<T: Copy>(_: &[T]) -> usize {
    std::mem::size_of::<T>()
}

pub fn byte_length<T: Copy>(values: &[T]) -> usize {
    byte_stride(values) * values.len()
}

pub fn byte_offset<T: Copy>(count: usize) -> usize {
    std::mem::size_of::<T>() * count
}

pub fn byte_cast<T: Copy>(values: &[T]) -> &[u8] {
    let len = byte_length(values) as usize;
    unsafe { slice::from_raw_parts(values.as_ptr() as *const _, len) }
}

pub fn to_float_secs(d: std::time::Duration) -> f32 {
    const NANOS_PER_SEC: u32 = 1_000_000_000;
    let time = (d.as_secs() as f64) + (d.subsec_nanos() as f64) / (NANOS_PER_SEC as f64);
    time as f32
}

/// Returns a clip correction matrix to account for the Vulkan coordinate system.
///
/// Vulkan clip space has inverted Y and half Z.
///
/// https://github.com/LunarG/VulkanSamples/commit/0dd36179880238014512c0637b0ba9f41febe803
///
/// https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/
///
/// http://anki3d.org/vulkan-coordinate-system/
///
/// ```glsl
/// gl_Position = clip * projection * view * model * vec4(position, 1.0);
/// ```
pub fn clip_correction_matrix() -> Matrix4<f32> {
    let mut clip = cgmath::Matrix4::identity();
    clip[1][1] = -1.0;
    clip[2][2] = 0.5;
    clip[3][2] = 0.5;

    clip
}
