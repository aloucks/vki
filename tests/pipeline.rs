#![allow(unused_imports)]

use std::borrow::Cow;
use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BlendDescriptor, BlendFactor, BlendOperation, BufferDescriptor, BufferUsageFlags, Color,
    ColorStateDescriptor, ColorWriteFlags, CompareFunction, ComputePipelineDescriptor, CullMode,
    DepthStencilStateDescriptor, Extent3D, FrontFace, IndexFormat, InputStateDescriptor, InputStepMode, LoadOp,
    PipelineLayoutDescriptor, PipelineStageDescriptor, PrimitiveTopology, RasterizationStateDescriptor,
    RenderPassColorAttachmentDescriptor, RenderPassDescriptor, RenderPipelineDescriptor, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderStageFlags, StencilOperation, StencilStateFaceDescriptor, StoreOp, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags, TextureView, VertexAttributeDescriptor,
    VertexFormat, VertexInputDescriptor,
};

pub mod support;

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

// Emulate a SwpachainImage
struct Frame {
    #[allow(unused)]
    texture: Texture,
    view: TextureView,
}

#[test]
fn create_pipeline_layout() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let bind_group_layout_descriptor = BindGroupLayoutDescriptor {
            bindings: &[
                BindGroupLayoutBinding {
                    binding: 0,
                    visibility: ShaderStageFlags::VERTEX,
                    binding_type: BindingType::UniformBuffer,
                },
                BindGroupLayoutBinding {
                    binding: 1,
                    visibility: ShaderStageFlags::FRAGMENT,
                    binding_type: BindingType::Sampler,
                },
                BindGroupLayoutBinding {
                    binding: 2,
                    visibility: ShaderStageFlags::FRAGMENT,
                    binding_type: BindingType::SampledTexture,
                },
            ],
        };
        let bind_group_layout = device.create_bind_group_layout(bind_group_layout_descriptor)?;

        let pipeline_layout_descriptor = PipelineLayoutDescriptor {
            bind_group_layouts: vec![bind_group_layout],
        };

        let _pipeline_layout = device.create_pipeline_layout(pipeline_layout_descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_compute_pipeline() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let shader_module_descriptor = ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/pipeline.comp.spv")),
        };
        let shader_module = device.create_shader_module(shader_module_descriptor)?;

        let bind_group_layout_descriptor = BindGroupLayoutDescriptor {
            bindings: &[
                BindGroupLayoutBinding {
                    binding: 0,
                    visibility: ShaderStageFlags::COMPUTE,
                    binding_type: BindingType::UniformBuffer,
                },
                BindGroupLayoutBinding {
                    binding: 1,
                    visibility: ShaderStageFlags::COMPUTE,
                    binding_type: BindingType::StorageBuffer,
                },
            ],
        };
        let bind_group_layout = device.create_bind_group_layout(bind_group_layout_descriptor)?;

        let pipeline_layout_descriptor = PipelineLayoutDescriptor {
            bind_group_layouts: vec![bind_group_layout],
        };

        let pipeline_layout = device.create_pipeline_layout(pipeline_layout_descriptor)?;

        let pipeline_stage_descriptor = PipelineStageDescriptor {
            entry_point: Cow::Borrowed("main"),
            module: shader_module,
        };

        let compute_pipeline_descriptor = ComputePipelineDescriptor {
            layout: pipeline_layout,
            compute_stage: pipeline_stage_descriptor,
        };

        let _compute_pipeline = device.create_compute_pipeline(compute_pipeline_descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_render_pipeline() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let vertex_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/pipeline.vert.spv")),
        })?;

        let fragment_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/pipeline.frag.spv")),
        })?;

        #[rustfmt::skip]
        let bind_group_layout = device.create_bind_group_layout(BindGroupLayoutDescriptor {
            bindings: &[
                BindGroupLayoutBinding {
                    binding: 0,
                    visibility: ShaderStageFlags::VERTEX,
                    binding_type: BindingType::UniformBuffer,
                }
            ],
        })?;

        let pipeline_layout = device.create_pipeline_layout(PipelineLayoutDescriptor {
            bind_group_layouts: vec![bind_group_layout],
        })?;

        #[repr(C)]
        struct Vertex {
            position: [f32; 3],
            normal: [f32; 3],
        }

        let color_replace = BlendDescriptor {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        };

        let stencil_disabled = StencilStateFaceDescriptor {
            compare: CompareFunction::Always,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            pass_op: StencilOperation::Keep,
        };

        #[rustfmt::skip]
        let render_pipeline_descriptor = RenderPipelineDescriptor {
            layout: pipeline_layout,
            primitive_topology: PrimitiveTopology::TriangleList,
            vertex_stage: PipelineStageDescriptor {
                entry_point: Cow::Borrowed("main"),
                module: vertex_shader_module,
            },
            fragment_stage: PipelineStageDescriptor {
                entry_point: Cow::Borrowed("main"),
                module: fragment_shader_module,
            },
            input_state: InputStateDescriptor {
                index_format: IndexFormat::U16,
                inputs: vec![
                    VertexInputDescriptor {
                        input_slot: 0,
                        step_mode: InputStepMode::Vertex,
                        stride: std::mem::size_of::<Vertex>(),
                    }
                ],
                attributes: vec![
                    VertexAttributeDescriptor {
                        input_slot: 0,
                        format: VertexFormat::Float3,
                        offset: offset_of!(Vertex, position),
                        shader_location: 0,
                    },
                    VertexAttributeDescriptor {
                        input_slot: 0,
                        format: VertexFormat::Float3,
                        offset: offset_of!(Vertex, normal),
                        shader_location: 1,
                    },
                ],
            },
            color_states: vec![
                ColorStateDescriptor {
                    format: TextureFormat::B8G8R8A8Unorm,
                    write_mask: ColorWriteFlags::ALL,
                    color_blend: color_replace,
                    alpha_blend: color_replace,
                }
            ],
            depth_stencil_state: Some(DepthStencilStateDescriptor {
                format: TextureFormat::D32FloatS8Uint,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil_back: stencil_disabled,
                stencil_front: stencil_disabled,
                stencil_write_mask: 0,
                stencil_read_mask: 0,
            }),
            rasterization_state: RasterizationStateDescriptor {
                front_face: FrontFace::Ccw,
                cull_mode: CullMode::Back,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            },
            sample_count: 1,
        };

        let _render_pipeline = device.create_render_pipeline(render_pipeline_descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_multi_sample_render_pipeline() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let vertex_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/pipeline.vert.spv")),
        })?;

        let fragment_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/pipeline.frag.spv")),
        })?;

        #[rustfmt::skip]
        let bind_group_layout = device.create_bind_group_layout(BindGroupLayoutDescriptor {
            bindings: &[
                BindGroupLayoutBinding {
                    binding: 0,
                    visibility: ShaderStageFlags::VERTEX,
                    binding_type: BindingType::UniformBuffer,
                }
            ],
        })?;

        let uniform_buffer_size = (std::mem::size_of::<f32>() * 16) as _;

        let uniform_buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
            size: uniform_buffer_size,
        })?;

        let bind_group = device.create_bind_group(BindGroupDescriptor {
            layout: bind_group_layout.clone(),
            bindings: vec![BindGroupBinding {
                binding: 0,
                resource: BindingResource::Buffer(uniform_buffer, 0..uniform_buffer_size),
            }],
        })?;

        let pipeline_layout = device.create_pipeline_layout(PipelineLayoutDescriptor {
            bind_group_layouts: vec![bind_group_layout],
        })?;

        #[repr(C)]
        struct Vertex {
            position: [f32; 3],
            normal: [f32; 3],
        }

        let vertex_buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsageFlags::VERTEX,
            size: (3 * std::mem::size_of::<Vertex>()) as _,
        })?;

        let color_replace = BlendDescriptor {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        };

        let sample_count = 4;

        #[rustfmt::skip]
        let render_pipeline_descriptor = RenderPipelineDescriptor {
            layout: pipeline_layout,
            primitive_topology: PrimitiveTopology::TriangleList,
            vertex_stage: PipelineStageDescriptor {
                entry_point: Cow::Borrowed("main"),
                module: vertex_shader_module,
            },
            fragment_stage: PipelineStageDescriptor {
                entry_point: Cow::Borrowed("main"),
                module: fragment_shader_module,
            },
            input_state: InputStateDescriptor {
                index_format: IndexFormat::U16,
                inputs: vec![
                    VertexInputDescriptor {
                        input_slot: 0,
                        step_mode: InputStepMode::Vertex,
                        stride: std::mem::size_of::<Vertex>(),
                    }
                ],
                attributes: vec![
                    VertexAttributeDescriptor {
                        input_slot: 0,
                        format: VertexFormat::Float3,
                        offset: offset_of!(Vertex, position),
                        shader_location: 0,
                    },
                    VertexAttributeDescriptor {
                        input_slot: 0,
                        format: VertexFormat::Float3,
                        offset: offset_of!(Vertex, normal),
                        shader_location: 1,
                    },
                ],
            },
            color_states: vec![
                ColorStateDescriptor {
                    format: TextureFormat::B8G8R8A8Unorm,
                    write_mask: ColorWriteFlags::ALL,
                    color_blend: color_replace,
                    alpha_blend: color_replace,
                }
            ],
            depth_stencil_state: None,
            rasterization_state: RasterizationStateDescriptor {
                front_face: FrontFace::Ccw,
                cull_mode: CullMode::Back,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            },
            sample_count,
        };

        let pipeline = device.create_render_pipeline(render_pipeline_descriptor)?;

        let size = Extent3D {
            width: 800,
            height: 600,
            depth: 1,
        };
        let mip_level_count = 1;
        let array_layer_count = 1;
        let dimension = TextureDimension::D2;
        let format = TextureFormat::B8G8R8A8Unorm;
        let usage = TextureUsageFlags::OUTPUT_ATTACHMENT;

        let frame_texture = device.create_texture(TextureDescriptor {
            sample_count: 1,
            size,
            mip_level_count,
            array_layer_count,
            dimension,
            format,
            usage,
        })?;

        let frame_view = frame_texture.create_default_view()?;

        let frame = Frame {
            view: frame_view,
            texture: frame_texture,
        };

        let output_texture = device.create_texture(TextureDescriptor {
            sample_count,
            size,
            mip_level_count,
            array_layer_count,
            dimension,
            format,
            usage,
        })?;

        let output_view = output_texture.create_default_view()?;

        let mut encoder = device.create_command_encoder()?;

        let mut render_pass = encoder.begin_render_pass(RenderPassDescriptor {
            color_attachments: &[RenderPassColorAttachmentDescriptor {
                attachment: &output_view,
                clear_color: Color {
                    r: 0.1,
                    g: 0.1,
                    b: 0.1,
                    a: 1.0,
                },
                load_op: LoadOp::Clear,
                store_op: StoreOp::Store,
                resolve_target: Some(&frame.view),
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, None);
        render_pass.set_vertex_buffers(0, &[vertex_buffer.clone()], &[0]);
        render_pass.draw(3, 1, 0, 0);
        render_pass.end_pass();

        let queue = device.get_queue();
        queue.submit(encoder.finish()?)?;

        Ok(instance)
    });
}
