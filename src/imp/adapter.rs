use crate::imp::{AdapterInner, DeviceInner, InstanceInner, SurfaceInner};
use crate::{Adapter, AdapterOptions, Device, DeviceDescriptor, Extensions, PowerPreference};

use crate::error::Error;

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

    pub fn properties(&self) -> AdapterProperties {
        self.inner.properties()
    }

    pub fn create_device(&self, descriptor: DeviceDescriptor) -> Result<Device, Error> {
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
    fn new(instance: &Arc<InstanceInner>, physical_device: vk::PhysicalDevice) -> Result<AdapterInner, Error> {
        let instance = Arc::clone(instance);
        let (name, extensions, physical_device_features, physical_device_properties) = unsafe {
            let physical_device_properties = instance.raw.get_physical_device_properties(physical_device);

            let name = CStr::from_ptr(physical_device_properties.device_name.as_ptr())
                .to_string_lossy()
                .into_owned();

            let api_version = version(physical_device_properties.api_version);
            let driver_version = version(physical_device_properties.driver_version);

            log::info!(
                "found physical device: {:?} {:?} {:?} {:?} {} {}",
                name,
                physical_device_properties.device_type,
                api_version,
                driver_version,
                physical_device_properties.device_id,
                physical_device_properties.vendor_id,
            );

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
            let ret = instance.raw.fp_v1_0().enumerate_device_layer_properties(
                physical_device,
                &mut num_layers,
                std::ptr::null_mut(),
            );
            if ret != vk::Result::SUCCESS {
                Err(Error::from(ret))?;
            }
            let mut layers = vec![vk::LayerProperties::default(); num_layers as usize];
            let ret = instance.raw.fp_v1_0().enumerate_device_layer_properties(
                physical_device,
                &mut num_layers,
                layers.as_mut_ptr(),
            );
            if ret != vk::Result::SUCCESS {
                Err(Error::from(ret))?;
            }
            for p in layers.iter() {
                let name = CStr::from_ptr(p.layer_name.as_ptr());
                log::debug!("found physical device layer: {}", name.to_string_lossy());
            }

            let physical_device_features = instance.raw.get_physical_device_features(physical_device);
            let extensions = Extensions {
                anisotropic_filtering: physical_device_features.sampler_anisotropy == vk::TRUE,
            };
            (name, extensions, physical_device_features, physical_device_properties)
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

    pub fn enumerate(instance: &Arc<InstanceInner>) -> Result<Vec<AdapterInner>, Error> {
        unsafe {
            let physical_devices = match instance.raw.enumerate_physical_devices() {
                Ok(physical_devices) => physical_devices,
                Err(e) => {
                    log::error!("failed to enumerate physical devices: {:?}", e);
                    return Err(Error::from(e))?;
                }
            };
            let mut adapters = Vec::with_capacity(physical_devices.len());
            for physical_device in physical_devices.iter() {
                adapters.push(AdapterInner::new(instance, *physical_device)?);
            }
            Ok(adapters)
        }
    }

    pub fn request(instance: Arc<InstanceInner>, options: AdapterOptions) -> Result<AdapterInner, Error> {
        let mut adapters = AdapterInner::enumerate(&instance)?;
        if adapters.is_empty() {
            return Err(Error::from("No adapters were found"));
        }
        let mut index = 0;
        for (i, adapter) in adapters.iter().enumerate() {
            let device_type = adapter.physical_device_properties.device_type;
            match options.power_preference {
                PowerPreference::HighPerformance => {
                    if device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                        index = i;
                        log::info!("power preference match: {:?}", options.power_preference);
                        break;
                    }
                }
                PowerPreference::LowPower => {
                    if device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
                        index = i;
                        log::info!("power preference match: {:?}", options.power_preference);
                        break;
                    }
                }
            }
        }
        Ok(adapters.remove(index))
    }

    pub fn get_surface_support(&self, surface: &SurfaceInner, queue_index: u32) -> Result<bool, Error> {
        unsafe {
            // self.instance
            //    .raw_ext
            //    .surface
            //    .get_physical_device_surface_support(self.physical_device, queue_index as u32, surface.handle)
            //    .map_err(Error::from)

            // TODO: ash has a breaking change in the return type for `get_physical_device_surface_support`.
            //       The section above is for the current master and the unreleased 0.30+.
            //       The section below is compatible with both 0.29 and the current master.
            enum Compat {
                Old(bool),
                New(Result<bool, vk::Result>),
            }
            impl From<bool> for Compat {
                fn from(value: bool) -> Compat {
                    Compat::Old(value)
                }
            }
            impl From<Result<bool, vk::Result>> for Compat {
                fn from(value: Result<bool, vk::Result>) -> Compat {
                    Compat::New(value)
                }
            }
            impl Into<Result<bool, Error>> for Compat {
                fn into(self) -> Result<bool, Error> {
                    match self {
                        Compat::Old(value) => Ok(value),
                        Compat::New(value) => Ok(value?),
                    }
                }
            }
            let supported = self.instance.raw_ext.surface.get_physical_device_surface_support(
                self.physical_device,
                queue_index as u32,
                surface.handle,
            );
            Compat::from(supported).into()
        }
    }

    pub fn properties(&self) -> AdapterProperties {
        let device_name = unsafe {
            std::ffi::CStr::from_ptr(self.physical_device_properties.device_name.as_ptr())
                .to_str()
                .unwrap_or("<unknown>")
        };
        AdapterProperties {
            api_version: version(self.physical_device_properties.api_version),
            driver_version: self.physical_device_properties.driver_version,
            vender_id: self.physical_device_properties.vendor_id,
            device_id: self.physical_device_properties.device_id,
            device_type: self.physical_device_properties.device_type,
            device_name,
            limits: self.physical_device_properties.limits,
        }
    }
}

fn version(v: u32) -> (u32, u32, u32) {
    (vk::api_version_major(v), vk::api_version_minor(v), vk::api_version_patch(v))
}

#[derive(Debug, Copy, Clone)]
pub struct AdapterProperties<'a> {
    pub device_name: &'a str,
    pub api_version: (u32, u32, u32),
    pub driver_version: u32,
    pub vender_id: u32,
    pub device_id: u32,
    pub device_type: vk::PhysicalDeviceType,
    pub limits: vk::PhysicalDeviceLimits,
}

impl<'a> AdapterProperties<'a> {
    pub fn driver_version_string(&self) -> String {
        let v = self.driver_version;
        if self.vender_id == 4318 {
            // NVIDIA
            let major = (v >> 22) & 0x3ff;
            let minor = (v >> 14) & 0x0ff;
            let patch = (v >> 6) & 0x0ff;
            let rev = (v >> 6) & 0x003f;
            format!("{}.{}.{}.{}", major, minor, patch, rev)
        } else if self.vender_id == 32902 {
            // INTEL (windows only?)
            let major = (v >> 14) & 0x3ff;
            let minor = v & 0x3ff;
            format!("{}.{}", major, minor)
        } else {
            let (major, minor, patch) = version(v);
            format!("{}.{}.{}", major, minor, patch)
        }
    }
}

impl Debug for Adapter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Adapter")
            .field("name", &self.inner.name)
            .field("physical_device", &self.inner.physical_device)
            .field("physical_device_properties", &self.inner.physical_device_properties)
            .field("physical_device_features", &self.inner.physical_device_features)
            .finish()
    }
}

impl AdapterOptions {
    pub fn new() -> AdapterOptions {
        AdapterOptions::default()
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
