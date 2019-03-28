use vki::{BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingType, ShaderStageFlags};

pub mod support;

#[test]
fn create_sampler() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let bind_group_layout_descriptor = BindGroupLayoutDescriptor {
            bindings: &[BindGroupLayoutBinding {
                binding: 0,
                visibility: ShaderStageFlags::VERTEX,
                binding_type: BindingType::UniformBuffer,
            }],
        };

        let _bind_group_layout = device.create_bind_group_layout(bind_group_layout_descriptor)?;

        Ok(instance)
    });
}
