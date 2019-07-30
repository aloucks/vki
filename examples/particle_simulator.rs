pub mod util;

use cgmath::SquareMatrix;
use cgmath::{Matrix4, Point3, Vector4};

use num_traits::Zero;

use std::borrow::Cow;

use crate::util::{App, EventHandler, EventHandlers};

use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BlendDescriptor, BlendFactor, BlendOperation, BufferUsageFlags, BufferViewDescriptor,
    BufferViewFormat, Color, ColorStateDescriptor, ColorWriteFlags, ComputePipelineDescriptor, CullMode, Fence,
    FrontFace, IndexFormat, InputStateDescriptor, InputStepMode, LoadOp, PipelineLayoutDescriptor,
    PipelineStageDescriptor, PrimitiveTopology, RasterizationStateDescriptor, RenderPassColorAttachmentDescriptor,
    RenderPassDescriptor, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStageFlags, StoreOp, SwapchainError,
    TextureFormat, VertexAttributeDescriptor, VertexBufferDescriptor, VertexFormat,
};

use rand::Rng;

use std::time::{Duration, Instant};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};

const PARTICLE_GROUP_SIZE: usize = 512;
const PARTICLE_GROUP_COUNT: usize = 8192;
const PARTICLE_COUNT: usize = PARTICLE_GROUP_SIZE * PARTICLE_GROUP_COUNT;
const MAX_ATTRACTORS: usize = 64;

// Derived from the particle simulator example of the OpenGL Programming Guide (chapter 12)

#[derive(Default)]
struct State {
    reset1: bool,
    reset2: bool,
}

struct ResetHandler;

impl EventHandler<State> for ResetHandler {
    fn on_event(&mut self, app: &mut App<State>, event: &Event<()>) -> bool {
        if let Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::F2),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
            ..
        } = event
        {
            app.state.reset1 = true;
        }
        if let Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::F3),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
            ..
        } = event
        {
            app.state.reset2 = true;
        }

        false
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = pretty_env_logger::try_init();

    let mut event_handlers = EventHandlers::default_event_handlers();

    event_handlers.push(Box::new(ResetHandler));

    let mut app: App<State> = App::init("particle_simulator.rs", 800, 600, EventHandlers::Custom(event_handlers))?;

    app.set_sample_count(4)?;

    app.camera.eye = Point3 {
        x: 0.0,
        y: 0.0,
        //z: -9500.0,
        z: -250.0,
    };

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct MvpBlock {
        pub mvp: Matrix4<f32>,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct AttractorBlock {
        pub attractor: [Vector4<f32>; MAX_ATTRACTORS],
        pub dt: f32,
        pub _pad0: [f32; 3],
    }

    let mut mvp_block_data = vec![MvpBlock { mvp: Zero::zero() }; 1];

    let mut attractor_block_data = vec![
        AttractorBlock {
            attractor: [Vector4::zero(); MAX_ATTRACTORS],
            dt: 0.0,
            _pad0: Default::default(),
        };
        1
    ];

    let mut encoder = app.device.create_command_encoder(None)?;

    let attractor_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsageFlags::MAP_WRITE | BufferUsageFlags::UNIFORM,
        &attractor_block_data,
    )?;

    let mut rng = rand::thread_rng();

    let position_data: Vec<Vector4<f32>> = std::iter::repeat(Vector4::<f32>::zero())
        .take(PARTICLE_COUNT)
        .map(|_| {
            let x = rng.gen_range(-0.1, 0.1);
            let y = rng.gen_range(-0.1, 0.1);
            let z = rng.gen_range(-0.1, 0.1);
            let w = rng.gen();
            Vector4::new(x, y, z, w)
        })
        .collect();

    let velocity_data: Vec<Vector4<f32>> = std::iter::repeat(Vector4::<f32>::zero())
        .take(PARTICLE_COUNT)
        .map(|_| {
            let x = rng.gen_range(-0.05, 0.05);
            let y = rng.gen_range(-0.05, 0.05);
            let z = rng.gen_range(-0.05, 0.05);
            let w = 0.0;
            Vector4::new(x, y, z, w)
        })
        .collect();;

    let velocity_buffer =
        util::create_buffer_with_data(&app.device, &mut encoder, BufferUsageFlags::STORAGE, &velocity_data)?;

    let velocity_buffer_view = velocity_buffer.create_view(BufferViewDescriptor {
        offset: 0,
        format: BufferViewFormat::Texture(TextureFormat::RGBA32Float),
        size: velocity_buffer.size(),
    })?;

    let position_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsageFlags::STORAGE | BufferUsageFlags::VERTEX,
        &position_data,
    )?;

    let position_buffer_view = position_buffer.create_view(BufferViewDescriptor {
        offset: 0,
        format: BufferViewFormat::Texture(TextureFormat::RGBA32Float),
        size: position_buffer.size(),
    })?;

    let mvp_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
        &mvp_block_data,
    )?;

    app.device.get_queue().submit(&[encoder.finish()?])?;

    #[rustfmt::skip]
    let compute_bind_group_layout = app.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        bindings: vec![
            BindGroupLayoutBinding {
                binding: 0,
                binding_type: BindingType::StorageTexelBuffer,
                visibility: ShaderStageFlags::COMPUTE,
            },
            BindGroupLayoutBinding {
                binding: 1,
                binding_type: BindingType::StorageTexelBuffer,
                visibility: ShaderStageFlags::COMPUTE,
            },
            BindGroupLayoutBinding {
                binding: 2,
                binding_type: BindingType::UniformBuffer,
                visibility: ShaderStageFlags::COMPUTE,
            }
        ],
    })?;

    #[rustfmt::skip]
    let compute_bind_group = app.device.create_bind_group(&BindGroupDescriptor {
        layout: compute_bind_group_layout.clone(),
        bindings: vec![
            BindGroupBinding {
                binding: 0,
                resource: BindingResource::BufferView(velocity_buffer_view),
            },
            BindGroupBinding {
                binding: 1,
                resource: BindingResource::BufferView(position_buffer_view),
            },
            BindGroupBinding {
                binding: 2,
                resource: BindingResource::Buffer(attractor_buffer.clone(), 0..util::byte_length(&attractor_block_data)),
            }
        ],
    })?;

    #[rustfmt::skip]
    let render_bind_group_layout = app.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        bindings: vec![
            BindGroupLayoutBinding {
                binding: 0,
                binding_type: BindingType::UniformBuffer,
                visibility: ShaderStageFlags::VERTEX,
            },
        ],
    })?;

    #[rustfmt::skip]
    let render_bind_group = app.device.create_bind_group(&BindGroupDescriptor {
        layout: render_bind_group_layout.clone(),
        bindings: vec![
            BindGroupBinding {
                binding: 0,
                resource: BindingResource::Buffer(mvp_buffer.clone(), 0..util::byte_length(&mvp_block_data)),
            }
        ],
    })?;

    let compute_pipeline_layout = app.device.create_pipeline_layout(&PipelineLayoutDescriptor {
        bind_group_layouts: vec![compute_bind_group_layout],
        push_constant_ranges: vec![],
    })?;

    let render_pipeline_layout = app.device.create_pipeline_layout(&PipelineLayoutDescriptor {
        bind_group_layouts: vec![render_bind_group_layout],
        push_constant_ranges: vec![],
    })?;

    let vs = app.device.create_shader_module(&ShaderModuleDescriptor {
        code: include_bytes!("shaders/particle_simulator.vert.spv"),
    })?;

    let fs = app.device.create_shader_module(&ShaderModuleDescriptor {
        code: include_bytes!("shaders/particle_simulator.frag.spv"),
    })?;

    let cs = app.device.create_shader_module(&ShaderModuleDescriptor {
        code: include_bytes!("shaders/particle_simulator.comp.spv"),
    })?;

    let compute_pipeline = app.device.create_compute_pipeline(&ComputePipelineDescriptor {
        layout: compute_pipeline_layout,
        compute_stage: PipelineStageDescriptor {
            module: cs,
            entry_point: Cow::Borrowed("main"),
        },
    })?;

    #[rustfmt::skip]
    let render_pipeline = app.device.create_render_pipeline(&RenderPipelineDescriptor {
        layout: render_pipeline_layout,
        vertex_stage: PipelineStageDescriptor { module: vs, entry_point: Cow::Borrowed("main") },
        fragment_stage: PipelineStageDescriptor { module: fs, entry_point: Cow::Borrowed("main") },
        rasterization_state: RasterizationStateDescriptor {
            front_face: FrontFace::Ccw,
            cull_mode: CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        },
        primitive_topology: PrimitiveTopology::PointList,
        color_states: vec![
            ColorStateDescriptor {
                format: util::DEFAULT_COLOR_FORMAT,
                color_blend: BlendDescriptor {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha_blend: BlendDescriptor::OPAQUE,
                write_mask: ColorWriteFlags::ALL,
            }
        ],
        depth_stencil_state: None,
        input_state: InputStateDescriptor {
            index_format: IndexFormat::U16,
            vertex_buffers: vec![
                VertexBufferDescriptor {
                    input_slot: 0,
                    stride: util::byte_stride(&position_data),
                    step_mode: InputStepMode::Vertex,
                    attributes: vec![
                        VertexAttributeDescriptor {
                            format: VertexFormat::Float4,
                            offset: 0,
                            shader_location: 0,
                        }
                    ],
                }
            ],
        },
        sample_count: app.get_sample_count(),
    })?;

    let start_time = Instant::now();
    let mut last_frame_time_secs = 0.0;

    let mut fence: Option<Fence> = None;

    let mut rng = rand::thread_rng();

    let attractor_masses: Vec<f32> = std::iter::repeat(0.0)
        .take(MAX_ATTRACTORS)
        .map(|_| rng.gen_range(0.5, 1.0))
        .collect();

    let mut last_fps_time = Instant::now();
    let mut last_fps_frame_count = 0;

    app.run(move |app| {
        let model = cgmath::Matrix4::identity();

        let time = (start_time.elapsed().as_millis() as f32) / 1000.0;
        let delta = time - last_frame_time_secs;
        last_frame_time_secs = time;

        last_fps_frame_count += 1;

        if last_fps_time.elapsed() > Duration::from_millis(1000) {
            println!("FPS: {}", last_fps_frame_count);
            last_fps_frame_count = 0;
            last_fps_time = Instant::now();
        }

        mvp_block_data[0].mvp = (app.camera.projection * app.camera.view * model).into();

        mvp_buffer.set_sub_data(0, &mvp_block_data)?;

        for (i, attractor) in attractor_block_data[0].attractor.iter_mut().enumerate() {
            let w = attractor_masses[i];
            let i = i as f32;
            let x = (time * (i + 4.0) * 7.5 * 20.0).sin() * 50.0;
            let y = (time * (i + 7.0) * 3.9 * 20.0).cos() * 50.0;
            let z = (time * (i + 3.0) * 5.3 * 20.0).sin() + (time * (i + 5.0) * 9.1).cos() * 100.0;
            *attractor = Vector4::new(x, y, z, w).into();
        }

        attractor_block_data[0].dt = delta * 200.0;

        if let Some(ref fence) = fence {
            fence.wait(Duration::from_millis(1_000_000_000))?;
            fence.reset()?;
        }

        let mapped_attractor_data = attractor_buffer.map_write()?;
        mapped_attractor_data.copy_from_slice(&attractor_block_data)?;

        let frame = match app.swapchain.acquire_next_image() {
            Ok(frame) => frame,
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        };

        let mut encoder = app.device.create_command_encoder(None)?;

        if app.state.reset1 {
            util::copy_to_buffer(&app.device, &mut encoder, &position_data, &position_buffer)?;
            util::copy_to_buffer(&app.device, &mut encoder, &velocity_data, &velocity_buffer)?;
            app.state.reset1 = false;
        }

        if app.state.reset2 {
            util::copy_to_buffer(&app.device, &mut encoder, &position_data, &position_buffer)?;
            app.state.reset2 = false;
        }

        let (attachment, resolve_target) = if app.get_sample_count() == 1 {
            (&frame.view, None)
        } else {
            (&app.color_view, Some(&frame.view))
        };

        let mut compute_pass = encoder.begin_compute_pass();
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &compute_bind_group, None);
        compute_pass.dispatch(PARTICLE_GROUP_COUNT as u32, 1, 1);
        compute_pass.end_pass();

        #[rustfmt::skip]
        let mut render_pass = encoder.begin_render_pass(RenderPassDescriptor {
            color_attachments: &[
                RenderPassColorAttachmentDescriptor {
                    attachment,
                    resolve_target,
                    store_op: StoreOp::Store,
                    load_op: LoadOp::Clear,
                    clear_color: Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 },
                }
            ],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&render_pipeline);
        render_pass.set_bind_group(0, &render_bind_group, None);
        render_pass.set_vertex_buffers(0, &[position_buffer.clone()], &[0]);
        render_pass.draw(PARTICLE_COUNT as u32, 1, 0, 0);
        render_pass.end_pass();

        let command_buffer = encoder.finish()?;

        let queue = app.device.get_queue();

        queue.submit(&[command_buffer])?;

        if fence.is_none() {
            fence = Some(queue.create_fence()?);
        }

        match queue.present(frame) {
            Ok(frame) => frame,
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        }

        Ok(())
    })
}
