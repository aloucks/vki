#[macro_use]
extern crate memoffset;

use vki::{
    AdapterOptions, BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor,
    BindingResource, BindingType, BlendDescriptor, BlendFactor, BlendOperation, BufferDescriptor, BufferUsageFlags,
    Color, ColorStateDescriptor, ColorWriteFlags, CullMode, DeviceDescriptor, FrontFace, IndexFormat,
    InputStateDescriptor, InputStepMode, Instance, LoadOp, PipelineLayoutDescriptor, PipelineStageDescriptor,
    PowerPreference, PrimitiveTopology, RasterizationStateDescriptor, RenderPassColorAttachmentDescriptor,
    RenderPassDescriptor, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStageFlags, StoreOp,
    SwapchainDescriptor, SwapchainError, TextureFormat, TextureUsageFlags, VertexAttributeDescriptor,
    VertexBufferDescriptor, VertexFormat,
};

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::desktop::EventLoopExtDesktop;

use std::borrow::Cow;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("VK_INSTANCE_LAYERS").is_err() {
        std::env::set_var("VK_INSTANCE_LAYERS", "VK_LAYER_LUNARG_standard_validation");
    }

    let _ = pretty_env_logger::try_init();

    let mut event_loop = EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("triangle.rs")
        .with_inner_size(LogicalSize::from((800, 600)))
        .with_visible(false)
        .build(&event_loop)?;

    let instance = Instance::new()?;
    let adapter_options = AdapterOptions {
        power_preference: PowerPreference::HighPerformance,
    };

    let adapter = instance.get_adapter(adapter_options)?;
    println!("Adapter: {}", adapter.name());

    let surface_descriptor = vki::winit_surface_descriptor!(&window);

    let surface = instance.create_surface(&surface_descriptor)?;

    let device_desc = DeviceDescriptor::default().with_surface_support(&surface);
    let device = adapter.create_device(device_desc)?;

    let formats = device.get_supported_swapchain_formats(&surface)?;
    println!("Supported swapchain formats: {:?}", formats);

    let swapchain_format = TextureFormat::B8G8R8A8Unorm;
    assert!(formats.contains(&swapchain_format));

    let swapchain_desc = SwapchainDescriptor {
        surface: &surface,
        format: swapchain_format,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    };

    let mut swapchain = device.create_swapchain(&swapchain_desc, None)?;
    window.set_visible(true);

    let vertex_shader = device.create_shader_module(ShaderModuleDescriptor {
        code: Cow::Borrowed(include_bytes!("shaders/triangle.vert.spv")),
    })?;

    let fragment_shader = device.create_shader_module(ShaderModuleDescriptor {
        code: Cow::Borrowed(include_bytes!("shaders/triangle.frag.spv")),
    })?;

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        bindings: &[BindGroupLayoutBinding {
            binding: 0,
            visibility: ShaderStageFlags::VERTEX,
            binding_type: BindingType::UniformBuffer,
        }],
    })?;

    let pipeline_layout = device.create_pipeline_layout(PipelineLayoutDescriptor {
        bind_group_layouts: vec![bind_group_layout.clone()],
        push_constant_ranges: vec![],
    })?;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct Uniforms {
        clip: [[f32; 4]; 4],
        time: f32,
    }

    let uniforms_size_bytes = std::mem::size_of::<Uniforms>();

    let uniform_buffer = device.create_buffer(&BufferDescriptor {
        size: uniforms_size_bytes,
        usage: BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
    })?;

    #[rustfmt::skip]
    let mut uniforms = Uniforms {
        // Vulkan clip space has inverted Y and half Z
        // https://github.com/LunarG/VulkanSamples/commit/0dd36179880238014512c0637b0ba9f41febe803
        // https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/
        // http://anki3d.org/vulkan-coordinate-system/
        clip: [
            [1.0,  0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0,  0.0, 0.5, 0.0],
            [0.0,  0.0, 0.5, 1.0],
        ],
        time: 0.0,
    };

    let bind_group = device.create_bind_group(BindGroupDescriptor {
        layout: bind_group_layout.clone(),
        bindings: vec![BindGroupBinding {
            binding: 0,
            resource: BindingResource::Buffer(uniform_buffer.clone(), 0..uniforms_size_bytes),
        }],
    })?;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct Vertex {
        position: [f32; 3],
        color: [f32; 3],
    }

    #[rustfmt::skip]
    let vertices = &[
        Vertex { position: [-0.5, -0.5, 0.0], color: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
        Vertex { position: [ 0.0,  0.5, 0.0], color: [0.0, 0.0, 1.0] },
    ];

    let vertices_size_bytes = std::mem::size_of::<Vertex>() * vertices.len();

    let vertex_buffer = device.create_buffer(&BufferDescriptor {
        size: vertices_size_bytes,
        usage: BufferUsageFlags::VERTEX | BufferUsageFlags::TRANSFER_DST,
    })?;

    let staging_vertex_buffer = device.create_buffer_mapped(&BufferDescriptor {
        size: vertices_size_bytes,
        usage: BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::MAP_WRITE,
    })?;

    staging_vertex_buffer.copy_from_slice(vertices)?;

    let mut encoder = device.create_command_encoder()?;

    encoder.copy_buffer_to_buffer(
        &staging_vertex_buffer.unmap(),
        0,
        &vertex_buffer,
        0,
        vertices_size_bytes,
    );

    device.get_queue().submit(&[encoder.finish()?])?;

    let color_replace = BlendDescriptor {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    };

    let render_pipeline_descriptor = RenderPipelineDescriptor {
        layout: pipeline_layout.clone(),
        primitive_topology: PrimitiveTopology::TriangleList,
        vertex_stage: PipelineStageDescriptor {
            entry_point: Cow::Borrowed("main"),
            module: vertex_shader,
        },
        fragment_stage: PipelineStageDescriptor {
            entry_point: Cow::Borrowed("main"),
            module: fragment_shader,
        },
        input_state: InputStateDescriptor {
            index_format: IndexFormat::U16,
            vertex_buffers: vec![VertexBufferDescriptor {
                input_slot: 0,
                step_mode: InputStepMode::Vertex,
                stride: std::mem::size_of::<Vertex>(),
                attributes: vec![
                    VertexAttributeDescriptor {
                        format: VertexFormat::Float3,
                        offset: offset_of!(Vertex, position),
                        shader_location: 0,
                    },
                    VertexAttributeDescriptor {
                        format: VertexFormat::Float3,
                        offset: offset_of!(Vertex, color),
                        shader_location: 1,
                    },
                ],
            }],
        },
        color_states: vec![ColorStateDescriptor {
            format: swapchain_format,
            write_mask: ColorWriteFlags::ALL,
            color_blend: color_replace,
            alpha_blend: color_replace,
        }],
        depth_stencil_state: None,
        rasterization_state: RasterizationStateDescriptor {
            front_face: FrontFace::Ccw,
            cull_mode: CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        },
        sample_count: 1,
    };

    let pipeline = device.create_render_pipeline(render_pipeline_descriptor)?;

    let start = Instant::now();

    const MIN_DURATION: Duration = Duration::from_millis(0);

    let mut last_frame_time = Instant::now();
    let mut last_fps_time = Instant::now();
    let mut frame_count = 0;

    event_loop.run_return(|event, _target, control_flow| {
        let mut handle_event = || {
            match event {
                Event::EventsCleared => {
                    if Instant::now() - MIN_DURATION >= last_frame_time {
                        window.request_redraw();
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    ..
                } => {
                    swapchain = device.create_swapchain(&swapchain_desc, Some(&swapchain))?;
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    last_frame_time = Instant::now();
                    *control_flow = ControlFlow::WaitUntil(last_frame_time + MIN_DURATION);

                    frame_count += 1;

                    if last_fps_time.elapsed() > Duration::from_millis(1000) {
                        println!("FPS: {}", frame_count);
                        frame_count = 0;
                        last_fps_time = Instant::now();
                    }

                    let frame = match swapchain.acquire_next_image() {
                        Ok(frame) => frame,
                        Err(SwapchainError::OutOfDate) => return Ok(()),
                        Err(e) => return Err(e)?,
                    };
                    let frame_time = Instant::now();
                    //println!("new frame; time: {:?}", frame_time);

                    uniforms.time = (start.elapsed().as_millis() as f32) / 1000.0;
                    uniform_buffer.set_sub_data(0, &[uniforms])?;

                    let mut encoder = device.create_command_encoder()?;
                    let mut render_pass = encoder.begin_render_pass(RenderPassDescriptor {
                        color_attachments: &[RenderPassColorAttachmentDescriptor {
                            attachment: &frame.view,
                            clear_color: Color {
                                r: 0.1,
                                g: 0.1,
                                b: 0.1,
                                a: 1.0,
                            },
                            load_op: LoadOp::Clear,
                            store_op: StoreOp::Store,
                            resolve_target: None,
                        }],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&pipeline);
                    render_pass.set_vertex_buffers(0, &[vertex_buffer.clone()], &[0]);
                    render_pass.set_bind_group(0, &bind_group, None);
                    render_pass.draw(3, 1, 0, 1);
                    render_pass.end_pass();

                    let queue = device.get_queue();

                    queue.submit(&[encoder.finish()?])?;

                    match queue.present(frame) {
                        Ok(frame) => frame,
                        Err(SwapchainError::OutOfDate) => return Ok(()),
                        Err(e) => return Err(e)?,
                    }

                    *control_flow = ControlFlow::WaitUntil(last_frame_time + Duration::from_millis(0));
                    last_frame_time = frame_time;
                }
                _ => {}
            }
            Ok(())
        };
        let result: Result<(), Box<dyn std::error::Error>> = handle_event();
        result.expect("event loop error");
    });

    Ok(())
}
