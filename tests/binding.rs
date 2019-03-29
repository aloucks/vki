use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BufferDescriptor, BufferUsageFlags, Extent3D, SamplerDescriptor, ShaderStageFlags, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsageFlags,
};

pub mod support;

#[test]
fn create_bind_group_layout() {
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

#[test]
fn create_bind_group() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let buffer_descriptor = BufferDescriptor {
            usage: BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
            size: 1024,
        };
        let buffer = device.create_buffer(buffer_descriptor)?;

        let sampler_descriptor = SamplerDescriptor::default();
        let sampler = device.create_sampler(sampler_descriptor)?;

        let texture_descriptor = TextureDescriptor {
            size: Extent3D {
                width: 256,
                height: 256,
                depth: 1,
            },
            array_layer_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsageFlags::SAMPLED,
        };
        let texture = device.create_texture(texture_descriptor)?;
        let texture_view = texture.create_default_view()?;

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

        let bind_group_descriptor = BindGroupDescriptor {
            layout: bind_group_layout,
            bindings: &[
                BindGroupBinding {
                    binding: 0,
                    resource: BindingResource::Buffer(buffer, 0..buffer_descriptor.size),
                },
                BindGroupBinding {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
                BindGroupBinding {
                    binding: 2,
                    resource: BindingResource::TextureView(texture_view),
                },
            ],
        };
        let _bind_group = device.create_bind_group(bind_group_descriptor)?;

        Ok(instance)
    });
}
