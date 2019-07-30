#[macro_use]
extern crate memoffset;

pub mod util;

use cgmath::SquareMatrix;
use cgmath::{Point3, Vector3};

use num_traits::Zero;

use std::borrow::Cow;

use crate::util::{App, EventHandlers};

use std::time::Instant;
use vki::{
    AddressMode, BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor,
    BindingResource, BindingType, BlendDescriptor, BufferCopyView, BufferUsageFlags, Color, ColorStateDescriptor,
    ColorWriteFlags, CompareFunction, CullMode, DepthStencilStateDescriptor, Extent3D, FilterMode, FrontFace,
    IndexFormat, InputStateDescriptor, InputStepMode, LoadOp, Origin3D, PipelineLayoutDescriptor,
    PipelineStageDescriptor, PrimitiveTopology, RasterizationStateDescriptor, RenderPassColorAttachmentDescriptor,
    RenderPassDepthStencilAttachmentDescriptor, RenderPassDescriptor, RenderPipelineDescriptor, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderStageFlags, StencilStateFaceDescriptor, StoreOp, SwapchainError, TextureBlitView,
    TextureCopyView, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags, VertexAttributeDescriptor,
    VertexBufferDescriptor, VertexFormat,
};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct PositionTexcoord {
    pub position: [f32; 3],
    pub texcoord: [f32; 2],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = pretty_env_logger::try_init();

    let mut app: App<()> = App::init("cube_texture.rs", 800, 600, EventHandlers::Default)?;

    app.set_sample_count(8)?;

    app.camera.eye = Point3 {
        x: 2.0,
        y: 2.0,
        z: -2.0,
    };

    let cube = util::shape::Cube {
        center: Vector3::zero(),
        x_extent: 0.5,
        y_extent: 0.5,
        z_extent: 0.5,
    };

    let vertices = cube
        .positions()
        .iter()
        .zip(cube.texcoords().iter())
        .map(|(position, texcoord)| PositionTexcoord {
            position: (*position).into(),
            texcoord: (*texcoord).into(),
        })
        .collect::<Vec<PositionTexcoord>>();

    let indices = cube.indices();

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

    let mut encoder = app.device.create_command_encoder(None)?;

    let vertex_buffer = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsageFlags::VERTEX, &vertices)?;
    let index_buffer = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsageFlags::INDEX, &indices)?;
    let uniform_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
        &uniforms,
    )?;

    let image = image::load_from_memory(include_bytes!("assets/container2.png"))?.to_rgba();

    let texture_size = Extent3D {
        width: image.width(),
        height: image.height(),
        depth: 1,
    };

    let sampler = app.device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        lod_max_clamp: 1000.0,
        lod_min_clamp: 0.0,
        mipmap_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        compare_function: CompareFunction::Never,
    })?;

    // create texture

    let mip_level_count = (texture_size.width.max(texture_size.height) as f32).log2().floor() as u32 + 1;

    println!("lod mip_levels: {}", mip_level_count);

    let container_texture = app.device.create_texture(&TextureDescriptor {
        mip_level_count,
        sample_count: 1,
        array_layer_count: 1,
        format: TextureFormat::R8G8B8A8Unorm,
        usage: TextureUsageFlags::SAMPLED | TextureUsageFlags::TRANSFER_DST | TextureUsageFlags::TRANSFER_SRC,
        size: texture_size,
        dimension: TextureDimension::D2,
    })?;

    let container_texture_view = container_texture.create_default_view()?;

    let texture_buffer = util::create_staging_buffer(&app.device, &image)?;

    encoder.copy_buffer_to_texture(
        BufferCopyView {
            offset: 0,
            buffer: &texture_buffer,
            image_height: texture_size.height,
            row_length: texture_size.width,
        },
        TextureCopyView {
            texture: &container_texture,
            origin: Origin3D { x: 0, y: 0, z: 0 },
            mip_level: 0,
            array_layer: 0,
        },
        texture_size,
    );

    // generate mipmaps

    let mut mip_width = texture_size.width;
    let mut mip_height = texture_size.height;

    for i in 1..mip_level_count {
        let src = TextureBlitView {
            texture: &container_texture,
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
            texture: &container_texture,
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

    app.device.get_queue().submit(&[encoder.finish()?])?;

    #[rustfmt::skip]
    let bind_group_layout = app.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        bindings: vec![
            BindGroupLayoutBinding {
                binding: 0,
                binding_type: BindingType::UniformBuffer,
                visibility: ShaderStageFlags::FRAGMENT | ShaderStageFlags::VERTEX,
            },
            BindGroupLayoutBinding {
                binding: 1,
                binding_type: BindingType::Sampler,
                visibility: ShaderStageFlags::FRAGMENT,
            },
            BindGroupLayoutBinding {
                binding: 2,
                binding_type: BindingType::SampledTexture,
                visibility: ShaderStageFlags::FRAGMENT,
            }
        ],
    })?;

    #[rustfmt::skip]
    let bind_group = app.device.create_bind_group(&BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[
            BindGroupBinding {
                binding: 0,
                resource: BindingResource::Buffer(uniform_buffer.clone(), 0..util::byte_length(&uniforms)),
            },
            BindGroupBinding {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
            BindGroupBinding {
                binding: 2,
                resource: BindingResource::TextureView(container_texture_view),
            }
        ],
    })?;

    let pipeline_layout = app.device.create_pipeline_layout(&PipelineLayoutDescriptor {
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    })?;

    let vs = app.device.create_shader_module(&ShaderModuleDescriptor {
        code: include_bytes!("shaders/cube_texture.vert.spv"),
    })?;

    let fs = app.device.create_shader_module(&ShaderModuleDescriptor {
        code: include_bytes!("shaders/cube_texture.frag.spv"),
    })?;

    #[rustfmt::skip]
    let render_pipeline = app.device.create_render_pipeline(&RenderPipelineDescriptor {
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
                            offset: offset_of!(PositionTexcoord, position),
                            shader_location: 0,
                        },
                        VertexAttributeDescriptor {
                            format: VertexFormat::Float4,
                            offset: offset_of!(PositionTexcoord, texcoord),
                            shader_location: 1,
                        }
                    ],
                }
            ],
        },
        sample_count: app.get_sample_count(),
    })?;

    let start = Instant::now();

    app.run(move |app| {
        let model = cgmath::Matrix4::identity();

        uniforms[0].time = (start.elapsed().as_millis() as f32) / 1000.0;
        uniforms[0].projection = app.camera.projection.into();
        uniforms[0].model = model.into();
        uniforms[0].view = app.camera.view.into();

        uniform_buffer.set_sub_data(0, &uniforms)?;

        let frame = match app.swapchain.acquire_next_image() {
            Ok(frame) => frame,
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        };

        let mut encoder = app.device.create_command_encoder(None)?;

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
