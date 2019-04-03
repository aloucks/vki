use std::sync::Arc;

use crate::imp::{BindGroupInner, BufferInner, ComputePipelineInner, RenderPipelineInner, TextureInner};
use crate::{
    Color, Extent3D, Origin3D, RenderPassColorAttachmentDescriptor, RenderPassDepthStencilAttachmentDescriptor,
    ShaderStageFlags,
};

pub struct BufferCopy {
    pub buffer: Arc<BufferInner>,
    pub offset_bytes: u32,
    pub row_pitch_bytes: u32,
    pub image_height_texels: u32,
}

pub struct TextureCopy {
    pub texture: Arc<TextureInner>,
    pub level: u32,
    pub slice: u32,
    pub origin_texels: Origin3D,
}

pub enum Command {
    BeginComputePass,
    BeginRenderPass {
        color_attachments: Vec<RenderPassColorAttachmentDescriptor>,
        depth_stencil_attachment: Option<RenderPassDepthStencilAttachmentDescriptor>,
        //
        width: u32,
        height: u32,
        sample_count: u32,
    },
    CopyBufferToBuffer {
        source: BufferCopy,
        destination: BufferCopy,
        size_bytes: u32,
    },
    CopyBufferToTexture {
        source: BufferCopy,
        destination: TextureCopy,
        copy_size_texels: Extent3D,
    },
    CopyTextureToBuffer {
        source: TextureCopy,
        destination: BufferCopy,
        copy_size_texels: Extent3D,
    },
    CopyTextureToTexture {
        source: TextureCopy,
        destination: TextureCopy,
        copy_size_texels: Extent3D,
    },
    Dispatch {
        x: u32,
        y: u32,
        z: u32,
    },
    Draw {
        // TODO: Draw
    },
    DrawIndexed {
        // TODO: DrawIndexed
    },
    EndComputePass,
    EndRenderPass,
    InsertDebugMarker {
        // TODO: InsertDebugMarker
    },
    PopDebugGroup,
    PushDebugGroup {
        // TODO: PushDebugGroup
    },
    SetComputePipeline {
        pipeline: Arc<ComputePipelineInner>,
    },
    SetRenderPipeline {
        pipeline: Arc<RenderPipelineInner>,
    },
    SetPushConstants {
        // TODO: SetPushConstants (32bit aligned?)
        stages: ShaderStageFlags,
        offset_bytes: u32, // bytes?
        count: u32,        // bytes?
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
        count: u32,
        buffers: Vec<Arc<BufferInner>>,
    },
}

//lazy_static! {
//    static ref DYNAMIC_OFFSETS: Mutex<Vec<Vec<u64>>> = {
//        let dynamic_offsets = Mutex::new(Vec::new());
//        extern "C" fn free() {
//            DYNAMIC_OFFSETS.lock().clear();
//        }
//        unsafe {
//            libc::atexit(free);
//        }
//        dynamic_offsets
//    };
//}
//
//pub struct DynamicOffsets(Vec<u64>);
//
//impl Drop for DynamicOffsets {
//    fn drop(&mut self) {
//        // Recycle the vec only if it allocated some capacity
//        if self.0.capacity() > 0 {
//            let mut recycle = Vec::new();
//            mem::swap(&mut recycle, &mut self.0);
//            unsafe {
//                recycle.set_len(0);
//            }
//            DYNAMIC_OFFSETS.lock().push(recycle);
//        }
//    }
//}
//
//impl DynamicOffsets {
//    pub fn alloc(dynamic_offsets: &[u64]) -> DynamicOffsets {
//        let mut data = DYNAMIC_OFFSETS.lock().pop().unwrap_or_else(Vec::new);
//        data.extend_from_slice(dynamic_offsets);
//        DynamicOffsets(data)
//    }
//
//    pub fn get(&self) -> &[u64] {
//        &self.0
//    }
//}

#[test]
fn command_size() {
    // The command size can balloon if we embed a SmallVec or fixed sized array. This just
    // raises awareness..
    assert_eq!(80, std::mem::size_of::<Command>());
}

//#[test]
//fn push_constants_alignment() {
//    assert_eq!(32, std::mem::align_of::<PushConstantsData>());
//}
