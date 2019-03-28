use ash::version::DeviceV1_0;
use ash::vk;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{BindGroupLayoutInner, DeviceInner};
use crate::{BindGroupLayout, BindGroupLayoutDescriptor, BindingType, ShaderStageFlags};
use std::sync::Arc;

pub fn descriptor_type(binding_type: BindingType) -> vk::DescriptorType {
    match binding_type {
        BindingType::Sampler => vk::DescriptorType::SAMPLER,
        BindingType::DynamicStorageBuffer => vk::DescriptorType::STORAGE_BUFFER_DYNAMIC,
        BindingType::SampledTexture => vk::DescriptorType::SAMPLED_IMAGE,
        BindingType::DynamicUniformBuffer => vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
        BindingType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
        BindingType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
    }
}

pub fn shader_stage_flags(visibility: ShaderStageFlags) -> vk::ShaderStageFlags {
    let mut flags = vk::ShaderStageFlags::empty();
    if visibility.contains(ShaderStageFlags::VERTEX) {
        flags |= vk::ShaderStageFlags::VERTEX;
    }
    if visibility.contains(ShaderStageFlags::FRAGMENT) {
        flags |= vk::ShaderStageFlags::FRAGMENT;
    }
    if visibility.contains(ShaderStageFlags::COMPUTE) {
        flags |= vk::ShaderStageFlags::COMPUTE;
    }
    flags
}

impl BindGroupLayoutInner {
    pub fn new(
        device: Arc<DeviceInner>,
        descriptor: BindGroupLayoutDescriptor,
    ) -> Result<BindGroupLayoutInner, vk::Result> {
        let bindings: Vec<_> = descriptor
            .bindings
            .iter()
            .map(|binding| vk::DescriptorSetLayoutBinding {
                binding: binding.binding,
                descriptor_type: descriptor_type(binding.binding_type),
                stage_flags: shader_stage_flags(binding.visibility),
                ..Default::default()
            })
            .collect();

        let create_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);
        let handle = unsafe { device.raw.create_descriptor_set_layout(&create_info, None)? };

        Ok(BindGroupLayoutInner { handle, device })
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
