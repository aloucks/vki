#![allow(unused_imports)]

use std::borrow::Cow;
use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BlendDescriptor, BlendFactor, BlendOperation, BufferDescriptor, BufferUsageFlags,
    ColorStateDescriptor, ColorWriteFlags, CompareFunction, ComputePipelineDescriptor, CullMode,
    DepthStencilStateDescriptor, Extent3D, FrontFace, IndexFormat, InputStateDescriptor, InputStepMode,
    PipelineLayoutDescriptor, PipelineStageDescriptor, PrimitiveTopology, RasterizationStateDescriptor,
    RenderPipelineDescriptor, SamplerDescriptor, ShaderModuleDescriptor, ShaderStageFlags, StencilOperation,
    StencilStateFaceDescriptor, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags,
    VertexAttributeDescriptor, VertexFormat, VertexInputDescriptor,
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
                        stride: std::mem::size_of::<Vertex>() as u64,
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
        };

        let _render_pipeline = device.create_render_pipeline(render_pipeline_descriptor)?;

        Ok(instance)
    });
}
