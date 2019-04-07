#![allow(clippy::needless_lifetimes)]
#![allow(dead_code)]

// TODO: Using `extern crate` here helps intellij find `bitflags!` defined structs,
//       although auto-completion for the associated constants still does not work.
#[macro_use]
extern crate bitflags;
//use bitflags::bitflags;

use std::sync::Arc;

use parking_lot::ReentrantMutexGuard;

#[macro_use]
mod macros;
mod error;
mod imp;

pub use crate::error::InitError;
pub use crate::imp::validate;

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Instance {
    inner: Arc<imp::InstanceInner>,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum PowerPreference {
    LowPower,
    HighPerformance,
}

impl Default for PowerPreference {
    fn default() -> PowerPreference {
        PowerPreference::HighPerformance
    }
}

#[derive(Clone, Debug, Default)]
pub struct RequestAdapterOptions {
    pub power_preference: PowerPreference,
}

#[derive(Clone, Debug, Default)]
pub struct Extensions {
    pub anisotropic_filtering: bool,
}

#[derive(Clone)]
pub struct Adapter {
    inner: Arc<imp::AdapterInner>,
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_bind_groups: u32,
}

#[derive(Clone, Debug, Default)]
pub struct DeviceDescriptor<'a> {
    pub extensions: Extensions,
    /// The queue created for the device will have support for the provided surface
    pub surface_support: Option<&'a Surface>,
    // pub queue_descriptors: &'a [QueueDescriptor<'a>],
}

impl<'a> DeviceDescriptor<'a> {
    pub fn with_surface_support(mut self, surface: &'a Surface) -> DeviceDescriptor<'a> {
        self.surface_support = Some(surface);
        self
    }
}

#[derive(Clone)]
pub struct Device {
    inner: Arc<imp::DeviceInner>,
}

#[derive(Clone, Copy, Debug)]
pub struct SwapchainDescriptor<'a> {
    pub surface: &'a Surface,
    pub format: TextureFormat,
    pub usage: TextureUsageFlags,
}

impl<'a> SwapchainDescriptor<'a> {
    pub fn default_with_surface(surface: &'a Surface) -> SwapchainDescriptor<'a> {
        SwapchainDescriptor {
            surface,
            format: TextureFormat::B8G8R8A8UnormSRGB,
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
        }
    }
}

// Note: Do not make this cloneable
#[derive(Debug)]
pub struct Swapchain {
    inner: Arc<imp::SwapchainInner>,
}

#[derive(Debug)]
pub struct SurfaceDescriptorWin32 {
    pub hwnd: *const std::ffi::c_void,
}

#[cfg(windows)]
pub type SurfaceDescriptor = SurfaceDescriptorWin32;

#[derive(Clone, Debug)]
pub struct Surface {
    inner: Arc<imp::SurfaceInner>,
}

pub struct Queue<'a> {
    inner: ReentrantMutexGuard<'a, imp::QueueInner>,
}

pub struct SwapchainImage {
    // TODO: See if this can still be ergonomic with a reference instead
    swapchain: Arc<imp::SwapchainInner>,
    image_index: u32,
    pub texture: Texture,
    pub view: TextureView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct Extent3D {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct Origin3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    // 8-bit formats
    R8Unorm,
    R8UnormSRGB,
    R8Snorm,
    R8Uint,
    R8Sint,

    // TODO: Update 16-bit formats
    R8G8Unorm,
    R8G8Uint,

    // TODO: Update 32-bit formats
    R8G8B8A8Unorm,
    R8G8B8A8Uint,
    B8G8R8A8Unorm,
    B8G8R8A8UnormSRGB,

    D32Float,
    D32FloatS8Uint,
}

bitflags! {
    #[repr(transparent)]
    pub struct TextureUsageFlags: u32 {
        const NONE = 0;
        const TRANSFER_SRC = 1;
        const TRANSFER_DST = 2;
        const SAMPLED = 4;
        const STORAGE = 8;
        const OUTPUT_ATTACHMENT = 16;
        #[doc(hidden)]
        const PRESENT = 32;
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    D1,
    D2,
    D3,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureViewDimension {
    D1,
    D2,
    D3,
    Cube,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureDescriptor {
    pub size: Extent3D,
    pub array_layer_count: u32,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub dimension: TextureDimension,
    pub format: TextureFormat,
    pub usage: TextureUsageFlags,
}

bitflags! {
    #[repr(transparent)]
    pub struct TextureAspectFlags: u32 {
        const COLOR = 0b1;      // vk::ImageAspectFlags::COLOR;
        const DEPTH = 0b10;     // vk::ImageAspectFlags::DEPTH;
        const STENCIL = 0b1000; // vk::ImageAspectFlags::STENCIL;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureViewDescriptor {
    pub format: TextureFormat,
    pub dimension: TextureViewDimension,
    pub aspect: TextureAspectFlags,
    pub base_mip_level: u32,
    pub mip_level_count: u32,
    pub base_array_layer: u32,
    pub array_layer_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureView {
    inner: Arc<imp::TextureViewInner>,
}

bitflags! {
    #[repr(transparent)]
    pub struct BufferUsageFlags: u32 {
        const NONE = 0;
        const MAP_READ = 1;
        const MAP_WRITE = 2;
        const TRANSFER_SRC = 4;
        const TRANSFER_DST = 8;
        const INDEX = 16;
        const VERTEX = 32;
        const UNIFORM = 64;
        const STORAGE = 128;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferDescriptor {
    pub size: u64,
    pub usage: BufferUsageFlags,
}

#[derive(Clone, Debug)]
pub struct Buffer {
    inner: Arc<imp::BufferInner>,
}

pub struct MappedBuffer {
    inner: Arc<imp::BufferInner>,
    data: *mut u8,
}

#[derive(Debug)]
pub struct Fence {
    inner: imp::FenceInner,
}

#[derive(Clone, Debug)]
pub struct Texture {
    inner: Arc<imp::TextureInner>,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDescriptor {
    pub address_mode_u: AddressMode,
    pub address_mode_v: AddressMode,
    pub address_mode_w: AddressMode,
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: FilterMode,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    pub compare_function: CompareFunction,
}

impl Eq for SamplerDescriptor {}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for SamplerDescriptor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use std::{mem, slice};
        let size = mem::size_of::<SamplerDescriptor>();
        let bytes = unsafe { slice::from_raw_parts(self as *const _ as *const u8, size) };
        state.write(bytes);
    }
}

impl Default for SamplerDescriptor {
    fn default() -> SamplerDescriptor {
        SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: std::f32::MAX,
            compare_function: CompareFunction::Never,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sampler {
    inner: Arc<imp::SamplerInner>,
}

bitflags! {
    #[repr(transparent)]
    pub struct ShaderStageFlags: u32 {
        const NONE = 0;
        const VERTEX = 1;
        const FRAGMENT = 2;
        const COMPUTE = 4;
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindingType {
    UniformBuffer,
    DynamicUniformBuffer,
    Sampler,
    SampledTexture,
    StorageBuffer,
    //StorageTexture, // TOOD: Not GpuWeb
    DynamicStorageBuffer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindGroupLayoutBinding {
    pub binding: u32,
    pub visibility: ShaderStageFlags,
    pub binding_type: BindingType,
}

#[derive(Clone, Copy, Debug)]
pub struct BindGroupLayoutDescriptor<'a> {
    pub bindings: &'a [BindGroupLayoutBinding],
}

#[derive(Clone, Debug)]
pub struct BindGroupLayout {
    inner: Arc<imp::BindGroupLayoutInner>,
}

#[derive(Clone, Debug)]
pub enum BindingResource {
    Sampler(Sampler),
    TextureView(TextureView),
    Buffer(Buffer, Range<u64>),
}

impl BindingResource {
    pub fn as_sampler(&self) -> Option<&Sampler> {
        if let BindingResource::Sampler(ref sampler) = self {
            Some(sampler)
        } else {
            None
        }
    }

    pub fn as_texture_view(&self) -> Option<&TextureView> {
        if let BindingResource::TextureView(ref texture_view) = self {
            Some(texture_view)
        } else {
            None
        }
    }

    pub fn as_buffer(&self) -> Option<(&Buffer, &Range<u64>)> {
        if let BindingResource::Buffer(ref buffer, range) = self {
            Some((buffer, range))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct BindGroupBinding {
    pub binding: u32,
    pub resource: BindingResource,
}

#[derive(Clone, Debug)]
pub struct BindGroupDescriptor {
    pub layout: BindGroupLayout,
    pub bindings: Vec<BindGroupBinding>,
}

#[derive(Clone, Debug)]
pub struct BindGroup {
    inner: Arc<imp::BindGroupInner>,
}

#[derive(Clone, Debug)]
pub struct PipelineLayoutDescriptor {
    pub bind_group_layouts: Vec<BindGroupLayout>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PipelineLayout {
    inner: Arc<imp::PipelineLayoutInner>,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrontFace {
    Ccw,
    Cw,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstColor,
    OneMinusDstColor,
    DstAlpha,
    OneMinusDstAlpha,
    BlendColor,
    OneMinusBlendColor,
    SrcAlphaSaturated,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

bitflags! {
    #[repr(transparent)]
    pub struct ColorWriteFlags: u32 {
        const NONE = 0;
        const RED = 1;
        const GREEN = 2;
        const BLUE = 4;
        const ALPHA = 8;
        const ALL = 15;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendDescriptor {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColorStateDescriptor {
    pub format: TextureFormat,
    pub alpha_blend: BlendDescriptor,
    pub color_blend: BlendDescriptor,
    pub write_mask: ColorWriteFlags,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    Invert,
    IncrementClamp,
    DecrementClamp,
    IncrementWrap,
    DecrementWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StencilStateFaceDescriptor {
    pub compare: CompareFunction,
    pub fail_op: StencilOperation,
    pub depth_fail_op: StencilOperation,
    pub pass_op: StencilOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepthStencilStateDescriptor {
    pub format: TextureFormat,
    pub depth_write_enabled: bool,
    pub depth_compare: CompareFunction,
    pub stencil_front: StencilStateFaceDescriptor,
    pub stencil_back: StencilStateFaceDescriptor,
    pub stencil_read_mask: u32,
    pub stencil_write_mask: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderModuleDescriptor {
    pub code: Cow<'static, [u8]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderModule {
    inner: Arc<imp::ShaderModuleInner>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PipelineStageDescriptor {
    pub module: ShaderModule,
    pub entry_point: Cow<'static, str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputePipelineDescriptor {
    pub layout: PipelineLayout,
    pub compute_stage: PipelineStageDescriptor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputePipeline {
    inner: Arc<imp::ComputePipelineInner>,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizationStateDescriptor {
    pub front_face: FrontFace,
    pub cull_mode: CullMode,
    pub depth_bias: i32,
    pub depth_bias_slope_scale: f32,
    pub depth_bias_clamp: f32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexFormat {
    U16,
    U32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputStepMode {
    Vertex,
    Instance,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    UChar2,
    UChar4,
    Char2,
    Char4,
    UChar2Norm,
    UChar4Norm,
    Char2Norm,
    Char4Norm,

    UShort2,
    UShort4,
    Short2,
    Short4,
    UShort2Norm,
    UShort4Norm,
    Short2Norm,
    Short4Norm,

    Half2,
    Half4,
    Float,
    Float2,
    Float3,
    Float4,

    UInt,
    UInt2,
    UInt3,
    UInt4,
    Int,
    Int2,
    Int3,
    Int4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexAttributeDescriptor {
    pub shader_location: u32,
    pub input_slot: u32,
    pub offset: u32,
    pub format: VertexFormat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexInputDescriptor {
    pub input_slot: u32,
    pub stride: u64,
    pub step_mode: InputStepMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputStateDescriptor {
    pub index_format: IndexFormat,
    pub attributes: Vec<VertexAttributeDescriptor>,
    pub inputs: Vec<VertexInputDescriptor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPipelineDescriptor {
    pub layout: PipelineLayout,
    pub vertex_stage: PipelineStageDescriptor,
    pub fragment_stage: PipelineStageDescriptor,
    pub primitive_topology: PrimitiveTopology,
    pub rasterization_state: RasterizationStateDescriptor,
    pub color_states: Vec<ColorStateDescriptor>,
    pub depth_stencil_state: Option<DepthStencilStateDescriptor>,
    pub input_state: InputStateDescriptor,
}

pub struct RenderPipeline {
    inner: Arc<imp::RenderPipelineInner>,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadOp {
    Clear,
    Load,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreOp {
    Store,
}

#[derive(Debug)]
pub struct CommandEncoder {
    inner: imp::CommandEncoderInner,
}

#[derive(Debug)]
pub struct ComputePassEncoder<'a> {
    inner: imp::ComputePassEncoderInner<'a>,
}

#[derive(Debug)]
pub struct RenderPassEncoder<'a> {
    inner: imp::RenderPassEncoderInner<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassColorAttachmentDescriptor<'a> {
    pub attachment: &'a TextureView,
    pub resolve_target: Option<&'a TextureView>,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub clear_color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassDepthStencilAttachmentDescriptor<'a> {
    pub attachment: &'a TextureView,
    pub depth_load_op: LoadOp,
    pub depth_store_op: StoreOp,
    pub clear_depth: f32,
    pub stencil_load_op: LoadOp,
    pub stencil_store_op: StoreOp,
    pub clear_stencil: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassDescriptor<'a> {
    pub color_attachments: &'a [RenderPassColorAttachmentDescriptor<'a>],
    pub depth_stencil_attachment: Option<RenderPassDepthStencilAttachmentDescriptor<'a>>,
}

#[derive(Debug)]
pub struct CommandBuffer {
    inner: imp::CommandBufferInner,
}

pub struct CommandEncoderDescriptor {}

#[derive(Clone, Debug)]
pub struct BufferCopyView<'a> {
    pub buffer: &'a Buffer,
    pub offset: u64,
    pub row_pitch: u32,
    pub image_height: u32,
}

#[derive(Clone, Debug)]
pub struct TextureCopyView<'a> {
    pub texture: &'a Texture,
    pub mip_level: u32,
    pub array_layer: u32,
    pub origin: Origin3D,
}
