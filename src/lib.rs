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

#[derive(Clone, Debug)]
pub struct Instance {
    inner: Arc<imp::InstanceInner>,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Extent3D {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    R8G8B8A8Unorm,
    R8G8Unorm,
    R8Unorm,
    R8G8B8A8Uint,
    R8G8Uint,
    R8Uint,
    B8G8R8A8Unorm,
    B8G8R8A8UnormSRGB,

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    D1,
    D2,
    D3,
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
    pub dimension: TextureDimension,
    pub aspect: TextureAspectFlags,
    pub base_mip_level: u32,
    pub mip_level_count: u32,
    pub base_array_layer: u32,
    pub array_layer_count: u32,
}

#[derive(Debug)]
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

pub struct Buffer {
    inner: Arc<imp::BufferInner>,
}

pub struct Texture {
    inner: Arc<imp::TextureInner>,
}
