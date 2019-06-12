use ash::version::DeviceV1_0;
use ash::vk;

use crate::error::Error;
use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{BindGroupInner, BindGroupLayoutInner, DeviceInner};
use crate::{
    BindGroup, BindGroupBinding, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutBinding,
    BindGroupLayoutDescriptor, BindingResource, BindingType, ShaderStageFlags,
};

use std::collections::HashMap;
use std::sync::Arc;

pub fn descriptor_type(binding_type: BindingType) -> vk::DescriptorType {
    match binding_type {
        BindingType::Sampler => vk::DescriptorType::SAMPLER,
        BindingType::DynamicStorageBuffer => vk::DescriptorType::STORAGE_BUFFER_DYNAMIC,
        BindingType::SampledTexture => vk::DescriptorType::SAMPLED_IMAGE,
        BindingType::DynamicUniformBuffer => vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
        BindingType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
        BindingType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
        BindingType::StorageTexelBuffer => vk::DescriptorType::STORAGE_TEXEL_BUFFER,
    }
}

pub fn shader_stage_flags(visibility: ShaderStageFlags) -> vk::ShaderStageFlags {
    let mut flags = vk::ShaderStageFlags::empty();
    if visibility.intersects(ShaderStageFlags::VERTEX) {
        flags |= vk::ShaderStageFlags::VERTEX;
    }
    if visibility.intersects(ShaderStageFlags::FRAGMENT) {
        flags |= vk::ShaderStageFlags::FRAGMENT;
    }
    if visibility.intersects(ShaderStageFlags::COMPUTE) {
        flags |= vk::ShaderStageFlags::COMPUTE;
    }
    flags
}

impl BindGroupLayoutInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: BindGroupLayoutDescriptor) -> Result<BindGroupLayoutInner, Error> {
        let bindings: Vec<_> = descriptor
            .bindings
            .iter()
            .map(|binding| vk::DescriptorSetLayoutBinding {
                binding: binding.binding,
                descriptor_type: descriptor_type(binding.binding_type),
                stage_flags: shader_stage_flags(binding.visibility),
                // TODO: Arrays?
                descriptor_count: 1,
                ..Default::default()
            })
            .collect();

        let create_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);

        let handle = unsafe { device.raw.create_descriptor_set_layout(&create_info, None)? };

        Ok(BindGroupLayoutInner {
            handle,
            device,
            layout_bindings: descriptor.bindings.to_vec(),
        })
    }
}

impl Drop for BindGroupLayoutInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}

impl Into<BindGroupLayout> for BindGroupLayoutInner {
    fn into(self) -> BindGroupLayout {
        BindGroupLayout { inner: Arc::new(self) }
    }
}

/// Finds the corresponding `BindGroupLayoutBinding`. The `bind_group_binding_descriptor_index` identifies
/// the index in `BindGroupDescriptor::bindings`. If the corresponding binding is found at this index
/// in `layout_bindings`, it's returned. Otherwise, a linear search is performed.
pub fn find_layout_binding(
    bind_group_binding_descriptor_index: usize,
    bind_group_binding: u32,
    layout_bindings: &[BindGroupLayoutBinding],
) -> Option<&BindGroupLayoutBinding> {
    // fast path
    let layout_binding = layout_bindings
        .get(bind_group_binding_descriptor_index)
        .filter(|layout| layout.binding == bind_group_binding);

    layout_binding.or_else(|| {
        layout_bindings
            .iter()
            .find(|layout| layout.binding == bind_group_binding)
    })
}

impl BindGroupInner {
    pub fn new(descriptor: &BindGroupDescriptor) -> Result<BindGroupInner, Error> {
        // TODO: DescriptorPool management. Dawn specifically calls out that this is inefficient

        let device = Arc::clone(&descriptor.layout.inner.device);

        let layout_bindings = &descriptor.layout.inner.layout_bindings;
        let mut pool_sizes = HashMap::with_capacity(layout_bindings.len());

        for layout_binding in layout_bindings.iter() {
            let descriptor_type = descriptor_type(layout_binding.binding_type);
            let mut pool_size: &mut vk::DescriptorPoolSize = pool_sizes.entry(descriptor_type).or_default();
            pool_size.ty = descriptor_type;
            pool_size.descriptor_count += 1;
        }

        let mut pool_sizes: Vec<vk::DescriptorPoolSize> = pool_sizes.values().cloned().collect();
        pool_sizes.sort_by(|a, b| a.ty.cmp(&b.ty));

        let create_info = vk::DescriptorPoolCreateInfo {
            max_sets: 1,
            p_pool_sizes: pool_sizes.as_ptr(),
            pool_size_count: pool_sizes.len() as u32,
            ..Default::default()
        };

        let mut bind_group = BindGroupInner {
            layout: descriptor.layout.inner.clone(),
            bindings: descriptor.bindings.to_vec(),
            descriptor_pool: vk::DescriptorPool::default(),
            handle: vk::DescriptorSet::default(),
        };

        unsafe {
            let result = device.raw.fp_v1_0().create_descriptor_pool(
                device.raw.handle(),
                &create_info,
                std::ptr::null(),
                &mut bind_group.descriptor_pool,
            );
            if result != vk::Result::SUCCESS {
                return Err(Error::from(result));
            }
        }

        let allocate_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: bind_group.descriptor_pool,
            descriptor_set_count: 1,
            p_set_layouts: &descriptor.layout.inner.handle,
            ..Default::default()
        };

        unsafe {
            let result = device.raw.fp_v1_0().allocate_descriptor_sets(
                device.raw.handle(),
                &allocate_info,
                &mut bind_group.handle,
            );
            if result != vk::Result::SUCCESS {
                return Err(Error::from(result));
            }
        }

        // TODO: Bind limits
        const MAX_BINDINGS_PER_GROUP: usize = 16;

        let mut writes = vec![vk::WriteDescriptorSet::default(); MAX_BINDINGS_PER_GROUP];
        let mut buffer_infos = vec![vk::DescriptorBufferInfo::default(); MAX_BINDINGS_PER_GROUP];
        let mut image_infos = vec![vk::DescriptorImageInfo::default(); MAX_BINDINGS_PER_GROUP];
        let mut texel_buffer_views = vec![vk::BufferView::null(); MAX_BINDINGS_PER_GROUP];

        let mut num_writes = 0;

        for (index, binding) in descriptor.bindings.iter().enumerate() {
            let layout_binding = find_layout_binding(index, binding.binding, &layout_bindings).ok_or_else(|| {
                let msg = format!(
                    "BindGroupLayout mismatch: BindGroupLayoutBinding not found (binding: {}, index: {})",
                    binding.binding, index
                );
                Error::from(msg)
            })?;

            let write = &mut writes[num_writes];
            write.dst_set = bind_group.handle;
            write.dst_binding = binding.binding;
            write.dst_array_element = 0;
            write.descriptor_count = 1;
            write.descriptor_type = descriptor_type(layout_binding.binding_type);

            match (&binding.resource, layout_binding.binding_type) {
                (&BindingResource::Buffer(ref buffer, ref range), BindingType::UniformBuffer)
                | (&BindingResource::Buffer(ref buffer, ref range), BindingType::DynamicUniformBuffer)
                | (&BindingResource::Buffer(ref buffer, ref range), BindingType::StorageBuffer)
                | (&BindingResource::Buffer(ref buffer, ref range), BindingType::DynamicStorageBuffer) => {
                    buffer_infos[num_writes].buffer = buffer.inner.handle;
                    buffer_infos[num_writes].offset = range.start as u64;
                    buffer_infos[num_writes].range = range.end as u64;
                    write.p_buffer_info = &buffer_infos[num_writes];
                }
                (&BindingResource::Sampler(ref sampler), BindingType::Sampler) => {
                    image_infos[num_writes].sampler = sampler.inner.handle;
                    write.p_image_info = &image_infos[num_writes];
                }
                (&BindingResource::TextureView(ref texture_view), BindingType::SampledTexture) => {
                    image_infos[num_writes].image_view = texture_view.inner.handle;
                    // TODO: Dawn notes that there could be two usages?
                    image_infos[num_writes].image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                    write.p_image_info = &image_infos[num_writes];
                }
                (&BindingResource::BufferView(ref buffer_view), BindingType::StorageTexelBuffer) => {
                    texel_buffer_views[num_writes] = buffer_view.inner.handle;
                    write.p_texel_buffer_view = &texel_buffer_views[num_writes];
                }
                _ => {
                    let resource_type = match binding.resource {
                        BindingResource::TextureView(_) => "TextureView",
                        BindingResource::Sampler(_) => "Sampler",
                        BindingResource::Buffer(_, _) => "Buffer",
                        BindingResource::BufferView(_) => "BufferView",
                    };
                    let msg = format!("BindingType is not valid for the BindingResource (binding: {}, index: {}): BindingType: {:?}, BindingResource: {:?}",
                          binding.binding, index, layout_binding.binding_type, resource_type);
                    return Err(Error::from(msg));
                }
            }

            num_writes += 1;
        }

        unsafe {
            device.raw.update_descriptor_sets(&writes[0..num_writes], &[]);
        }

        Ok(bind_group)
    }
}

impl Drop for BindGroupInner {
    fn drop(&mut self) {
        // TODO: VK_DESCRIPTOR_POOL_CREATE_FREE_DESCRIPTOR_SET_BIT
        // if self.descriptor_set != vk::DescriptorSet::default() {
        //     unsafe {
        //         self.layout
        //             .device
        //             .raw
        //             .free_descriptor_sets(self.descriptor_pool, &[self.descriptor_set]);
        //     }
        // }
        if self.descriptor_pool != vk::DescriptorPool::default() {
            let mut state = self.layout.device.state.lock();
            let serial = state.get_next_pending_serial();
            state
                .get_fenced_deleter()
                .delete_when_unused(self.descriptor_pool, serial);
        }
    }
}

impl Into<BindGroup> for BindGroupInner {
    fn into(self) -> BindGroup {
        BindGroup { inner: Arc::new(self) }
    }
}

impl BindGroup {
    pub fn bindings(&self) -> &[BindGroupBinding] {
        &self.inner.bindings
    }
}
