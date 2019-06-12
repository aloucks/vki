#[macro_use]
extern crate memoffset;

pub mod util;

use cgmath::Point3;
use cgmath::SquareMatrix;

use std::borrow::Cow;

use crate::util::{App, EventHandlers};

use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BlendDescriptor, BufferUsageFlags, Color, ColorStateDescriptor, ColorWriteFlags, CompareFunction,
    CullMode, DepthStencilStateDescriptor, FrontFace, IndexFormat, InputStateDescriptor, InputStepMode, LoadOp,
    PipelineLayoutDescriptor, PipelineStageDescriptor, PrimitiveTopology, RasterizationStateDescriptor,
    RenderPassColorAttachmentDescriptor, RenderPassDepthStencilAttachmentDescriptor, RenderPassDescriptor,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStageFlags, StencilStateFaceDescriptor, StoreOp,
    SwapchainError, VertexAttributeDescriptor, VertexBufferDescriptor, VertexFormat,
};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PositionColor {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = pretty_env_logger::try_init();

    let mut app: App<()> = App::init("cube.rs", 800, 600, EventHandlers::Default)?;

    app.set_sample_count(8)?;

    app.camera.eye = Point3 {
        x: 2.0,
        y: 2.0,
        z: -2.0,
    };

    // Note that we use a clip space correction matrix with the projection so that these vertices
    // can be specified in a right handed coordinate system (with positive `y`) and counter-clockwise
    // wind ordering.
    #[rustfmt::skip]
    let vertices = vec![
        PositionColor { position: [-0.5,  0.5,  0.5], color: [0.0, 1.0, 0.0, 1.0] }, // 0 - green
        PositionColor { position: [-0.5, -0.5,  0.5], color: [0.0, 0.0, 0.0, 1.0] }, // 1 - black
        PositionColor { position: [ 0.5,  0.5,  0.5], color: [0.0, 1.0, 1.0, 1.0] }, // 2 - cyan
        PositionColor { position: [ 0.5, -0.5,  0.5], color: [0.0, 0.0, 1.0, 1.0] }, // 3 - blue

        PositionColor { position: [-0.5,  0.5, -0.5], color: [1.0, 1.0, 0.0, 1.0] }, // 4 - yellow
        PositionColor { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 0.0, 1.0] }, // 5 - red
        PositionColor { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0, 1.0] }, // 6 - white
        PositionColor { position: [ 0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0, 1.0] }, // 7 - magenta
    ];

    #[rustfmt::skip]
    let indices: &[u16] = &[
        // front
        0, 1, 2,
        2, 1, 3,

        // back
        7, 5, 6,
        6, 5, 4,

        // right
        3, 7, 2,
        2, 7, 6,

        // left
        4, 5, 0,
        0, 5, 1,

        // top
        4, 0, 6,
        6, 0, 2,

        // bottom
        3, 1, 7,
        7, 1, 5,
    ];

    #[repr(C)]
    #[derive(Debug, Default, Copy, Clone)]
    pub struct Uniforms {
        pub projection: [[f32; 4]; 4],
        pub view: [[f32; 4]; 4],
        pub model: [[f32; 4]; 4],
        pub time: f32,
        pub _pad0: [f32; 3],
    }

    let mut uniforms = vec![Uniforms::default(); 1];

    let mut encoder = app.device.create_command_encoder()?;

    let vertex_buffer = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsageFlags::VERTEX, &vertices)?;
    let index_buffer = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsageFlags::INDEX, &indices)?;
    let uniform_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
        &uniforms,
    )?;

    app.device.get_queue().submit(&[encoder.finish()?])?;

    #[rustfmt::skip]
    let bind_group_layout = app.device.create_bind_group_layout(BindGroupLayoutDescriptor {
        bindings: vec![
            BindGroupLayoutBinding {
                binding: 0,
                binding_type: BindingType::UniformBuffer,
                visibility: ShaderStageFlags::FRAGMENT | ShaderStageFlags::VERTEX,
            }
        ],
    })?;

    #[rustfmt::skip]
    let bind_group = app.device.create_bind_group(BindGroupDescriptor {
        layout: bind_group_layout.clone(),
        bindings: vec![
            BindGroupBinding {
                binding: 0,
                resource: BindingResource::Buffer(uniform_buffer.clone(), 0..util::byte_length(&uniforms)),
            }
        ],
    })?;

    let pipeline_layout = app.device.create_pipeline_layout(PipelineLayoutDescriptor {
        bind_group_layouts: vec![bind_group_layout],
        push_constant_ranges: vec![],
    })?;

    let vs = app.device.create_shader_module(ShaderModuleDescriptor {
        code: Cow::Borrowed(include_bytes!("shaders/cube.vert.spv")),
    })?;

    let fs = app.device.create_shader_module(ShaderModuleDescriptor {
        code: Cow::Borrowed(include_bytes!("shaders/cube.frag.spv")),
    })?;

    #[rustfmt::skip]
    let render_pipeline = app.device.create_render_pipeline(RenderPipelineDescriptor {
        layout: pipeline_layout,
        vertex_stage: PipelineStageDescriptor { module: vs, entry_point: Cow::Borrowed("main") },
        fragment_stage: PipelineStageDescriptor { module: fs, entry_point: Cow::Borrowed("main") },
        rasterization_state: RasterizationStateDescriptor {
            front_face: FrontFace::Ccw,
            cull_mode: CullMode::Back,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        },
        primitive_topology: PrimitiveTopology::TriangleList,
        color_states: vec![
            ColorStateDescriptor {
                format: util::DEFAULT_COLOR_FORMAT,
                color_blend: BlendDescriptor::OPAQUE,
                alpha_blend: BlendDescriptor::OPAQUE,
                write_mask: ColorWriteFlags::ALL,
            }
        ],
        depth_stencil_state: Some(DepthStencilStateDescriptor {
            format: util::DEFAULT_DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: CompareFunction::Less,
            stencil_back: StencilStateFaceDescriptor::IGNORE,
            stencil_front: StencilStateFaceDescriptor::IGNORE,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
        }),
        input_state: InputStateDescriptor {
            index_format: IndexFormat::U16,
            vertex_buffers: vec![
                VertexBufferDescriptor {
                    input_slot: 0,
                    stride: util::byte_stride(&vertices),
                    step_mode: InputStepMode::Vertex,
                    attributes: vec![
                        VertexAttributeDescriptor {
                            format: VertexFormat::Float3,
                            offset: offset_of!(PositionColor, position),
                            shader_location: 0,
                        },
                        VertexAttributeDescriptor {
                            format: VertexFormat::Float4,
                            offset: offset_of!(PositionColor, color),
                            shader_location: 1,
                        }
                    ],
                }
            ],
        },
        sample_count: app.get_sample_count(),
    })?;

    app.run(move |app| {
        let model = cgmath::Matrix4::identity();

        uniforms[0].time = 0.0;
        uniforms[0].projection = app.camera.projection.into();
        uniforms[0].model = model.into();
        uniforms[0].view = app.camera.view.into();

        uniform_buffer.set_sub_data(0, &uniforms)?;

        let frame = match app.swapchain.acquire_next_image() {
            Ok(frame) => frame,
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        };

        let mut encoder = app.device.create_command_encoder()?;

        let (attachment, resolve_target) = if app.get_sample_count() == 1 {
            (&frame.view, None)
        } else {
            (&app.color_view, Some(&frame.view))
        };

        #[rustfmt::skip]
        let mut render_pass = encoder.begin_render_pass(RenderPassDescriptor {
            color_attachments: &[
                RenderPassColorAttachmentDescriptor {
                    attachment,
                    resolve_target,
                    store_op: StoreOp::Store,
                    load_op: LoadOp::Clear,
                    clear_color: Color { r: 0.2, g: 0.6, b: 0.8, a: 1.0 },
                }
            ],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachmentDescriptor {
                attachment: &app.depth_view,
                clear_depth: 1.0,
                clear_stencil: 0,
                depth_load_op: LoadOp::Clear,
                depth_store_op: StoreOp::Store,
                stencil_load_op: LoadOp::Clear,
                stencil_store_op: StoreOp::Store,
            }),
        });

        render_pass.set_pipeline(&render_pipeline);
        render_pass.set_bind_group(0, &bind_group, None);
        render_pass.set_vertex_buffers(0, &[vertex_buffer.clone()], &[0]);
        render_pass.set_index_buffer(&index_buffer, 0);
        render_pass.draw_indexed(indices.len() as u32, 1, 0, 0, 0);
        render_pass.end_pass();

        let command_buffer = encoder.finish()?;

        let queue = app.device.get_queue();

        queue.submit(&[command_buffer])?;

        match queue.present(frame) {
            Ok(frame) => frame,
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        }

        Ok(())
    })
}
