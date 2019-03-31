use lazy_static::lazy_static;
use libc;
use parking_lot::Mutex;

use crate::imp::{
    BindGroupInner, BufferInner, CommandEncoderInner, ComputePipelineInner, DeviceInner, RenderPipelineInner,
    TextureInner,
};

use crate::{
    BindGroup, Buffer, BufferUsageFlags, Color, CommandEncoder, ComputePassEncoder, Extent3D, Origin3D,
    RenderPassColorAttachmentDescriptor, RenderPassDepthStencilAttachmentDescriptor, RenderPassDescriptor,
    RenderPassEncoder, ShaderStageFlags, TextureUsageFlags,
};

use std::collections::HashSet;
use std::mem;
use std::sync::Arc;

struct PassResourceUsage {
    buffers: Vec<(Arc<BufferInner>, BufferUsageFlags)>,
    textures: Vec<(Arc<TextureInner>, TextureUsageFlags)>,
}

struct CommandBufferResourceUsage {
    per_pass: Vec<PassResourceUsage>,
    top_level_buffers: HashSet<Arc<BufferInner>>,
    top_level_textures: HashSet<Arc<TextureInner>>,
}

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

struct BufferCopy {
    buffer: Arc<BufferInner>,
    offset_bytes: u32,
    row_pitch_bytes: u32,
    image_height_texels: u32,
}

struct TextureCopy {
    texture: Arc<TextureInner>,
    level: u32,
    slice: u32,
    origin_texels: Origin3D,
}

#[repr(align(32))]
struct PushConstantsData([u8; 128]);

enum Command {
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
        // TODO: SetPushConstants; use a pool for the boxed push constants data
        stages: ShaderStageFlags,
        offset_bytes: u32, // bytes?
        count: u32,        // bytes?
        data: Box<PushConstantsData>,
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
        group: Arc<BindGroupInner>,
        dynamic_offsets: Vec<u64>,
    },
    SetIndexBuffer {
        buffer: Arc<BufferInner>,
        offset: u32,
    },
    SetVertexBuffers {
        start_slot: u32,
        count: u32,
    },
}

lazy_static! {
    static ref DYNAMIC_OFFSETS: Mutex<Vec<Vec<u64>>> = {
        let dynamic_offsets = Mutex::new(Vec::new());
        extern "C" fn deallocate() {
            DYNAMIC_OFFSETS.lock().clear();
        }
        unsafe {
            libc::atexit(deallocate);
        }
        dynamic_offsets
    };
}

fn alloc_dynamic_offsets() -> Vec<u64> {
    if let Some(dynamic_offsets) = DYNAMIC_OFFSETS.lock().pop() {
        dynamic_offsets
    } else {
        Vec::new()
    }
}

impl Drop for Command {
    fn drop(&mut self) {
        if let Command::SetBindGroup {
            ref mut dynamic_offsets,
            ..
        } = self
        {
            if dynamic_offsets.capacity() > 0 {
                let mut temp = Vec::new();
                mem::swap(&mut temp, dynamic_offsets);
                unsafe {
                    temp.set_len(0);
                }
                DYNAMIC_OFFSETS.lock().push(temp);
            }
        }
    }
}

#[test]
fn command_size() {
    // The command size can balloon if if embed SmallVec or fixed sized arrays. This just
    // raises awareness..
    assert_eq!(80, std::mem::size_of::<Command>());
}

#[test]
fn push_constants_alignment() {
    assert_eq!(32, std::mem::align_of::<PushConstantsData>());
}

// TODO: Dawn uses block allocation. We'll use a simple global pool and maybe switch to block allocation later.

lazy_static! {
    static ref COMMANDS: Mutex<Vec<Vec<Command>>> = {
        let commands = Mutex::new(Vec::new());
        extern "C" fn deallocate() {
            COMMANDS.lock().clear();
        }
        unsafe {
            libc::atexit(deallocate);
        }
        commands
    };
}

pub struct CommandEncoderState {
    commands: Vec<Command>,
    device: Arc<DeviceInner>,
}

impl Drop for CommandEncoderState {
    fn drop(&mut self) {
        let mut temp = Vec::new();
        mem::swap(&mut temp, &mut self.commands);
        temp.clear();
        COMMANDS.lock().push(temp);
    }
}

impl CommandEncoderState {
    pub fn new(device: Arc<DeviceInner>) -> CommandEncoderState {
        let commands = match COMMANDS.lock().pop() {
            Some(commands) => commands,
            None => Vec::new(),
        };
        CommandEncoderState { commands, device }
    }

    fn push(&mut self, command: Command) {
        self.commands.push(command);
    }
}

impl CommandEncoderInner {
    fn push(&mut self, command: Command) {
        self.state.push(command)
    }

    fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u64]>) {
        let dynamic_offsets = match dynamic_offsets {
            Some(dynamic_offsets) => {
                let mut buffer = alloc_dynamic_offsets();
                buffer.extend_from_slice(dynamic_offsets);
                buffer
            }
            None => Vec::new(),
        };
        self.push(Command::SetBindGroup {
            index,
            dynamic_offsets,
            group: bind_group.inner.clone(),
        });
    }
}

impl CommandEncoder {
    fn push(&mut self, command: Command) {
        self.inner.push(command)
    }

    pub fn begin_compute_pass(&mut self) -> ComputePassEncoder {
        self.push(Command::BeginComputePass);
        ComputePassEncoder {
            top_level_encoder: &mut self.inner,
        }
    }

    pub fn begin_render_pass(&mut self, descriptor: RenderPassDescriptor) -> RenderPassEncoder {
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

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u64]>) {
        self.top_level_encoder
            .set_bind_group(index, bind_group, dynamic_offsets);
    }

    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.top_level_encoder.push(Command::Dispatch { x, y, z });
    }
}

impl<'a> Drop for RenderPassEncoder<'a> {
    fn drop(&mut self) {
        self.top_level_encoder.push(Command::EndComputePass)
    }
}

impl<'a> RenderPassEncoder<'a> {
    pub fn end_pass(self) {
        /* drop */
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, dynamic_offsets: Option<&[u64]>) {
        self.top_level_encoder
            .set_bind_group(index, bind_group, dynamic_offsets);
    }
}
