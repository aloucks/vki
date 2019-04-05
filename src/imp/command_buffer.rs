use ash::version::DeviceV1_0;
use ash::vk;

use crate::imp::command::{BufferCopy, Command, TextureCopy};
use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::pass_resource_usage::CommandBufferResourceUsage;
use crate::imp::render_pass::{ColorInfo, DepthStencilInfo, RenderPassCacheQuery};
use crate::imp::{render_pass, texture};
use crate::imp::{CommandBufferInner, RenderPipelineInner};
use crate::{
    BufferUsageFlags, Extent3D, IndexFormat, RenderPassColorAttachmentDescriptor,
    RenderPassDepthStencilAttachmentDescriptor, TextureUsageFlags,
};
use smallvec::SmallVec;
use std::sync::Arc;

pub const MAX_VERTEX_INPUTS: usize = 16;

#[derive(Debug)]
pub struct CommandBufferState {
    pub commands: Vec<Command>,
    pub resource_usages: CommandBufferResourceUsage,
}

fn index_type(format: IndexFormat) -> vk::IndexType {
    match format {
        IndexFormat::U16 => vk::IndexType::UINT16,
        IndexFormat::U32 => vk::IndexType::UINT32,
    }
}

fn buffer_image_copy(
    buffer_copy: &BufferCopy,
    texture_copy: &TextureCopy,
    size_texels: Extent3D,
) -> vk::BufferImageCopy {
    vk::BufferImageCopy {
        buffer_offset: buffer_copy.offset as _,
        buffer_row_length: buffer_copy.row_pitch, // TODO: row_pitch
        buffer_image_height: buffer_copy.image_height,
        image_subresource: vk::ImageSubresourceLayers {
            aspect_mask: texture::aspect_mask(texture_copy.texture.descriptor.format),
            mip_level: texture_copy.mip_level,
            base_array_layer: texture_copy.array_layer,
            layer_count: 1,
        },
        image_offset: vk::Offset3D {
            x: texture_copy.origin_texels.x,
            y: texture_copy.origin_texels.y,
            z: texture_copy.origin_texels.z,
        },
        image_extent: vk::Extent3D {
            width: size_texels.width,
            height: size_texels.height,
            depth: size_texels.depth,
        },
    }
}

fn image_copy(src: &TextureCopy, dst: &TextureCopy, size_texels: Extent3D) -> vk::ImageCopy {
    vk::ImageCopy {
        src_subresource: vk::ImageSubresourceLayers {
            aspect_mask: texture::aspect_mask(src.texture.descriptor.format),
            mip_level: src.mip_level,
            base_array_layer: src.array_layer,
            layer_count: 1,
        },
        src_offset: vk::Offset3D {
            x: src.origin_texels.x,
            y: src.origin_texels.y,
            z: src.origin_texels.z,
        },
        dst_subresource: vk::ImageSubresourceLayers {
            aspect_mask: texture::aspect_mask(dst.texture.descriptor.format),
            mip_level: dst.mip_level,
            base_array_layer: dst.array_layer,
            layer_count: 1,
        },
        dst_offset: vk::Offset3D {
            x: dst.origin_texels.x,
            y: dst.origin_texels.y,
            z: dst.origin_texels.z,
        },
        extent: vk::Extent3D {
            width: size_texels.width,
            height: size_texels.height,
            depth: size_texels.depth,
        },
    }
}

impl CommandBufferInner {
    pub fn record_commands(&mut self, command_buffer: vk::CommandBuffer) -> Result<(), vk::Result> {
        let mut pass = 0;
        let mut command_index = 0;
        while let Some(command) = self.state.commands.get(command_index) {
            match command {
                Command::CopyBufferToBuffer { src, dst, size_bytes } => {
                    src.buffer
                        .transition_usage_now(command_buffer, BufferUsageFlags::TRANSFER_SRC)?;
                    dst.buffer
                        .transition_usage_now(command_buffer, BufferUsageFlags::TRANSFER_DST)?;
                    let region = vk::BufferCopy {
                        size: *size_bytes as _,
                        src_offset: src.offset as _,
                        dst_offset: dst.offset as _,
                    };
                    unsafe {
                        self.device.raw.cmd_copy_buffer(
                            command_buffer,
                            src.buffer.handle,
                            dst.buffer.handle,
                            &[region],
                        );
                    }
                }
                Command::CopyBufferToTexture { src, dst, size_texels } => {
                    src.buffer
                        .transition_usage_now(command_buffer, BufferUsageFlags::TRANSFER_SRC)?;
                    dst.texture
                        .transition_usage_now(command_buffer, TextureUsageFlags::TRANSFER_DST)?;
                    let region = buffer_image_copy(src, dst, *size_texels);
                    unsafe {
                        self.device.raw.cmd_copy_buffer_to_image(
                            command_buffer,
                            src.buffer.handle,
                            dst.texture.handle,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            &[region],
                        );
                    }
                }
                Command::CopyTextureToBuffer { src, dst, size_texels } => {
                    src.texture
                        .transition_usage_now(command_buffer, TextureUsageFlags::TRANSFER_SRC)?;
                    dst.buffer
                        .transition_usage_now(command_buffer, BufferUsageFlags::TRANSFER_DST)?;
                    let region = buffer_image_copy(dst, src, *size_texels);
                    unsafe {
                        self.device.raw.cmd_copy_image_to_buffer(
                            command_buffer,
                            src.texture.handle,
                            vk::ImageLayout::GENERAL,
                            dst.buffer.handle,
                            &[region],
                        );
                    }
                }
                Command::CopyTextureToTexture { dst, src, size_texels } => {
                    src.texture
                        .transition_usage_now(command_buffer, TextureUsageFlags::TRANSFER_SRC)?;
                    dst.texture
                        .transition_usage_now(command_buffer, TextureUsageFlags::TRANSFER_DST)?;
                    let region = image_copy(src, dst, *size_texels);
                    unsafe {
                        self.device.raw.cmd_copy_image(
                            command_buffer,
                            src.texture.handle,
                            vk::ImageLayout::GENERAL,
                            dst.texture.handle,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            &[region],
                        );
                    }
                }
                Command::BeginRenderPass {
                    color_attachments,
                    depth_stencil_attachment,
                    width,
                    height,
                    sample_count,
                } => {
                    self.state.resource_usages.per_pass[pass].transition_for_pass(command_buffer)?;
                    command_index = self.record_render_pass(
                        command_buffer,
                        command_index,
                        color_attachments,
                        depth_stencil_attachment,
                        *width,
                        *height,
                        *sample_count,
                    )?;
                    pass += 1;
                }
                Command::BeginComputePass => {
                    self.state.resource_usages.per_pass[pass].transition_for_pass(command_buffer)?;
                    command_index = self.record_compute_pass(command_buffer, command_index)?;
                    pass += 1;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn record_render_pass_begin(
        &self,
        command_buffer: vk::CommandBuffer,
        color_attachments: &[RenderPassColorAttachmentDescriptor],
        depth_stencil_attachment: &Option<RenderPassDepthStencilAttachmentDescriptor>,
        width: u32,
        height: u32,
    ) -> Result<(), vk::Result> {
        let mut query = RenderPassCacheQuery::default();

        for color_attachment in color_attachments.iter() {
            query.add_color(ColorInfo {
                format: color_attachment.attachment.inner.texture.descriptor.format,
                load_op: color_attachment.load_op,
            });
        }

        if let Some(depth_stencil_attachment) = depth_stencil_attachment {
            query.set_depth_stencil(DepthStencilInfo {
                format: depth_stencil_attachment.attachment.inner.texture.descriptor.format,
                stencil_load_op: depth_stencil_attachment.stencil_load_op,
                depth_load_op: depth_stencil_attachment.depth_load_op,
            })
        }

        let mut clear_values = SmallVec::<[vk::ClearValue; render_pass::MAX_COLOR_ATTACHMENTS]>::new();
        let mut attachments = SmallVec::<[vk::ImageView; render_pass::MAX_COLOR_ATTACHMENTS + 1]>::new();

        for color_attachment in color_attachments.iter() {
            clear_values.push(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        color_attachment.clear_color.r,
                        color_attachment.clear_color.g,
                        color_attachment.clear_color.b,
                        color_attachment.clear_color.a,
                    ],
                },
            });
            attachments.push(color_attachment.attachment.inner.handle);
        }

        if let Some(depth_stencil_attachment) = depth_stencil_attachment {
            clear_values.push(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: depth_stencil_attachment.clear_depth,
                    stencil: depth_stencil_attachment.clear_stencil,
                },
            });
            attachments.push(depth_stencil_attachment.attachment.inner.handle);
        }

        let mut state = self.device.state.lock();

        let render_pass = state.get_render_pass(query, &self.device)?;
        let create_info = vk::FramebufferCreateInfo {
            render_pass,
            p_attachments: attachments.as_ptr(),
            attachment_count: attachments.len() as u32,
            width,
            height,
            layers: 1,
            ..Default::default()
        };
        let framebuffer = unsafe { self.device.raw.create_framebuffer(&create_info, None)? };
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(framebuffer, serial);

        drop(state);

        let begin_info = vk::RenderPassBeginInfo {
            render_pass,
            framebuffer,
            render_area: vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width, height },
            },
            clear_value_count: clear_values.len() as u32,
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };

        unsafe {
            self.device
                .raw
                .cmd_begin_render_pass(command_buffer, &begin_info, vk::SubpassContents::INLINE);
        }

        Ok(())
    }

    fn record_render_pass_dynamic_state_defaults(&self, command_buffer: vk::CommandBuffer, width: u32, height: u32) {
        unsafe {
            self.device.raw.cmd_set_line_width(command_buffer, 1.0);
            self.device.raw.cmd_set_depth_bounds(command_buffer, 0.0, 1.0);
            self.device
                .raw
                .cmd_set_stencil_reference(command_buffer, vk::StencilFaceFlags::STENCIL_FRONT_AND_BACK, 0);
            self.device
                .raw
                .cmd_set_blend_constants(command_buffer, &[0.0, 0.0, 0.0, 0.0]);
            self.device.raw.cmd_set_viewport(
                command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.raw.cmd_set_scissor(
                command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D { width, height },
                }],
            );
        }
    }

    fn record_render_pass(
        &self,
        command_buffer: vk::CommandBuffer,
        mut command_index: usize,
        color_attachments: &[RenderPassColorAttachmentDescriptor],
        depth_stencil_attachment: &Option<RenderPassDepthStencilAttachmentDescriptor>,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> Result<usize, vk::Result> {
        self.record_render_pass_begin(
            command_buffer,
            color_attachments,
            depth_stencil_attachment,
            width,
            height,
        )?;

        self.record_render_pass_dynamic_state_defaults(command_buffer, width, height);

        let mut last_pipeline: Option<&Arc<RenderPipelineInner>> = None;

        while let Some(command) = self.state.commands.get(command_index) {
            match command {
                Command::EndRenderPass => {
                    return Ok(command_index);
                }
                Command::Draw {
                    vertex_count,
                    instance_count,
                    first_vertex,
                    first_instance,
                } => {
                    // TODO: flush descriptor sets
                    unsafe {
                        self.device.raw.cmd_draw(
                            command_buffer,
                            *vertex_count,
                            *instance_count,
                            *first_vertex,
                            *first_instance,
                        );
                    }
                }
                Command::DrawIndexed {
                    index_count,
                    instance_count,
                    first_index,
                    base_vertex,
                    first_instance,
                } => {
                    // TODO: flush descriptor sets
                    unsafe {
                        let base_vertex = *base_vertex as i32;
                        self.device.raw.cmd_draw_indexed(
                            command_buffer,
                            *index_count,
                            *instance_count,
                            *first_index,
                            base_vertex,
                            *first_instance,
                        )
                    }
                }
                Command::SetBindGroup {
                    index,
                    bind_group,
                    dynamic_offsets,
                } => {
                    // TODO: track and bind descriptor sets

                }
                Command::SetBlendColor { color } => {
                    let blend_constants = [color.r, color.g, color.b, color.a];
                    unsafe {
                        self.device
                            .raw
                            .cmd_set_blend_constants(command_buffer, &blend_constants);
                    }
                }
                Command::SetIndexBuffer { buffer, offset } => {
                    // TODO: set_index_buffer / set_pipeline error handling
                    let pipeline = last_pipeline.expect("RenderPass: set_index_buffer called before set_pipeline");
                    let index_type = index_type(pipeline.descriptor.input_state.index_format);
                    let offset = *offset as u64;
                    unsafe {
                        self.device
                            .raw
                            .cmd_bind_index_buffer(command_buffer, buffer.handle, offset, index_type);
                    }
                }
                Command::SetVertexBuffers {
                    start_slot,
                    count,
                    buffers,
                    offsets,
                } => {
                    let buffers = buffers
                        .iter()
                        .map(|buffer| buffer.handle)
                        .collect::<SmallVec<[vk::Buffer; MAX_VERTEX_INPUTS]>>();
                    unsafe {
                        self.device
                            .raw
                            .cmd_bind_vertex_buffers(command_buffer, *start_slot, &*buffers, offsets);
                    }
                }
                Command::SetRenderPipeline { pipeline } => {
                    last_pipeline = Some(pipeline);
                    let bind_point = vk::PipelineBindPoint::GRAPHICS;
                    unsafe {
                        self.device
                            .raw
                            .cmd_bind_pipeline(command_buffer, bind_point, pipeline.handle);
                    }
                    // TODO on_pipline_layout_change
                }
                Command::SetStencilReference { reference } => {
                    let front_face = vk::StencilFaceFlags::STENCIL_FRONT_AND_BACK;
                    unsafe {
                        self.device
                            .raw
                            .cmd_set_stencil_reference(command_buffer, front_face, *reference);
                    }
                }
                Command::SetScissorRect { x, y, width, height } => {
                    let (x, y, width, height) = (*x as i32, *y as i32, *width, *height);
                    unsafe {
                        self.device.raw.cmd_set_scissor(
                            command_buffer,
                            0,
                            &[vk::Rect2D {
                                offset: vk::Offset2D { x, y },
                                extent: vk::Extent2D { width, height },
                            }],
                        );
                    }
                }
                _ => {
                    // TODO: RenderPass debug commands
                }
            }
            command_index += 1;
        }

        unreachable!()
    }

    fn record_compute_pass(
        &mut self,
        _command_buffer: vk::CommandBuffer,
        command_index: usize,
    ) -> Result<usize, vk::Result> {
        Ok(command_index)
    }
}
