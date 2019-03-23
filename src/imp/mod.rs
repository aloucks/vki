use ash::extensions::{ext, khr};
use ash::vk;
use parking_lot::{Mutex, ReentrantMutex};

use std::sync::Arc;

mod adapter;
mod convert;
mod debug;
mod device;
mod fenced_deleter;
mod instance;
mod queue;
mod serial;
mod surface;
mod swapchain;
mod texture;

mod cookbook;
mod polyfill;

pub use crate::imp::debug::validate;

use crate::imp::device::DeviceState;
use crate::{Extensions, Limits, TextureDescriptor, TextureUsageFlags};

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

    state: Mutex<DeviceState>,
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
    //images: Vec<vk::Image>,
    textures: Vec<TextureInner>,
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
    descriptor: TextureDescriptor,
    last_usage: Mutex<TextureUsageFlags>,
    owned: bool,
}
