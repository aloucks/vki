use ash::version::DeviceV1_0;
use ash::vk;

use crate::error::Error;
use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{DeviceInner, SamplerInner};
use crate::{AddressMode, CompareFunction, FilterMode, Sampler, SamplerDescriptor};

use std::sync::Arc;

pub fn address_mode(mode: AddressMode) -> vk::SamplerAddressMode {
    match mode {
        AddressMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
        AddressMode::MirrorRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
        AddressMode::Repeat => vk::SamplerAddressMode::REPEAT,
    }
}

pub fn filter_mode(mode: FilterMode) -> vk::Filter {
    match mode {
        FilterMode::Nearest => vk::Filter::NEAREST,
        FilterMode::Linear => vk::Filter::LINEAR,
    }
}

pub fn mipmap_mode(mode: FilterMode) -> vk::SamplerMipmapMode {
    match mode {
        FilterMode::Linear => vk::SamplerMipmapMode::LINEAR,
        FilterMode::Nearest => vk::SamplerMipmapMode::NEAREST,
    }
}

pub fn compare_op(func: CompareFunction) -> vk::CompareOp {
    match func {
        CompareFunction::Less => vk::CompareOp::LESS,
        CompareFunction::LessEqual => vk::CompareOp::LESS_OR_EQUAL,
        CompareFunction::Never => vk::CompareOp::NEVER,
        CompareFunction::Always => vk::CompareOp::ALWAYS,
        CompareFunction::Greater => vk::CompareOp::GREATER,
        CompareFunction::GreaterEqual => vk::CompareOp::GREATER_OR_EQUAL,
        CompareFunction::Equal => vk::CompareOp::EQUAL,
        CompareFunction::NotEqual => vk::CompareOp::NOT_EQUAL,
    }
}

impl SamplerInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: SamplerDescriptor) -> Result<SamplerInner, Error> {
        let create_info = vk::SamplerCreateInfo {
            address_mode_u: address_mode(descriptor.address_mode_u),
            address_mode_v: address_mode(descriptor.address_mode_v),
            address_mode_w: address_mode(descriptor.address_mode_w),
            mag_filter: filter_mode(descriptor.mag_filter),
            min_filter: filter_mode(descriptor.min_filter),
            mipmap_mode: mipmap_mode(descriptor.mipmap_filter),
            mip_lod_bias: 0.0,
            // TODO: Inspect device features/properties to set anisotropy values
            anisotropy_enable: vk::FALSE,
            max_anisotropy: 1.0,
            compare_op: compare_op(descriptor.compare_function),
            compare_enable: if descriptor.compare_function == CompareFunction::Never {
                vk::FALSE
            } else {
                vk::TRUE
            },
            min_lod: descriptor.lod_min_clamp,
            max_lod: descriptor.lod_max_clamp,
            border_color: vk::BorderColor::FLOAT_TRANSPARENT_BLACK,
            unnormalized_coordinates: vk::FALSE,
            ..Default::default()
        };

        let handle = unsafe { device.raw.create_sampler(&create_info, None)? };

        Ok(SamplerInner {
            handle,
            device,
            descriptor,
        })
    }
}

impl Drop for SamplerInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}

impl Into<Sampler> for SamplerInner {
    fn into(self) -> Sampler {
        Sampler { inner: Arc::new(self) }
    }
}
