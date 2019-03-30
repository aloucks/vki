use ash::version::DeviceV1_0;
use ash::vk;

use std::sync::Arc;

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{DeviceInner, PipelineLayoutInner};
use crate::{PipelineLayout, PipelineLayoutDescriptor};

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

