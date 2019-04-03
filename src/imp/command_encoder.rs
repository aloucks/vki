use ash::vk;

use crate::{
    BindGroup, BindingType, Buffer, BufferUsageFlags, CommandBuffer, CommandEncoder, ComputePassEncoder,
    RenderPassDescriptor, RenderPassEncoder, TextureUsageFlags,
};

use std::sync::Arc;

use crate::imp::command::{BufferCopy, Command};
use crate::imp::command_buffer::CommandBufferState;
use crate::imp::pass_resource_usage::{CommandBufferResourceUsage, PassResourceUsageTracker};
use crate::imp::{CommandBufferInner, CommandEncoderInner, DeviceInner};

use crate::error::EncoderError;

//struct RenderPassColorAttachmentInfo {
//    view: Arc<TextureViewInner>,
//    resolve_target: Arc<TextureViewInner>,
//    load_op: LoadOp,
//    store_op: StoreOp,
//    clear_color: Color,
//}
//
//struct RenderPassDepthStencilAttachmentInfo {
//    view: Arc<TextureViewInner>,
//    depth_load_op: LoadOp,
//    depth_store_op: StoreOp,
//    stencil_load_op: LoadOp,
//    stencil_store_op: StoreOp,
//    clear_depth: f32,
//    clear_stencil: u32,
//}

pub struct CommandEncoderState {
    pub commands: Vec<Command>,
    pub resource_usages: CommandBufferResourceUsage,
}

impl CommandEncoderState {
    pub fn new() -> CommandEncoderState {
        let commands = Vec::new();
        let resource_usages = CommandBufferResourceUsage::default();
        CommandEncoderState {
            commands,
            resource_usages,
        }
    }

    fn push(&mut self, command: Command) {
        self.commands.push(command);
    }
}

impl CommandEncoderInner {
    pub fn new(device: Arc<DeviceInner>) -> Result<CommandEncoderInner, vk::Result> {
        let state = CommandEncoderState::new();
        Ok(CommandEncoderInner { device, state })
    }

    fn push(&mut self, command: Command) {
        self.state.push(command)
    }

    fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u32]>) {
        let dynamic_offsets = dynamic_offsets.map(|v| v.to_vec());
        self.push(Command::SetBindGroup {
            index,
            dynamic_offsets,
            bind_group: bind_group.inner.clone(),
        });
    }
}

impl Into<CommandBufferState> for CommandEncoderState {
    fn into(self) -> CommandBufferState {
        CommandBufferState {
            commands: self.commands,
            resource_usages: self.resource_usages,
        }
    }
}

impl Into<CommandEncoder> for CommandEncoderInner {
    fn into(self) -> CommandEncoder {
        CommandEncoder { inner: self }
    }
}

impl CommandEncoder {
    fn push(&mut self, command: Command) {
        self.inner.push(command)
    }

    pub fn begin_compute_pass<'a>(&'a mut self) -> ComputePassEncoder<'a> {
        self.push(Command::BeginComputePass);
        ComputePassEncoder {
            top_level_encoder: &mut self.inner,
        }
    }

    pub fn begin_render_pass<'a>(&'a mut self, descriptor: RenderPassDescriptor) -> RenderPassEncoder<'a> {
        self.inner.state.push(Command::BeginRenderPass {
            color_attachments: descriptor.color_attachments.to_vec(),
            depth_stencil_attachment: descriptor.depth_stencil_attachment.cloned(),
            // TODO: Set and validate the sample_count, width, and height taking base mipmap
            //       level into consideration.
            sample_count: 1,
            width: 0,
            height: 0,
        });
        RenderPassEncoder {
            top_level_encoder: &mut self.inner,
        }
    }

    pub fn copy_buffer_to_buffer(
        &mut self,
        src: Buffer,
        src_offset: u64,
        dst: Buffer,
        dst_offset: u64,
        size_bytes: u64,
    ) {
        // TODO: Fix inconsistency with offset and size types (u64 vs u32 vs usize)
        self.inner.push(Command::CopyBufferToBuffer {
            source: BufferCopy {
                buffer: src.inner.clone(),
                offset_bytes: src_offset as u32,
                image_height_texels: 0,
                row_pitch_bytes: 0,
            },
            destination: BufferCopy {
                buffer: dst.inner.clone(),
                offset_bytes: dst_offset as u32,
                image_height_texels: 0,
                row_pitch_bytes: 0,
            },
            size_bytes: size_bytes as u32,
        });
        let top_level_buffers = &mut self.inner.state.resource_usages.top_level_buffers;
        top_level_buffers.insert(src.inner.clone());
        top_level_buffers.insert(dst.inner.clone());
    }

    fn validate_render_pass(&mut self, mut command_index: usize) -> Result<usize, EncoderError> {
        let begin_render_pass_command_index = command_index;
        let mut usage_tracker = PassResourceUsageTracker::default();
        //let mut state_tracker = .. ;
        while let Some(command) = self.inner.state.commands.get(command_index) {
            match command {
                Command::BeginRenderPass {
                    color_attachments,
                    depth_stencil_attachment,
                    ..
                } => {
                    debug_assert_eq!(begin_render_pass_command_index, command_index);
                    for info in color_attachments.iter() {
                        let texture = Arc::clone(&info.attachment.inner.texture);
                        usage_tracker.texture_used_as(texture, TextureUsageFlags::OUTPUT_ATTACHMENT);

                        if let Some(ref resolve_target) = info.resolve_target {
                            let texture = Arc::clone(&resolve_target.inner.texture);
                            usage_tracker.texture_used_as(texture, TextureUsageFlags::OUTPUT_ATTACHMENT);
                        }
                    }

                    if let Some(ref info) = depth_stencil_attachment {
                        let texture = Arc::clone(&info.attachment.inner.texture);
                        usage_tracker.texture_used_as(texture, TextureUsageFlags::OUTPUT_ATTACHMENT);
                    }
                }
                Command::SetRenderPipeline { .. } => {
                    // state.set_render_pipeline
                }
                Command::SetBindGroup { bind_group, .. } => {
                    // state.set_bind_group
                    for (index, layout_binding) in bind_group.layout.bindings.iter().enumerate() {
                        match layout_binding.binding_type {
                            BindingType::UniformBuffer => {
                                let (buffer, _) = bind_group.bindings[index]
                                    .resource
                                    .as_buffer()
                                    .expect("BindingResource::Buffer");
                                usage_tracker.buffer_used_as(buffer.inner.clone(), BufferUsageFlags::UNIFORM);
                            }
                            BindingType::StorageBuffer => {
                                let (buffer, _) = bind_group.bindings[index]
                                    .resource
                                    .as_buffer()
                                    .expect("BindingResource::Buffer");
                                usage_tracker.buffer_used_as(buffer.inner.clone(), BufferUsageFlags::STORAGE);
                            }
                            BindingType::SampledTexture => {
                                let texture_view = bind_group.bindings[index]
                                    .resource
                                    .as_texture_view()
                                    .expect("BindingResource::TextureView");
                                usage_tracker
                                    .texture_used_as(texture_view.inner.texture.clone(), TextureUsageFlags::SAMPLED);
                            }
                            _ => {}
                        }
                    }
                }
                Command::SetIndexBuffer { buffer, .. } => {
                    // state.set_index_buffer
                    usage_tracker.buffer_used_as(Arc::clone(buffer), BufferUsageFlags::INDEX);
                }
                Command::SetVertexBuffers { buffers, .. } => {
                    // state.set_vertex_buffers
                    for buffer in buffers.iter() {
                        usage_tracker.buffer_used_as(Arc::clone(buffer), BufferUsageFlags::VERTEX);
                    }
                }
                Command::EndRenderPass => {
                    self.inner
                        .state
                        .resource_usages
                        .per_pass
                        .push(usage_tracker.acquire_resource_usage());
                    return Ok(command_index);
                }
                _ => {}
            }
            command_index += 1;
        }
        unreachable!()
    }

    fn validate_compute_pass(&mut self, mut command_index: usize) -> Result<usize, EncoderError> {
        let mut usage_tracker = PassResourceUsageTracker::default();
        let begin_compute_pass_command_index = command_index;
        while let Some(command) = self.inner.state.commands.get(command_index) {
            match command {
                Command::BeginComputePass => {
                    debug_assert_eq!(begin_compute_pass_command_index, command_index);
                }
                Command::EndComputePass => {
                    self.inner
                        .state
                        .resource_usages
                        .per_pass
                        .push(usage_tracker.acquire_resource_usage());
                    return Ok(command_index);
                }
                _ => {}
            }
            command_index += 1;
        }
        unreachable!()
    }

    pub fn finish(mut self) -> Result<CommandBuffer, EncoderError> {
        let mut command_index = 0;
        while let Some(command) = self.inner.state.commands.get(command_index) {
            match command {
                Command::BeginComputePass => {
                    self.validate_compute_pass(command_index)?;
                }
                Command::BeginRenderPass { .. } => {
                    self.validate_render_pass(command_index)?;
                }
                _ => {}
            }
            command_index += 1;
        }
        let command_buffer = CommandBufferInner {
            state: self.inner.state.into(),
            device: self.inner.device,
        };
        Ok(CommandBuffer { inner: command_buffer })
    }
}

impl<'a> Drop for ComputePassEncoder<'a> {
    fn drop(&mut self) {
        self.top_level_encoder.push(Command::EndComputePass)
    }
}

impl<'a> ComputePassEncoder<'a> {
    pub fn end_pass(self) {
        /* drop */
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u32]>) {
        self.top_level_encoder
            .set_bind_group(index, bind_group, dynamic_offsets);
    }

    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.top_level_encoder.push(Command::Dispatch { x, y, z });
    }
}

impl<'a> Drop for RenderPassEncoder<'a> {
    fn drop(&mut self) {
        self.top_level_encoder.push(Command::EndRenderPass)
    }
}

impl<'a> RenderPassEncoder<'a> {
    pub fn end_pass(self) {
        /* drop */
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u32]>) {
        self.top_level_encoder
            .set_bind_group(index, bind_group, dynamic_offsets);
    }
}
