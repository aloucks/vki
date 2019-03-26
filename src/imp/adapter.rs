use crate::imp::{AdapterInner, DeviceInner, InstanceInner, SurfaceInner};
use crate::{Adapter, Device, DeviceDescriptor, Extensions, PowerPreference, RequestAdapterOptions};

use ash::version::InstanceV1_0;
use ash::vk;

use std::ffi::CStr;
use std::fmt::{self, Debug};
use std::sync::Arc;

impl Adapter {
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn extensions(&self) -> &Extensions {
        &self.inner.extensions
    }

    pub fn create_device(&self, descriptor: DeviceDescriptor) -> Result<Device, vk::Result> {
        let device = DeviceInner::new(self.inner.clone(), descriptor)?;
        Ok(device.into())
    }
}

impl Into<Adapter> for AdapterInner {
    fn into(self) -> Adapter {
        Adapter { inner: Arc::new(self) }
    }
}

impl AdapterInner {
    pub fn new(instance: Arc<InstanceInner>, options: RequestAdapterOptions) -> Result<AdapterInner, vk::Result> {
        fn new_inner(
            instance: Arc<InstanceInner>,
            physical_device: vk::PhysicalDevice,
            physical_device_properties: vk::PhysicalDeviceProperties,
        ) -> Result<AdapterInner, vk::Result> {
            let (extensions, physical_device_features) = unsafe {
                // TODO: capture these
                for p in instance
                    .raw
                    .enumerate_device_extension_properties(physical_device)?
                    .iter()
                {
                    let name = CStr::from_ptr(p.extension_name.as_ptr());
                    log::debug!("found physical device extension: {}", name.to_string_lossy());
                }

                // TODO: capture these
                let mut num_layers = 0;
                instance.raw.fp_v1_0().enumerate_device_layer_properties(
                    physical_device,
                    &mut num_layers,
                    std::ptr::null_mut(),
                );
                let mut layers = vec![vk::LayerProperties::default(); num_layers as usize];
                instance.raw.fp_v1_0().enumerate_device_layer_properties(
                    physical_device,
                    &mut num_layers,
                    layers.as_mut_ptr(),
                );
                for p in layers.iter() {
                    let name = CStr::from_ptr(p.layer_name.as_ptr());
                    log::debug!("found physical device layer: {}", name.to_string_lossy());
                }

                let physical_device_features = instance.raw.get_physical_device_features(physical_device);
                let extensions = Extensions {
                    anisotropic_filtering: physical_device_features.sampler_anisotropy == vk::TRUE,
                };
                (extensions, physical_device_features)
            };

            let mut physical_device_format_properties = Vec::new();
            for format in VK_FORMATS.iter().cloned() {
                let format_properties = unsafe {
                    instance
                        .raw
                        .get_physical_device_format_properties(physical_device, format)
                };
                physical_device_format_properties.push((format, format_properties));
            }
            physical_device_format_properties.sort_by(|(a, _), (b, _)| a.cmp(&b));

            let queue_family_properties = unsafe {
                instance
                    .raw
                    .get_physical_device_queue_family_properties(physical_device)
            };

            let name = unsafe { CStr::from_ptr(physical_device_properties.device_name.as_ptr()) }
                .to_string_lossy()
                .into_owned();

            Ok(AdapterInner {
                instance,
                physical_device,
                name,
                physical_device_features,
                physical_device_properties,
                physical_device_format_properties,
                queue_family_properties,
                extensions,
            })
        }

        unsafe {
            let physical_devices = instance.raw.enumerate_physical_devices()?;
            let physical_device_properties = &mut Vec::with_capacity(physical_devices.len());
            for physical_device in physical_devices.iter().cloned() {
                physical_device_properties.push(instance.raw.get_physical_device_properties(physical_device));
            }
            match options.power_preference {
                PowerPreference::HighPerformance => {
                    for (index, properties) in physical_device_properties.iter().enumerate() {
                        if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                            let inner = new_inner(instance.clone(), physical_devices[index], *properties)?;
                            return Ok(inner);
                        }
                    }
                }
                PowerPreference::LowPower => {
                    for (index, properties) in physical_device_properties.iter().enumerate() {
                        if properties.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
                            let inner = new_inner(instance.clone(), physical_devices[index], *properties)?;
                            return Ok(inner);
                        }
                    }
                }
            }
            physical_devices
                .first()
                .cloned()
                .ok_or(vk::Result::ERROR_INITIALIZATION_FAILED)
                .and_then(|physical_device| new_inner(instance, physical_device, physical_device_properties[0]))
        }
    }

    pub fn get_surface_support(&self, surface: &SurfaceInner, queue_index: u32) -> Result<bool, vk::Result> {
        unsafe {
            Ok(self.instance.raw_ext.surface.get_physical_device_surface_support(
                self.physical_device,
                queue_index as u32,
                surface.handle,
            ))
        }
    }
}

impl Debug for Adapter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Adapter")
            .field("name", &self.inner.name)
            .field("physical_device", &self.inner.physical_device)
            //.field("physical_device_properties", &self.inner.physical_device_properties)
            //.field("physical_device_features", &self.inner.physical_device_features)
            .finish()
    }
}

impl RequestAdapterOptions {
    pub fn new() -> RequestAdapterOptions {
        RequestAdapterOptions::default()
    }

    pub fn high_performance(mut self) -> Self {
        self.power_preference = PowerPreference::HighPerformance;
        self
    }

    pub fn low_power(mut self) -> Self {
        self.power_preference = PowerPreference::LowPower;
        self
    }
}

static VK_FORMATS: &'static [vk::Format] = &[
    vk::Format::UNDEFINED,
    vk::Format::R4G4_UNORM_PACK8,
    vk::Format::R4G4B4A4_UNORM_PACK16,
    vk::Format::B4G4R4A4_UNORM_PACK16,
    vk::Format::R5G6B5_UNORM_PACK16,
    vk::Format::B5G6R5_UNORM_PACK16,
    vk::Format::R5G5B5A1_UNORM_PACK16,
    vk::Format::B5G5R5A1_UNORM_PACK16,
    vk::Format::A1R5G5B5_UNORM_PACK16,
    vk::Format::R8_UNORM,
    vk::Format::R8_SNORM,
    vk::Format::R8_USCALED,
    vk::Format::R8_SSCALED,
    vk::Format::R8_UINT,
    vk::Format::R8_SINT,
    vk::Format::R8_SRGB,
    vk::Format::R8G8_UNORM,
    vk::Format::R8G8_SNORM,
    vk::Format::R8G8_USCALED,
    vk::Format::R8G8_SSCALED,
    vk::Format::R8G8_UINT,
    vk::Format::R8G8_SINT,
    vk::Format::R8G8_SRGB,
    vk::Format::R8G8B8_UNORM,
    vk::Format::R8G8B8_SNORM,
    vk::Format::R8G8B8_USCALED,
    vk::Format::R8G8B8_SSCALED,
    vk::Format::R8G8B8_UINT,
    vk::Format::R8G8B8_SINT,
    vk::Format::R8G8B8_SRGB,
    vk::Format::B8G8R8_UNORM,
    vk::Format::B8G8R8_SNORM,
    vk::Format::B8G8R8_USCALED,
    vk::Format::B8G8R8_SSCALED,
    vk::Format::B8G8R8_UINT,
    vk::Format::B8G8R8_SINT,
    vk::Format::B8G8R8_SRGB,
    vk::Format::R8G8B8A8_UNORM,
    vk::Format::R8G8B8A8_SNORM,
    vk::Format::R8G8B8A8_USCALED,
    vk::Format::R8G8B8A8_SSCALED,
    vk::Format::R8G8B8A8_UINT,
    vk::Format::R8G8B8A8_SINT,
    vk::Format::R8G8B8A8_SRGB,
    vk::Format::B8G8R8A8_UNORM,
    vk::Format::B8G8R8A8_SNORM,
    vk::Format::B8G8R8A8_USCALED,
    vk::Format::B8G8R8A8_SSCALED,
    vk::Format::B8G8R8A8_UINT,
    vk::Format::B8G8R8A8_SINT,
    vk::Format::B8G8R8A8_SRGB,
    vk::Format::A8B8G8R8_UNORM_PACK32,
    vk::Format::A8B8G8R8_SNORM_PACK32,
    vk::Format::A8B8G8R8_USCALED_PACK32,
    vk::Format::A8B8G8R8_SSCALED_PACK32,
    vk::Format::A8B8G8R8_UINT_PACK32,
    vk::Format::A8B8G8R8_SINT_PACK32,
    vk::Format::A8B8G8R8_SRGB_PACK32,
    vk::Format::A2R10G10B10_UNORM_PACK32,
    vk::Format::A2R10G10B10_SNORM_PACK32,
    vk::Format::A2R10G10B10_USCALED_PACK32,
    vk::Format::A2R10G10B10_SSCALED_PACK32,
    vk::Format::A2R10G10B10_UINT_PACK32,
    vk::Format::A2R10G10B10_SINT_PACK32,
    vk::Format::A2B10G10R10_UNORM_PACK32,
    vk::Format::A2B10G10R10_SNORM_PACK32,
    vk::Format::A2B10G10R10_USCALED_PACK32,
    vk::Format::A2B10G10R10_SSCALED_PACK32,
    vk::Format::A2B10G10R10_UINT_PACK32,
    vk::Format::A2B10G10R10_SINT_PACK32,
    vk::Format::R16_UNORM,
    vk::Format::R16_SNORM,
    vk::Format::R16_USCALED,
    vk::Format::R16_SSCALED,
    vk::Format::R16_UINT,
    vk::Format::R16_SINT,
    vk::Format::R16_SFLOAT,
    vk::Format::R16G16_UNORM,
    vk::Format::R16G16_SNORM,
    vk::Format::R16G16_USCALED,
    vk::Format::R16G16_SSCALED,
    vk::Format::R16G16_UINT,
    vk::Format::R16G16_SINT,
    vk::Format::R16G16_SFLOAT,
    vk::Format::R16G16B16_UNORM,
    vk::Format::R16G16B16_SNORM,
    vk::Format::R16G16B16_USCALED,
    vk::Format::R16G16B16_SSCALED,
    vk::Format::R16G16B16_UINT,
    vk::Format::R16G16B16_SINT,
    vk::Format::R16G16B16_SFLOAT,
    vk::Format::R16G16B16A16_UNORM,
    vk::Format::R16G16B16A16_SNORM,
    vk::Format::R16G16B16A16_USCALED,
    vk::Format::R16G16B16A16_SSCALED,
    vk::Format::R16G16B16A16_UINT,
    vk::Format::R16G16B16A16_SINT,
    vk::Format::R16G16B16A16_SFLOAT,
    vk::Format::R32_UINT,
    vk::Format::R32_SINT,
    vk::Format::R32_SFLOAT,
    vk::Format::R32G32_UINT,
    vk::Format::R32G32_SINT,
    vk::Format::R32G32_SFLOAT,
    vk::Format::R32G32B32_UINT,
    vk::Format::R32G32B32_SINT,
    vk::Format::R32G32B32_SFLOAT,
    vk::Format::R32G32B32A32_UINT,
    vk::Format::R32G32B32A32_SINT,
    vk::Format::R32G32B32A32_SFLOAT,
    vk::Format::R64_UINT,
    vk::Format::R64_SINT,
    vk::Format::R64_SFLOAT,
    vk::Format::R64G64_UINT,
    vk::Format::R64G64_SINT,
    vk::Format::R64G64_SFLOAT,
    vk::Format::R64G64B64_UINT,
    vk::Format::R64G64B64_SINT,
    vk::Format::R64G64B64_SFLOAT,
    vk::Format::R64G64B64A64_UINT,
    vk::Format::R64G64B64A64_SINT,
    vk::Format::R64G64B64A64_SFLOAT,
    vk::Format::B10G11R11_UFLOAT_PACK32,
    vk::Format::E5B9G9R9_UFLOAT_PACK32,
    vk::Format::D16_UNORM,
    vk::Format::X8_D24_UNORM_PACK32,
    vk::Format::D32_SFLOAT,
    vk::Format::S8_UINT,
    vk::Format::D16_UNORM_S8_UINT,
    vk::Format::D24_UNORM_S8_UINT,
    vk::Format::D32_SFLOAT_S8_UINT,
    vk::Format::BC1_RGB_UNORM_BLOCK,
    vk::Format::BC1_RGB_SRGB_BLOCK,
    vk::Format::BC1_RGBA_UNORM_BLOCK,
    vk::Format::BC1_RGBA_SRGB_BLOCK,
    vk::Format::BC2_UNORM_BLOCK,
    vk::Format::BC2_SRGB_BLOCK,
    vk::Format::BC3_UNORM_BLOCK,
    vk::Format::BC3_SRGB_BLOCK,
    vk::Format::BC4_UNORM_BLOCK,
    vk::Format::BC4_SNORM_BLOCK,
    vk::Format::BC5_UNORM_BLOCK,
    vk::Format::BC5_SNORM_BLOCK,
    vk::Format::BC6H_UFLOAT_BLOCK,
    vk::Format::BC6H_SFLOAT_BLOCK,
    vk::Format::BC7_UNORM_BLOCK,
    vk::Format::BC7_SRGB_BLOCK,
    vk::Format::ETC2_R8G8B8_UNORM_BLOCK,
    vk::Format::ETC2_R8G8B8_SRGB_BLOCK,
    vk::Format::ETC2_R8G8B8A1_UNORM_BLOCK,
    vk::Format::ETC2_R8G8B8A1_SRGB_BLOCK,
    vk::Format::ETC2_R8G8B8A8_UNORM_BLOCK,
    vk::Format::ETC2_R8G8B8A8_SRGB_BLOCK,
    vk::Format::EAC_R11_UNORM_BLOCK,
    vk::Format::EAC_R11_SNORM_BLOCK,
    vk::Format::EAC_R11G11_UNORM_BLOCK,
    vk::Format::EAC_R11G11_SNORM_BLOCK,
    vk::Format::ASTC_4X4_UNORM_BLOCK,
    vk::Format::ASTC_4X4_SRGB_BLOCK,
    vk::Format::ASTC_5X4_UNORM_BLOCK,
    vk::Format::ASTC_5X4_SRGB_BLOCK,
    vk::Format::ASTC_5X5_UNORM_BLOCK,
    vk::Format::ASTC_5X5_SRGB_BLOCK,
    vk::Format::ASTC_6X5_UNORM_BLOCK,
    vk::Format::ASTC_6X5_SRGB_BLOCK,
    vk::Format::ASTC_6X6_UNORM_BLOCK,
    vk::Format::ASTC_6X6_SRGB_BLOCK,
    vk::Format::ASTC_8X5_UNORM_BLOCK,
    vk::Format::ASTC_8X5_SRGB_BLOCK,
    vk::Format::ASTC_8X6_UNORM_BLOCK,
    vk::Format::ASTC_8X6_SRGB_BLOCK,
    vk::Format::ASTC_8X8_UNORM_BLOCK,
    vk::Format::ASTC_8X8_SRGB_BLOCK,
    vk::Format::ASTC_10X5_UNORM_BLOCK,
    vk::Format::ASTC_10X5_SRGB_BLOCK,
    vk::Format::ASTC_10X6_UNORM_BLOCK,
    vk::Format::ASTC_10X6_SRGB_BLOCK,
    vk::Format::ASTC_10X8_UNORM_BLOCK,
    vk::Format::ASTC_10X8_SRGB_BLOCK,
    vk::Format::ASTC_10X10_UNORM_BLOCK,
    vk::Format::ASTC_10X10_SRGB_BLOCK,
    vk::Format::ASTC_12X10_UNORM_BLOCK,
    vk::Format::ASTC_12X10_SRGB_BLOCK,
    vk::Format::ASTC_12X12_UNORM_BLOCK,
    vk::Format::ASTC_12X12_SRGB_BLOCK,
    vk::Format::G8B8G8R8_422_UNORM,
    vk::Format::B8G8R8G8_422_UNORM,
    vk::Format::G8_B8_R8_3PLANE_420_UNORM,
    vk::Format::G8_B8R8_2PLANE_420_UNORM,
    vk::Format::G8_B8_R8_3PLANE_422_UNORM,
    vk::Format::G8_B8R8_2PLANE_422_UNORM,
    vk::Format::G8_B8_R8_3PLANE_444_UNORM,
    vk::Format::R10X6_UNORM_PACK16,
    vk::Format::R10X6G10X6_UNORM_2PACK16,
    vk::Format::R10X6G10X6B10X6A10X6_UNORM_4PACK16,
    vk::Format::G10X6B10X6G10X6R10X6_422_UNORM_4PACK16,
    vk::Format::B10X6G10X6R10X6G10X6_422_UNORM_4PACK16,
    vk::Format::G10X6_B10X6_R10X6_3PLANE_420_UNORM_3PACK16,
    vk::Format::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16,
    vk::Format::G10X6_B10X6_R10X6_3PLANE_422_UNORM_3PACK16,
    vk::Format::G10X6_B10X6R10X6_2PLANE_422_UNORM_3PACK16,
    vk::Format::G10X6_B10X6_R10X6_3PLANE_444_UNORM_3PACK16,
    vk::Format::R12X4_UNORM_PACK16,
    vk::Format::R12X4G12X4_UNORM_2PACK16,
    vk::Format::R12X4G12X4B12X4A12X4_UNORM_4PACK16,
    vk::Format::G12X4B12X4G12X4R12X4_422_UNORM_4PACK16,
    vk::Format::B12X4G12X4R12X4G12X4_422_UNORM_4PACK16,
    vk::Format::G12X4_B12X4_R12X4_3PLANE_420_UNORM_3PACK16,
    vk::Format::G12X4_B12X4R12X4_2PLANE_420_UNORM_3PACK16,
    vk::Format::G12X4_B12X4_R12X4_3PLANE_422_UNORM_3PACK16,
    vk::Format::G12X4_B12X4R12X4_2PLANE_422_UNORM_3PACK16,
    vk::Format::G12X4_B12X4_R12X4_3PLANE_444_UNORM_3PACK16,
    vk::Format::G16B16G16R16_422_UNORM,
    vk::Format::B16G16R16G16_422_UNORM,
    vk::Format::G16_B16_R16_3PLANE_420_UNORM,
    vk::Format::G16_B16R16_2PLANE_420_UNORM,
    vk::Format::G16_B16_R16_3PLANE_422_UNORM,
    vk::Format::G16_B16R16_2PLANE_422_UNORM,
    vk::Format::G16_B16_R16_3PLANE_444_UNORM,
    vk::Format::PVRTC1_2BPP_UNORM_BLOCK_IMG,
    vk::Format::PVRTC1_4BPP_UNORM_BLOCK_IMG,
    vk::Format::PVRTC2_2BPP_UNORM_BLOCK_IMG,
    vk::Format::PVRTC2_4BPP_UNORM_BLOCK_IMG,
    vk::Format::PVRTC1_2BPP_SRGB_BLOCK_IMG,
    vk::Format::PVRTC1_4BPP_SRGB_BLOCK_IMG,
    vk::Format::PVRTC2_2BPP_SRGB_BLOCK_IMG,
    vk::Format::PVRTC2_4BPP_SRGB_BLOCK_IMG,
    //    Ash doesn't have these..
    //    vk::Format::G8B8G8R8_422_UNORM_KHR,
    //    vk::Format::B8G8R8G8_422_UNORM_KHR,
    //    vk::Format::G8_B8_R8_3PLANE_420_UNORM_KHR,
    //    vk::Format::G8_B8R8_2PLANE_420_UNORM_KHR,
    //    vk::Format::G8_B8_R8_3PLANE_422_UNORM_KHR,
    //    vk::Format::G8_B8R8_2PLANE_422_UNORM_KHR,
    //    vk::Format::G8_B8_R8_3PLANE_444_UNORM_KHR,
    //    vk::Format::R10X6_UNORM_PACK16_KHR,
    //    vk::Format::R10X6G10X6_UNORM_2PACK16_KHR,
    //    vk::Format::R10X6G10X6B10X6A10X6_UNORM_4PACK16_KHR,
    //    vk::Format::G10X6B10X6G10X6R10X6_422_UNORM_4PACK16_KHR,
    //    vk::Format::B10X6G10X6R10X6G10X6_422_UNORM_4PACK16_KHR,
    //    vk::Format::G10X6_B10X6_R10X6_3PLANE_420_UNORM_3PACK16_KHR,
    //    vk::Format::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16_KHR,
    //    vk::Format::G10X6_B10X6_R10X6_3PLANE_422_UNORM_3PACK16_KHR,
    //    vk::Format::G10X6_B10X6R10X6_2PLANE_422_UNORM_3PACK16_KHR,
    //    vk::Format::G10X6_B10X6_R10X6_3PLANE_444_UNORM_3PACK16_KHR,
    //    vk::Format::R12X4_UNORM_PACK16_KHR,
    //    vk::Format::R12X4G12X4_UNORM_2PACK16_KHR,
    //    vk::Format::R12X4G12X4B12X4A12X4_UNORM_4PACK16_KHR,
    //    vk::Format::G12X4B12X4G12X4R12X4_422_UNORM_4PACK16_KHR,
    //    vk::Format::B12X4G12X4R12X4G12X4_422_UNORM_4PACK16_KHR,
    //    vk::Format::G12X4_B12X4_R12X4_3PLANE_420_UNORM_3PACK16_KHR,
    //    vk::Format::G12X4_B12X4R12X4_2PLANE_420_UNORM_3PACK16_KHR,
    //    vk::Format::G12X4_B12X4_R12X4_3PLANE_422_UNORM_3PACK16_KHR,
    //    vk::Format::G12X4_B12X4R12X4_2PLANE_422_UNORM_3PACK16_KHR,
    //    vk::Format::G12X4_B12X4_R12X4_3PLANE_444_UNORM_3PACK16_KHR,
    //    vk::Format::G16B16G16R16_422_UNORM_KHR,
    //    vk::Format::B16G16R16G16_422_UNORM_KHR,
    //    vk::Format::G16_B16_R16_3PLANE_420_UNORM_KHR,
    //    vk::Format::G16_B16R16_2PLANE_420_UNORM_KHR,
    //    vk::Format::G16_B16_R16_3PLANE_422_UNORM_KHR,
    //    vk::Format::G16_B16R16_2PLANE_422_UNORM_KHR,
    //    vk::Format::G16_B16_R16_3PLANE_444_UNORM_KHR,
];
