use ash::extensions::{ext, khr};
use ash::version::InstanceV1_0;
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

macro_rules! handle_traits {
    ($Name:ident) => {
        impl PartialEq for $Name {
            fn eq(&self, rhs: &Self) -> bool {
                self.handle.eq(&rhs.handle)
            }
        }

        impl Eq for $Name {}

        impl Hash for $Name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                state.write_u64(self.handle.as_raw())
            }
        }
    };
}

pub struct InstanceInner {
    raw: ash::Instance,
    raw_ext: InstanceExt,
    debug_report_callback: Option<vk::DebugReportCallbackEXT>,
}

impl PartialEq for InstanceInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.raw.handle().eq(&rhs.raw.handle())
    }
}

impl Eq for InstanceInner {}

impl Hash for InstanceInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.raw.handle().as_raw())
    }
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

impl PartialEq for AdapterInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.physical_device.eq(&rhs.physical_device)
    }
}

impl Eq for AdapterInner {}

impl Hash for AdapterInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.physical_device.as_raw())
    }
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

impl PartialEq for DeviceInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.raw.handle().eq(&rhs.raw.handle())
    }
}

impl Eq for DeviceInner {}

impl Hash for DeviceInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.raw.handle().as_raw())
    }
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

handle_traits!(SwapchainInner);

#[derive(Debug)]
pub struct SurfaceInner {
    handle: vk::SurfaceKHR,
    instance: Arc<InstanceInner>,
}

handle_traits!(SurfaceInner);

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

handle_traits!(TextureInner);

#[derive(Debug)]
pub struct TextureViewInner {
    handle: vk::ImageView,
    texture: Arc<TextureInner>,
    descriptor: TextureViewDescriptor,
}

handle_traits!(TextureViewInner);

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

handle_traits!(BufferInner);

#[derive(Debug)]
pub struct SamplerInner {
    handle: vk::Sampler,
    device: Arc<DeviceInner>,
    descriptor: SamplerDescriptor,
}

handle_traits!(SamplerInner);

#[derive(Debug)]
pub struct BindGroupLayoutInner {
    handle: vk::DescriptorSetLayout,
    device: Arc<DeviceInner>,
    bindings: Vec<BindGroupLayoutBinding>,
}

handle_traits!(BindGroupLayoutInner);

#[derive(Debug)]
pub struct BindGroupInner {
    handle: vk::DescriptorSet,
    descriptor_pool: vk::DescriptorPool,
    layout: Arc<BindGroupLayoutInner>,
    // Keep the resources alive as long as the bind group exists
    bindings: Vec<BindGroupBinding>,
}

handle_traits!(BindGroupInner);

pub struct ShaderModuleInner {
    handle: vk::ShaderModule,
    device: Arc<DeviceInner>,
}

handle_traits!(ShaderModuleInner);

#[derive(Debug)]
pub struct PipelineLayoutInner {
    handle: vk::PipelineLayout,
    device: Arc<DeviceInner>,
}

handle_traits!(PipelineLayoutInner);

#[derive(Debug)]
pub struct ComputePipelineInner {
    handle: vk::Pipeline,
    layout: Arc<PipelineLayoutInner>,
}

handle_traits!(ComputePipelineInner);

#[derive(Debug)]
pub struct RenderPipelineInner {
    handle: vk::Pipeline,
    layout: Arc<PipelineLayoutInner>,
}

handle_traits!(RenderPipelineInner);

#[derive(Debug)]
pub struct CommandEncoderInner {
    state: command_encoder::CommandEncoderState,
    device: Arc<DeviceInner>,
}

#[derive(Debug)]
pub struct CommandBufferInner {
    state: command_buffer::CommandBufferState,
    device: Arc<DeviceInner>,
}

#[derive(Debug)]
pub struct ComputePassEncoderInner<'a> {
    top_level_encoder: &'a mut CommandEncoderInner,
    usage_tracker: pass_resource_usage::PassResourceUsageTracker,
}

#[derive(Debug)]
pub struct RenderPassEncoderInner<'a> {
    top_level_encoder: &'a mut CommandEncoderInner,
    usage_tracker: pass_resource_usage::PassResourceUsageTracker,
}
