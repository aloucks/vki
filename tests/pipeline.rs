#![allow(unused_imports)]

use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BufferDescriptor, BufferUsageFlags, ComputePipelineDescriptor, Extent3D, PipelineLayoutDescriptor,
    PipelineStageDescriptor, SamplerDescriptor, ShaderModuleDescriptor, ShaderStageFlags, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsageFlags,
};

pub mod support;

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
            bind_group_layouts: &[bind_group_layout],
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
            code: include_bytes!("data/pipeline.comp.spv"),
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
            bind_group_layouts: &[bind_group_layout],
        };

        let pipeline_layout = device.create_pipeline_layout(pipeline_layout_descriptor)?;

        let pipeline_stage_descriptor = PipelineStageDescriptor {
            entry_point: "main",
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
