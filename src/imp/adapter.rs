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
                            return Ok(inner.into());
                        }
                    }
                }
                PowerPreference::LowPower => {
                    for (index, properties) in physical_device_properties.iter().enumerate() {
                        if properties.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
                            let inner = new_inner(instance.clone(), physical_devices[index], *properties)?;
                            return Ok(inner.into());
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
            .field("physical_device", &self.inner.physical_device)
            .field("name", &self.inner.name)
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
