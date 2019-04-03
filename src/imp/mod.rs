use ash::extensions::{ext, khr};
use ash::vk::{self, Handle};
use parking_lot::{Mutex, ReentrantMutex};
use vk_mem::{Allocation, AllocationInfo};

use std::sync::Arc;

mod adapter;
mod binding;
mod buffer;
mod command;
mod command_buffer;
mod command_encoder;
mod debug;
mod device;
mod fenced_deleter;
mod instance;
mod pass_resource_usage;
mod pipeline;
mod queue;
mod render_pass;
mod sampler;
mod serial;
mod shader;
mod surface;
mod swapchain;
mod texture;
mod util;

mod cookbook;
mod polyfill;

pub use crate::imp::debug::validate;

use crate::{
    BindGroupBinding, BindGroupLayoutBinding, BufferDescriptor, BufferUsageFlags, Extensions, Limits,
    SamplerDescriptor, TextureDescriptor, TextureUsageFlags, TextureViewDescriptor,
};
use std::hash::{Hash, Hasher};

pub struct InstanceInner {
    raw: ash::Instance,
    raw_ext: InstanceExt,
    debug_report_callback: Option<vk::DebugReportCallbackEXT>,
}

/// Instance extension functions
struct InstanceExt {
    surface: khr::Surface,
    #[cfg(windows)]
    surface_win32: khr::Win32Surface,
    debug_utils: ext::DebugUtils,
    debug_report: ext::DebugReport,
}

#[allow(dead_code)]
pub struct AdapterInner {
    instance: Arc<InstanceInner>,
    physical_device: vk::PhysicalDevice,
    physical_device_features: vk::PhysicalDeviceFeatures,
    physical_device_properties: vk::PhysicalDeviceProperties,
    physical_device_format_properties: Vec<(vk::Format, vk::FormatProperties)>,
    queue_family_properties: Vec<vk::QueueFamilyProperties>,
    name: String,
    extensions: Extensions,
}

#[allow(dead_code)]
pub struct DeviceInner {
    raw: ash::Device,
    raw_ext: DeviceExt,
    adapter: Arc<AdapterInner>,
    extensions: Extensions,
    limits: Limits,

    queue: ReentrantMutex<QueueInner>,

    state: Mutex<device::DeviceState>,
}

/// Device extension functions
struct DeviceExt {
    swapchain: khr::Swapchain,
}

// Note: Do not make this cloneable
#[derive(Debug)]
pub struct SwapchainInner {
    handle: vk::SwapchainKHR,
    device: Arc<DeviceInner>,
    surface: Arc<SurfaceInner>,
    //images: Vec<vk::Image>,
    textures: Vec<Arc<TextureInner>>,
    views: Vec<Arc<TextureViewInner>>,
}

#[derive(Debug)]
pub struct SurfaceInner {
    handle: vk::SurfaceKHR,
    instance: Arc<InstanceInner>,
}

#[derive(Copy, Clone, Debug)]
pub struct QueueInner {
    handle: vk::Queue,
    queue_index: u32,
    queue_family_index: u32,
}

#[derive(Debug)]
pub struct TextureInner {
    handle: vk::Image,
    device: Arc<DeviceInner>,
    descriptor: TextureDescriptor,
    last_usage: Mutex<TextureUsageFlags>,
    // if the allocation is None, the image is owned by the swapchain
    allocation: Option<Allocation>,
    allocation_info: Option<AllocationInfo>,
}

impl PartialEq for TextureInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle.eq(&rhs.handle)
    }
}

impl Eq for TextureInner {}

impl Hash for TextureInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.handle.as_raw())
    }
}

#[derive(Debug)]
pub struct TextureViewInner {
    handle: vk::ImageView,
    texture: Arc<TextureInner>,
    descriptor: TextureViewDescriptor,
}

impl PartialEq for TextureViewInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle.eq(&rhs.handle)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BufferState {
    Mapped,
    Unmapped,
    Destroyed,
}

#[derive(Debug)]
pub struct BufferInner {
    handle: vk::Buffer,
    device: Arc<DeviceInner>,
    descriptor: BufferDescriptor,
    allocation: Allocation,
    allocation_info: AllocationInfo,
    last_usage: Mutex<BufferUsageFlags>,
    buffer_state: Mutex<BufferState>,
}

impl PartialEq for BufferInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.handle.eq(&rhs.handle)
    }
}

impl Eq for BufferInner {}

impl Hash for BufferInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.handle.as_raw())
    }
}

#[derive(Debug)]
pub struct SamplerInner {
    handle: vk::Sampler,
    device: Arc<DeviceInner>,
    descriptor: SamplerDescriptor,
}

#[derive(Debug)]
pub struct BindGroupLayoutInner {
    handle: vk::DescriptorSetLayout,
    device: Arc<DeviceInner>,
    bindings: Vec<BindGroupLayoutBinding>,
}

#[derive(Debug)]
pub struct BindGroupInner {
    handle: vk::DescriptorSet,
    descriptor_pool: vk::DescriptorPool,
    layout: Arc<BindGroupLayoutInner>,
    // Keep the resources alive as long as the bind group exists
    bindings: Vec<BindGroupBinding>,
}

pub struct ShaderModuleInner {
    handle: vk::ShaderModule,
    device: Arc<DeviceInner>,
}

#[derive(Debug)]
pub struct PipelineLayoutInner {
    handle: vk::PipelineLayout,
    device: Arc<DeviceInner>,
}

#[derive(Debug)]
pub struct ComputePipelineInner {
    handle: vk::Pipeline,
    layout: Arc<PipelineLayoutInner>,
}

#[derive(Debug)]
pub struct RenderPipelineInner {
    handle: vk::Pipeline,
    layout: Arc<PipelineLayoutInner>,
}

pub struct CommandEncoderInner {
    state: command_encoder::CommandEncoderState,
    device: Arc<DeviceInner>,
}

pub struct CommandBufferInner {
    state: command_buffer::CommandBufferState,
    device: Arc<DeviceInner>,
}
