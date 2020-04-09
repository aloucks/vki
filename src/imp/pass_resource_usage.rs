use crate::imp::{BufferInner, TextureInner};
use crate::{BufferUsage, Error, TextureUsage};

use ash::vk;
use std::sync::Arc;

use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone)]
pub struct PassResourceUsage {
    pub buffers: Vec<(Arc<BufferInner>, BufferUsage)>,
    pub textures: Vec<(Arc<TextureInner>, TextureUsage)>,
}

impl PassResourceUsage {
    pub fn transition_for_pass(&self, command_buffer: vk::CommandBuffer) -> Result<(), Error> {
        for (buffer, usage) in self.buffers.iter() {
            buffer.transition_usage_now(command_buffer, *usage)?;
        }
        for (texture, usage) in self.textures.iter() {
            texture.transition_usage_now(command_buffer, *usage, None)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct CommandBufferResourceUsage {
    pub per_pass: Vec<PassResourceUsage>,
    pub top_level_buffers: HashSet<Arc<BufferInner>>,
    pub top_level_textures: HashSet<Arc<TextureInner>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PassType {
    Render,
    Compute,
}

#[derive(Debug, Default)]
pub struct PassResourceUsageTracker {
    buffer_usages: HashMap<Arc<BufferInner>, BufferUsage>,
    texture_usages: HashMap<Arc<TextureInner>, TextureUsage>,
    storage_used_multiple_times: bool,
}

impl PassResourceUsageTracker {
    pub fn buffer_used_as(&mut self, buffer: Arc<BufferInner>, usage: BufferUsage) {
        let existing_usage = self.buffer_usages.entry(buffer.clone()).or_insert(BufferUsage::NONE);
        if usage == BufferUsage::STORAGE && existing_usage.intersects(BufferUsage::STORAGE) {
            self.storage_used_multiple_times = true;
        }
        existing_usage.insert(usage);
    }

    pub fn texture_used_as(&mut self, texture: Arc<TextureInner>, usage: TextureUsage) {
        let existing_usage = self.texture_usages.entry(texture.clone()).or_insert(TextureUsage::NONE);
        if usage == TextureUsage::STORAGE && existing_usage.intersects(TextureUsage::STORAGE) {
            self.storage_used_multiple_times = true;
        }
        existing_usage.insert(usage);
    }

    // TODO: PassResourceUsageTracker::validate_usages
    pub fn validate_usages(_pass_type: PassType) -> Result<(), ()> {
        Ok(())
    }

    //fn validate_render_pass(render_pass: )

    // TODO: Verify that this is draining operation. Dawn doesn't explicitly
    //      drain the buffer_usages or texture_usages, but the pass tracker
    //      object only lives for a single pass and it wouldn't make sense
    //      for more commands to record usage after the pass has ended.
    //      By draining here, we can prevent Arc::clone on all the buffers
    //      and textures.
    pub fn acquire_resource_usage(&mut self) -> PassResourceUsage {
        let mut result = PassResourceUsage::default();
        result.buffers.reserve(self.buffer_usages.len());
        result.textures.reserve(self.texture_usages.len());
        for (buffer, usage) in self.buffer_usages.drain() {
            result.buffers.push((buffer, usage));
        }
        for (texture, usage) in self.texture_usages.drain() {
            result.textures.push((texture, usage));
        }
        result
    }
}
