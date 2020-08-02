use vki::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
    BufferDescriptor, BufferUsage, BufferViewDescriptor, BufferViewFormat, Extent3d, SamplerDescriptor, ShaderStage,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsage,
};

pub mod support;

#[test]
fn create_bind_group_layout() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let bind_group_layout_descriptor = BindGroupLayoutDescriptor {
            entries: vec![BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStage::VERTEX,
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
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            size: 1024,
        };
        let buffer = device.create_buffer(buffer_descriptor)?;

        let texel_buffer_descriptor = BufferDescriptor {
            usage: BufferUsage::STORAGE,
            size: 1024,
        };
        let texel_buffer = device.create_buffer(texel_buffer_descriptor)?;
        let texel_buffer_view = texel_buffer.create_view(BufferViewDescriptor {
            format: BufferViewFormat::Texture(TextureFormat::R8G8B8A8Unorm),
            size: 1024,
            offset: 0,
        })?;

        let sampler_descriptor = SamplerDescriptor::default();
        let sampler = device.create_sampler(sampler_descriptor)?;

        let sampled_texture = device.create_texture(TextureDescriptor {
            size: Extent3d {
                width: 256,
                height: 256,
                depth: 1,
            },
            array_layer_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsage::SAMPLED,
        })?;
        let sampled_texture_view = sampled_texture.create_default_view()?;

        let readonly_storage_texture = device.create_texture(TextureDescriptor {
            size: Extent3d {
                width: 256,
                height: 256,
                depth: 1,
            },
            array_layer_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsage::STORAGE,
        })?;
        let readonly_storage_view = readonly_storage_texture.create_default_view()?;

        let writeonly_storage_texture = device.create_texture(TextureDescriptor {
            size: Extent3d {
                width: 256,
                height: 256,
                depth: 1,
            },
            array_layer_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsage::STORAGE,
        })?;
        let writeonly_storage_view = writeonly_storage_texture.create_default_view()?;

        let bind_group_layout_descriptor = BindGroupLayoutDescriptor {
            entries: vec![
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::VERTEX,
                    binding_type: BindingType::UniformBuffer,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStage::FRAGMENT,
                    binding_type: BindingType::Sampler,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStage::FRAGMENT,
                    binding_type: BindingType::SampledTexture,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStage::FRAGMENT,
                    binding_type: BindingType::StorageTexelBuffer,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStage::FRAGMENT,
                    binding_type: BindingType::ReadOnlyStorageTexture,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStage::FRAGMENT,
                    binding_type: BindingType::WriteOnlyStorageTexture,
                },
            ],
        };
        let bind_group_layout = device.create_bind_group_layout(bind_group_layout_descriptor)?;

        let bind_group_descriptor = BindGroupDescriptor {
            layout: bind_group_layout,
            entries: vec![
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(buffer, 0..buffer_descriptor.size),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(sampled_texture_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::BufferView(texel_buffer_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(readonly_storage_view),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::TextureView(writeonly_storage_view),
                },
            ],
        };
        let _bind_group = device.create_bind_group(bind_group_descriptor)?;

        Ok(instance)
    });
}
