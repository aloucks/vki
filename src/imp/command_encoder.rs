use ash::vk;

use crate::{
    BindGroup, BindingType, Buffer, BufferCopyView, BufferUsageFlags, Color, CommandBuffer, CommandEncoder,
    ComputePassEncoder, ComputePipeline, Extent3D, LoadOp, RenderPassColorAttachmentDescriptor,
    RenderPassDepthStencilAttachmentDescriptor, RenderPassDescriptor, RenderPassEncoder, RenderPipeline, StoreOp,
    TextureCopyView, TextureUsageFlags,
};

use std::sync::Arc;

use crate::imp::command::{BufferCopy, Command, TextureCopy};
use crate::imp::command_buffer::CommandBufferState;
use crate::imp::pass_resource_usage::{CommandBufferResourceUsage, PassResourceUsageTracker};
use crate::imp::{
    CommandBufferInner, CommandEncoderInner, ComputePassEncoderInner, DeviceInner, RenderPassEncoderInner,
    TextureViewInner,
};

use crate::error::EncoderError;

#[derive(Debug, Clone)]
pub struct RenderPassColorAttachmentInfo {
    pub view: Arc<TextureViewInner>,
    pub resolve_target: Option<Arc<TextureViewInner>>,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub clear_color: Color,
}

impl<'a> From<&RenderPassColorAttachmentDescriptor<'a>> for RenderPassColorAttachmentInfo {
    fn from(descriptor: &RenderPassColorAttachmentDescriptor<'a>) -> RenderPassColorAttachmentInfo {
        RenderPassColorAttachmentInfo {
            view: Arc::clone(&descriptor.attachment.inner),
            resolve_target: descriptor.resolve_target.map(|v| Arc::clone(&v.inner)),
            load_op: descriptor.load_op,
            store_op: descriptor.store_op,
            clear_color: descriptor.clear_color,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderPassDepthStencilAttachmentInfo {
    pub view: Arc<TextureViewInner>,
    pub depth_load_op: LoadOp,
    pub depth_store_op: StoreOp,
    pub clear_depth: f32,
    pub stencil_load_op: LoadOp,
    pub stencil_store_op: StoreOp,
    pub clear_stencil: u32,
}

impl<'a> From<RenderPassDepthStencilAttachmentDescriptor<'a>> for RenderPassDepthStencilAttachmentInfo {
    fn from(descriptor: RenderPassDepthStencilAttachmentDescriptor<'a>) -> RenderPassDepthStencilAttachmentInfo {
        RenderPassDepthStencilAttachmentInfo {
            view: Arc::clone(&descriptor.attachment.inner),
            depth_load_op: descriptor.depth_load_op,
            depth_store_op: descriptor.depth_store_op,
            clear_depth: descriptor.clear_depth,
            stencil_load_op: descriptor.stencil_load_op,
            stencil_store_op: descriptor.stencil_store_op,
            clear_stencil: descriptor.clear_stencil,
        }
    }
}

#[derive(Debug)]
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

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &BindGroup,
        dynamic_offsets: Option<&[u32]>,
        usage_tracker: &mut PassResourceUsageTracker,
    ) {
        for (index, layout_binding) in bind_group.inner.layout.bindings.iter().enumerate() {
            match layout_binding.binding_type {
                BindingType::UniformBuffer => {
                    let (buffer, _) = bind_group.inner.bindings[index]
                        .resource
                        .as_buffer()
                        .expect("BindingType::UniformBuffer => BindingResource::Buffer");
                    usage_tracker.buffer_used_as(buffer.inner.clone(), BufferUsageFlags::UNIFORM);
                }
                BindingType::StorageBuffer => {
                    let (buffer, _) = bind_group.inner.bindings[index]
                        .resource
                        .as_buffer()
                        .expect("BindingType::StorageBuffer => BindingResource::Buffer");
                    usage_tracker.buffer_used_as(buffer.inner.clone(), BufferUsageFlags::STORAGE);
                }
                BindingType::SampledTexture => {
                    let texture_view = bind_group.inner.bindings[index]
                        .resource
                        .as_texture_view()
                        .expect("BindingType::SampledTexture => BindingResource::TextureView");
                    usage_tracker.texture_used_as(texture_view.inner.texture.clone(), TextureUsageFlags::SAMPLED);
                }
                _ => {}
            }
        }

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
    pub fn begin_render_pass<'a>(&'a mut self, descriptor: RenderPassDescriptor) -> RenderPassEncoder<'a> {
        RenderPassEncoder::begin_render_pass(&mut self.inner, descriptor)
    }

    pub fn begin_compute_pass<'a>(&'a mut self) -> ComputePassEncoder<'a> {
        ComputePassEncoder::begin_compute_pass(&mut self.inner)
    }

    pub fn copy_buffer_to_buffer(
        &mut self,
        src: Buffer,
        src_offset: usize,
        dst: Buffer,
        dst_offset: usize,
        size_bytes: usize,
    ) {
        // TODO: Fix inconsistency with offset and size types (u64 vs u32 vs usize)
        self.inner.push(Command::CopyBufferToBuffer {
            src: BufferCopy {
                buffer: src.inner.clone(),
                offset: src_offset,
                image_height: 0,
                row_pitch: 0,
            },
            dst: BufferCopy {
                buffer: dst.inner.clone(),
                offset: dst_offset,
                image_height: 0,
                row_pitch: 0,
            },
            size_bytes,
        });

        let top_level_buffers = &mut self.inner.state.resource_usages.top_level_buffers;

        top_level_buffers.insert(src.inner.clone());
        top_level_buffers.insert(dst.inner.clone());
    }

    // TODO: row_pitch bytes vs texels
    pub fn copy_buffer_to_texture(&mut self, src: BufferCopyView, dst: TextureCopyView, copy_size: Extent3D) {
        self.inner.push(Command::CopyBufferToTexture {
            src: BufferCopy {
                buffer: Arc::clone(&src.buffer.inner),
                row_pitch: src.row_pitch,
                image_height: src.image_height,
                offset: src.offset,
            },
            dst: TextureCopy {
                texture: Arc::clone(&dst.texture.inner),
                mip_level: dst.mip_level,
                origin_texels: dst.origin,
                array_layer: dst.array_layer, // TODO: slice ?
            },
            size_texels: copy_size,
        });

        let top_level_buffers = &mut self.inner.state.resource_usages.top_level_buffers;
        let top_level_textures = &mut self.inner.state.resource_usages.top_level_textures;

        top_level_buffers.insert(src.buffer.inner.clone());
        top_level_textures.insert(dst.texture.inner.clone());
    }

    // TODO: row_pitch bytes vs texels
    pub fn copy_texture_to_texture(&mut self, src: TextureCopyView, dst: TextureCopyView, copy_size: Extent3D) {
        self.inner.push(Command::CopyTextureToTexture {
            src: TextureCopy {
                texture: Arc::clone(&src.texture.inner),
                mip_level: src.mip_level,
                origin_texels: src.origin,
                array_layer: src.array_layer, // TODO: slice ?
            },
            dst: TextureCopy {
                texture: Arc::clone(&dst.texture.inner),
                mip_level: dst.mip_level,
                origin_texels: dst.origin,
                array_layer: dst.array_layer, // TODO: slice ?
            },
            size_texels: copy_size,
        });

        let top_level_textures = &mut self.inner.state.resource_usages.top_level_textures;

        top_level_textures.insert(src.texture.inner.clone());
        top_level_textures.insert(dst.texture.inner.clone());
    }

    pub fn finish(self) -> Result<CommandBuffer, EncoderError> {
        let mut command_index = 0;
        while let Some(command) = self.inner.state.commands.get(command_index) {
            match command {
                Command::BeginComputePass => {
                    command_index = validate_compute_pass(&self, command_index)?;
                }
                Command::BeginRenderPass { .. } => {
                    command_index = validate_render_pass(&self, command_index)?;
                }
                _ => {}
            }
            command_index += 1;
        }
        debug_assert_eq!(command_index, self.inner.state.commands.len());
        let command_buffer = CommandBufferInner {
            state: self.inner.state.into(),
            device: self.inner.device,
        };
        Ok(CommandBuffer { inner: command_buffer })
    }
}

// TODO: validate_render_pass
fn validate_render_pass(encoder: &CommandEncoder, mut command_index: usize) -> Result<usize, EncoderError> {
    let begin_render_pass_command_index = command_index;

    //let mut state_tracker = .. ;
    while let Some(command) = encoder.inner.state.commands.get(command_index) {
        match command {
            Command::BeginRenderPass { .. } => {
                debug_assert_eq!(begin_render_pass_command_index, command_index);
            }
            Command::SetRenderPipeline { .. } => {}
            Command::SetBindGroup { .. } => {}
            Command::SetIndexBuffer { .. } => {}
            Command::SetVertexBuffers { .. } => {}
            Command::EndRenderPass => {
                return Ok(command_index);
            }
            _ => {}
        }
        command_index += 1;
    }
    unreachable!()
}

// TODO: validate_compute_pass
fn validate_compute_pass(encoder: &CommandEncoder, mut command_index: usize) -> Result<usize, EncoderError> {
    let begin_compute_pass_command_index = command_index;
    while let Some(command) = encoder.inner.state.commands.get(command_index) {
        match command {
            Command::BeginComputePass => {
                debug_assert_eq!(begin_compute_pass_command_index, command_index);
            }
            Command::EndComputePass => {
                return Ok(command_index);
            }
            _ => {}
        }
        command_index += 1;
    }
    unreachable!()
}

impl<'a> Drop for ComputePassEncoderInner<'a> {
    fn drop(&mut self) {
        let pass_resource_usage = self.usage_tracker.acquire_resource_usage();
        self.top_level_encoder
            .state
            .resource_usages
            .per_pass
            .push(pass_resource_usage);

        self.top_level_encoder.push(Command::EndComputePass);
    }
}

impl<'a> ComputePassEncoder<'a> {
    fn begin_compute_pass(top_level_encoder: &'a mut CommandEncoderInner) -> ComputePassEncoder<'a> {
        top_level_encoder.push(Command::BeginComputePass);

        ComputePassEncoder {
            inner: ComputePassEncoderInner {
                top_level_encoder,
                usage_tracker: PassResourceUsageTracker::default(),
            },
        }
    }

    pub fn end_pass(self) {
        /* drop */
    }

    pub fn set_pipeline(&mut self, pipeline: &ComputePipeline) {
        // state.set_render_pipeline
        self.inner.top_level_encoder.push(Command::SetComputePipeline {
            pipeline: Arc::clone(&pipeline.inner),
        })
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u32]>) {
        let usage_tracker = &mut self.inner.usage_tracker;
        self.inner
            .top_level_encoder
            .set_bind_group(index, bind_group, dynamic_offsets, usage_tracker);
    }

    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.inner.top_level_encoder.push(Command::Dispatch { x, y, z });
    }
}

impl<'a> Drop for RenderPassEncoderInner<'a> {
    fn drop(&mut self) {
        let pass_resource_usage = self.usage_tracker.acquire_resource_usage();
        self.top_level_encoder
            .state
            .resource_usages
            .per_pass
            .push(pass_resource_usage);

        self.top_level_encoder.push(Command::EndRenderPass);
    }
}

impl<'a> RenderPassEncoder<'a> {
    fn begin_render_pass(
        top_level_encoder: &'a mut CommandEncoderInner,
        descriptor: RenderPassDescriptor,
    ) -> RenderPassEncoder<'a> {
        let mut usage_tracker = PassResourceUsageTracker::default();

        for info in descriptor.color_attachments.iter() {
            let texture = Arc::clone(&info.attachment.inner.texture);
            usage_tracker.texture_used_as(texture, TextureUsageFlags::OUTPUT_ATTACHMENT);

            if let Some(ref resolve_target) = info.resolve_target {
                let texture = Arc::clone(&resolve_target.inner.texture);
                usage_tracker.texture_used_as(texture, TextureUsageFlags::OUTPUT_ATTACHMENT);
            }
        }

        if let Some(ref info) = descriptor.depth_stencil_attachment {
            let texture = Arc::clone(&info.attachment.inner.texture);
            usage_tracker.texture_used_as(texture, TextureUsageFlags::OUTPUT_ATTACHMENT);
        }

        // The framebuffer needs to be created with the smallest image size. In general, these should
        // always be the same. The exception is during window resize. If the window event processing
        // and rendering are on different threads, the width and height of the target and swapchain
        // images (that are used as resolve textures) could mismatch.

        let mut sample_count = 1;
        let mut width = 1;
        let mut height = 1;

        for descriptor in descriptor.color_attachments.iter() {
            let (a_sample_count, size) = descriptor.attachment.inner.get_sample_count_and_mipmap_size();
            sample_count = sample_count.max(a_sample_count);
            width = if width == 1 { size.width } else { width.min(size.width) };
            height = if height == 1 {
                size.height
            } else {
                height.min(size.height)
            };
        }

        if let Some(ref a) = descriptor.depth_stencil_attachment {
            let (a_sample_count, size) = a.attachment.inner.get_sample_count_and_mipmap_size();
            sample_count = sample_count.max(a_sample_count);
            width = width.min(size.width);
            height = height.min(size.height);
        }

        for descriptor in descriptor
            .color_attachments
            .iter()
            .filter_map(|a| a.resolve_target.as_ref())
        {
            let (a_sample_count, size) = descriptor.inner.get_sample_count_and_mipmap_size();
            sample_count = sample_count.max(a_sample_count);
            width = width.min(size.width);
            height = height.min(size.height);
        }

        width = width.max(1);
        height = height.max(1);

        log::trace!(
            "begin_render_pass; sample_count: {}, width: {}, height: {}",
            sample_count,
            width,
            height
        );

        top_level_encoder.push(Command::BeginRenderPass {
            color_attachments: descriptor.color_attachments.iter().map(Into::into).collect(),
            depth_stencil_attachment: descriptor.depth_stencil_attachment.map(Into::into),
            sample_count,
            width,
            height,
        });

        RenderPassEncoder {
            inner: RenderPassEncoderInner {
                top_level_encoder,
                usage_tracker,
            },
        }
    }

    pub fn end_pass(self) {
        /* drop */
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u32]>) {
        let usage_tracker = &mut self.inner.usage_tracker;
        self.inner
            .top_level_encoder
            .set_bind_group(index, bind_group, dynamic_offsets, usage_tracker);
    }

    pub fn set_index_buffer(&mut self, buffer: &Buffer, offset: u32) {
        // TODO: If the pipeline isn't set first, this will fail in the recording phase
        // state.set_index_buffer
        self.inner
            .usage_tracker
            .buffer_used_as(Arc::clone(&buffer.inner), BufferUsageFlags::INDEX);

        self.inner.top_level_encoder.push(Command::SetIndexBuffer {
            buffer: Arc::clone(&buffer.inner),
            offset,
        });
    }

    /// Set the vertex buffers, starting at the `start_slot` binding index.
    ///
    /// ## Panics
    ///
    /// Panics if the length of `buffers` is not equal to the length of `offsets`.
    pub fn set_vertex_buffers(&mut self, start_slot: u32, buffers: &[Buffer], offsets: &[u64]) {
        // state.set_vertex_buffers

        assert_eq!(buffers.len(), offsets.len(), "buffers.len() != offsets.len()");

        let mut buffers_vec = Vec::with_capacity(buffers.len());

        for buffer in buffers.iter() {
            buffers_vec.push(Arc::clone(&buffer.inner));
            self.inner
                .usage_tracker
                .buffer_used_as(Arc::clone(&buffer.inner), BufferUsageFlags::VERTEX);
        }

        self.inner.top_level_encoder.push(Command::SetVertexBuffers {
            buffers: buffers_vec,
            start_slot,
            offsets: offsets.to_owned(),
        });
    }

    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) {
        // state.set_render_pipeline
        self.inner.top_level_encoder.push(Command::SetRenderPipeline {
            pipeline: Arc::clone(&pipeline.inner),
        })
    }

    pub fn set_blend_color(&mut self, color: Color) {
        self.inner.top_level_encoder.push(Command::SetBlendColor { color });
    }

    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.inner
            .top_level_encoder
            .push(Command::SetStencilReference { reference });
    }

    pub fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32, min_depth: f32, max_depth: f32) {
        self.inner.top_level_encoder.push(Command::SetViewport {
            x,
            y,
            width,
            height,
            min_depth,
            max_depth,
        })
    }

    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.inner
            .top_level_encoder
            .push(Command::SetScissorRect { x, y, width, height });
    }

    pub fn draw(&mut self, vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32) {
        self.inner.top_level_encoder.push(Command::Draw {
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        })
    }

    pub fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) {
        self.inner.top_level_encoder.push(Command::DrawIndexed {
            index_count,
            instance_count,
            first_index,
            base_vertex,
            first_instance,
        })
    }
}
