#![allow(unused_imports)]

use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BufferDescriptor, BufferUsageFlags, Extent3D, PipelineLayoutDescriptor, SamplerDescriptor,
    ShaderStageFlags, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags,
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
