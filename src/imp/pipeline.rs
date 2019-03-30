use ash::version::DeviceV1_0;
use ash::vk;

use std::ffi::CString;
use std::sync::Arc;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{ComputePipelineInner, DeviceInner, PipelineLayoutInner};
use crate::{ComputePipeline, ComputePipelineDescriptor, PipelineLayout, PipelineLayoutDescriptor};

pub const MAX_PUSH_CONSTANTS_SIZE: u32 = 128;

impl PipelineLayoutInner {
    pub fn new(
        device: Arc<DeviceInner>,
        descriptor: PipelineLayoutDescriptor,
    ) -> Result<PipelineLayoutInner, vk::Result> {
        let push_constant_range = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::ALL,
            offset: 0,
            size: MAX_PUSH_CONSTANTS_SIZE,
        };

        let descriptor_set_layouts: Vec<_> = descriptor
            .bind_group_layouts
            .iter()
            .map(|bind_group_layout| bind_group_layout.inner.handle)
            .collect();

        let create_info = vk::PipelineLayoutCreateInfo::builder()
            .push_constant_ranges(&[push_constant_range])
            .set_layouts(&descriptor_set_layouts)
            .build();

        let handle = unsafe { device.raw.create_pipeline_layout(&create_info, None)? };

        Ok(PipelineLayoutInner { handle, device })
    }
}

impl Into<PipelineLayout> for PipelineLayoutInner {
    fn into(self) -> PipelineLayout {
        PipelineLayout { inner: Arc::new(self) }
    }
}

impl Drop for PipelineLayoutInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}

impl ComputePipelineInner {
    pub fn new(
        device: Arc<DeviceInner>,
        descriptor: ComputePipelineDescriptor,
    ) -> Result<ComputePipelineInner, vk::Result> {
        // TODO: inspect push constants

        let entry_point = CString::new(descriptor.compute_stage.entry_point).map_err(|e| {
            log::error!("invalid entry point: {:?}", e);
            vk::Result::ERROR_VALIDATION_FAILED_EXT
        })?;

        let create_info = vk::ComputePipelineCreateInfo {
            layout: descriptor.layout.inner.handle,
            base_pipeline_handle: vk::Pipeline::null(),
            base_pipeline_index: -1,
            stage: vk::PipelineShaderStageCreateInfo::builder()
                .name(entry_point.as_c_str())
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(descriptor.compute_stage.module.inner.handle)
                .build(),
            ..Default::default()
        };

        let pipeline_cache = vk::PipelineCache::null();
        let mut handle = vk::Pipeline::null();

        unsafe {
            device.raw.fp_v1_0().create_compute_pipelines(
                device.raw.handle(),
                pipeline_cache,
                1,
                &create_info,
                std::ptr::null(),
                &mut handle,
            )
        };

        Ok(ComputePipelineInner { handle, device })
    }
}

impl Into<ComputePipeline> for ComputePipelineInner {
    fn into(self) -> ComputePipeline {
        ComputePipeline { inner: Arc::new(self) }
    }
}

impl Drop for ComputePipelineInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}
