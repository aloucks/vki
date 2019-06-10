use std::sync::Arc;

use crate::imp::command_encoder::{RenderPassColorAttachmentInfo, RenderPassDepthStencilAttachmentInfo};
use crate::imp::{BindGroupInner, BufferInner, ComputePipelineInner, RenderPipelineInner, TextureInner};
use crate::{Buffer, Color, Extent3D, FilterMode, Origin3D, ShaderStageFlags};

#[derive(Debug)]
pub struct BufferCopy {
    pub buffer: Arc<BufferInner>,
    pub offset: usize,
    pub row_pitch: u32,
    pub image_height: u32,
}

#[derive(Debug)]
pub struct TextureCopy {
    pub texture: Arc<TextureInner>,
    pub mip_level: u32,
    pub array_layer: u32,
    pub origin_texels: Origin3D,
}

#[derive(Debug)]
pub struct TextureBlit {
    pub texture: Arc<TextureInner>,
    pub mip_level: u32,
    pub array_layer: u32,
    pub bounds_texels: [Origin3D; 2],
}

#[derive(Debug)]
pub enum Command {
    BeginComputePass,
    BeginRenderPass {
        color_attachments: Vec<RenderPassColorAttachmentInfo>,
        depth_stencil_attachment: Option<RenderPassDepthStencilAttachmentInfo>,
        //
        width: u32,
        height: u32,
        sample_count: u32,
    },
    CopyBufferToBuffer {
        src: BufferCopy,
        dst: BufferCopy,
        size_bytes: usize,
    },
    CopyBufferToTexture {
        src: BufferCopy,
        dst: TextureCopy,
        size_texels: Extent3D,
    },
    CopyTextureToBuffer {
        src: TextureCopy,
        dst: BufferCopy,
        size_texels: Extent3D,
    },
    CopyTextureToTexture {
        src: TextureCopy,
        dst: TextureCopy,
        size_texels: Extent3D,
    },
    BlitTextureToTexture {
        src: TextureBlit,
        dst: TextureBlit,
        filter: FilterMode,
    },
    Dispatch {
        x: u32,
        y: u32,
        z: u32,
    },
    Draw {
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    DrawIndexed {
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    },
    DrawIndirect {
        buffer: Buffer,
        indirect_offset: usize,
    },
    DrawIndexedIndirect {
        buffer: Buffer,
        indirect_offset: usize,
    },
    DispatchIndirect {
        buffer: Buffer,
        indirect_offset: usize,
    },
    EndComputePass,
    EndRenderPass,
    InsertDebugMarker {
        marker_label: String,
    },
    PopDebugGroup,
    PushDebugGroup {
        group_label: String,
    },
    SetComputePipeline {
        pipeline: Arc<ComputePipelineInner>,
    },
    SetRenderPipeline {
        pipeline: Arc<RenderPipelineInner>,
    },
    SetPushConstants {
        stages: ShaderStageFlags,
        offset_bytes: u32,
        size_bytes: u32,
        values: Vec<u8>,
    },
    SetStencilReference {
        reference: u32,
    },
    SetScissorRect {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    SetBlendColor {
        color: Color,
    },
    SetBindGroup {
        index: u32,
        bind_group: Arc<BindGroupInner>,
        dynamic_offsets: Option<Vec<u32>>,
    },
    SetIndexBuffer {
        buffer: Arc<BufferInner>,
        offset: u32,
    },
    SetVertexBuffers {
        start_slot: u32,
        buffers: Vec<Arc<BufferInner>>,
        offsets: Vec<u64>,
    },
    SetViewport {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    },
}

#[test]
fn command_size() {
    // The command size can balloon if we embed a SmallVec or fixed sized array. This just
    // raises awareness..
    assert_eq!(88, std::mem::size_of::<Command>());
}
