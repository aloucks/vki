use ash::extensions::khr;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk;
use parking_lot::Mutex;
use vk_mem::{Allocator, AllocatorCreateInfo};

use crate::error::Error;

use crate::imp::fenced_deleter::{DeleteWhenUnused, FencedDeleter};
use crate::imp::render_pass::{RenderPassCache, RenderPassCacheQuery};
use crate::imp::serial::{Serial, SerialQueue};
use crate::imp::{swapchain, texture};

use crate::imp::{
    AdapterInner, BindGroupInner, BindGroupLayoutInner, BufferInner, CommandEncoderInner, ComputePipelineInner,
    DeviceExt, DeviceInner, PipelineLayoutInner, QueueInfo, QueueInner, RenderPipelineInner, SamplerInner,
    ShaderModuleInner, SurfaceInner, SwapchainInner, TextureInner,
};

use crate::{
    Adapter, BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, Buffer, BufferDescriptor,
    CommandEncoder, ComputePipeline, ComputePipelineDescriptor, Device, DeviceDescriptor, Limits, MappedBuffer,
    PipelineLayout, PipelineLayoutDescriptor, Queue, RenderPipeline, RenderPipelineDescriptor, Sampler,
    SamplerDescriptor, ShaderModule, ShaderModuleDescriptor, Surface, Swapchain, SwapchainDescriptor, Texture,
    TextureDescriptor, TextureFormat,
};

use std::fmt::{self, Debug};
use std::mem::ManuallyDrop;
use std::sync::Arc;

pub struct DeviceState {
    // the fences in flight for our single queue
    fences_in_flight: SerialQueue<vk::Fence>,

    // commands in flight for our single queue
    commands_in_flight: SerialQueue<CommandPoolAndBuffer>,

    wait_semaphores: Vec<vk::Semaphore>,
    unused_fences: Vec<vk::Fence>,

    last_completed_serial: Serial,
    last_submitted_serial: Serial,

    pending_commands: Option<CommandPoolAndBuffer>,
    unused_commands: Vec<CommandPoolAndBuffer>,

    fenced_deleter: FencedDeleter,

    renderpass_cache: RenderPassCache,
}

#[derive(Copy, Clone, Debug, Default)]
struct CommandPoolAndBuffer {
    pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
}

impl Device {
    pub fn create_swapchain(
        &self,
        descriptor: SwapchainDescriptor,
        old_swapchain: Option<&Swapchain>,
    ) -> Result<Swapchain, Error> {
        let swapchain = SwapchainInner::new(self.inner.clone(), descriptor, old_swapchain.map(|s| &*s.inner))?;

        self.inner.tick()?;

        Ok(swapchain.into())
    }

    pub fn get_supported_swapchain_formats(&self, surface: &Surface) -> Result<Vec<TextureFormat>, Error> {
        let physical_device = self.inner.adapter.physical_device;
        let formats = surface
            .inner
            .get_physical_device_surface_formats(physical_device)?
            .iter()
            .cloned()
            .filter(|format| format.color_space == swapchain::COLOR_SPACE)
            .map(|format| texture::texture_format(format.format))
            .collect();

        Ok(formats)
    }

    pub fn get_queue(&self) -> Queue {
        Queue {
            inner: QueueInner {
                device: Arc::clone(&self.inner),
                queue: self.inner.queue,
            },
        }
    }

    pub fn adapter(&self) -> Adapter {
        Adapter {
            inner: Arc::clone(&self.inner.adapter),
        }
    }

    pub fn create_buffer(&self, descriptor: BufferDescriptor) -> Result<Buffer, Error> {
        let buffer = BufferInner::new(self.inner.clone(), descriptor)?;
        Ok(buffer.into())
    }

    pub fn create_buffer_mapped(&self, descriptor: BufferDescriptor) -> Result<MappedBuffer, Error> {
        let buffer = BufferInner::new(self.inner.clone(), descriptor)?;
        let data = unsafe { buffer.get_mapped_ptr()? };
        Ok(MappedBuffer {
            inner: Arc::new(buffer),
            data,
        })
    }

    pub fn create_texture(&self, descriptor: TextureDescriptor) -> Result<Texture, Error> {
        let texture = TextureInner::new(self.inner.clone(), descriptor)?;
        Ok(texture.into())
    }

    pub fn create_sampler(&self, descriptor: SamplerDescriptor) -> Result<Sampler, Error> {
        let sampler = SamplerInner::new(self.inner.clone(), descriptor)?;
        Ok(sampler.into())
    }

    pub fn create_bind_group_layout(&self, descriptor: BindGroupLayoutDescriptor) -> Result<BindGroupLayout, Error> {
        let bind_group_layout = BindGroupLayoutInner::new(self.inner.clone(), descriptor)?;
        Ok(bind_group_layout.into())
    }

    pub fn create_bind_group(&self, descriptor: BindGroupDescriptor) -> Result<BindGroup, Error> {
        let bind_group = BindGroupInner::new(descriptor)?;
        Ok(bind_group.into())
    }

    pub fn create_shader_module(&self, descriptor: ShaderModuleDescriptor) -> Result<ShaderModule, Error> {
        let shader_module = ShaderModuleInner::new(self.inner.clone(), descriptor)?;
        Ok(shader_module.into())
    }

    pub fn create_pipeline_layout(&self, descriptor: PipelineLayoutDescriptor) -> Result<PipelineLayout, Error> {
        let pipeline_layout = PipelineLayoutInner::new(self.inner.clone(), descriptor)?;
        Ok(pipeline_layout.into())
    }

    pub fn create_compute_pipeline(&self, descriptor: ComputePipelineDescriptor) -> Result<ComputePipeline, Error> {
        let compute_pipeline = ComputePipelineInner::new(self.inner.clone(), descriptor)?;
        Ok(compute_pipeline.into())
    }

    pub fn create_render_pipeline(&self, descriptor: RenderPipelineDescriptor) -> Result<RenderPipeline, Error> {
        let render_pipeline = RenderPipelineInner::new(self.inner.clone(), descriptor)?;
        Ok(render_pipeline.into())
    }

    pub fn create_command_encoder(&self) -> Result<CommandEncoder, Error> {
        let command_encoder = CommandEncoderInner::new(self.inner.clone())?;
        Ok(command_encoder.into())
    }
}

impl DeviceInner {
    pub fn new(adapter: Arc<AdapterInner>, descriptor: DeviceDescriptor) -> Result<DeviceInner, Error> {
        log::info!("requesting device from adapter: {}", adapter.name);
        let extension_names = if descriptor.surface_support.is_some() {
            vec![c_str!("VK_KHR_swapchain")]
        } else {
            vec![]
        };

        for name in extension_names.iter() {
            let name = unsafe { std::ffi::CStr::from_ptr(*name).to_string_lossy() };
            log::info!("requesting device extension: {}", name);
        }

        let surface = descriptor.surface_support.map(|v| v.inner.as_ref());
        let queue_flags = vk::QueueFlags::COMPUTE | vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER;
        let queue_family_index = select_queue_family_index(&adapter, queue_flags, surface)?;

        unsafe {
            assert!(adapter.queue_family_properties[queue_family_index as usize].queue_count > 0);
            let features = vk::PhysicalDeviceFeatures::builder()
                .fill_mode_non_solid(adapter.physical_device_features.fill_mode_non_solid > 0)
                .build();
            let queue_priorities = [1.0];
            let queue_create_infos = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&queue_priorities)
                .build()];

            let create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_create_infos)
                .enabled_features(&features)
                .enabled_extension_names(&extension_names);

            let raw = adapter
                .instance
                .raw
                .create_device(adapter.physical_device, &create_info, None)?;

            let limits = Limits { max_bind_groups: 0 };
            let extensions = descriptor.extensions.clone();

            let queue_index = 0;
            let queue = QueueInfo {
                handle: raw.get_device_queue(queue_family_index, queue_index),
                queue_index,
                queue_family_index,
            };

            let swapchain = khr::Swapchain::new(&adapter.instance.raw, &raw);
            let raw_ext = DeviceExt { swapchain };

            let allocator_create_info = AllocatorCreateInfo {
                device: raw.clone(),
                instance: adapter.instance.raw.clone(),
                physical_device: adapter.physical_device,
                ..Default::default()
            };

            // TODO: Add generic error type
            let allocator = Allocator::new(&allocator_create_info).expect("Allocator creation failed");

            let state = DeviceState {
                fences_in_flight: SerialQueue::default(),
                commands_in_flight: SerialQueue::default(),
                wait_semaphores: Vec::default(),
                unused_fences: Vec::default(),
                last_completed_serial: Serial::zero(),
                last_submitted_serial: Serial::one(),
                pending_commands: None,
                unused_commands: Vec::new(),
                fenced_deleter: FencedDeleter::default(),
                renderpass_cache: RenderPassCache::default(),
            };

            let state = Mutex::new(state);

            let inner = DeviceInner {
                raw,
                raw_ext,
                extensions,
                limits,
                adapter,
                queue,
                state,
                allocator: ManuallyDrop::new(allocator),
            };

            Ok(inner)
        }
    }

    pub fn tick(&self) -> Result<(), Error> {
        let mut state = self.state.lock();
        state.tick(self)?;
        Ok(())
    }
}

impl Debug for DeviceInner {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self.raw.handle())
    }
}

impl Debug for Device {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Device")
            .field("handle", &self.inner.raw.handle())
            .finish()
    }
}

impl Into<Device> for DeviceInner {
    fn into(self) -> Device {
        Device { inner: Arc::new(self) }
    }
}

impl Drop for DeviceInner {
    fn drop(&mut self) {
        unsafe {
            let mut state = self.state.lock();

            state
                .tick(self)
                .map_err(|e| log::error!("device.drop; tick: {:?}", e))
                .ok();

            self.raw
                .device_wait_idle()
                .map_err(|e| log::error!("device.drop; device_wait_idle: {:?}", e))
                .ok();

            while !state.fences_in_flight.is_empty() {
                state
                    .check_passed_fences(self)
                    .map_err(|e| log::error!("device.drop; check_passed_fences: {:?}", e))
                    .ok();
                std::thread::yield_now();
            }

            // All fences should be complete after waiting for the device to become idle
            if !std::thread::panicking() {
                assert_eq!(0, state.fences_in_flight.len());
            }

            // Increment the completed serial to account for any pending deletes
            log::trace!("drop: setting last_completed_serial to last_submitted_serial and incrementing");
            state.last_completed_serial = state.last_submitted_serial;
            let serial = state.last_completed_serial.increment();
            log::trace!("drop: last_completed_serial: {:?}", serial);

            // Work-around for a weird borrow issue with the mutex guard auto-deref
            {
                let state = &mut *state;
                state.fenced_deleter.tick(serial, &self, &self.allocator);
                if !std::thread::panicking() {
                    assert!(state.fenced_deleter.is_empty());
                }
            }

            for (fence, _) in state.fences_in_flight.drain(..) {
                self.raw.destroy_fence(fence, None);
            }
            for fence in state.unused_fences.drain(..) {
                self.raw.destroy_fence(fence, None);
            }

            for (commands, _) in state.commands_in_flight.drain(..) {
                self.raw.destroy_command_pool(commands.pool, None);
            }
            for commands in state.unused_commands.drain(..) {
                self.raw.destroy_command_pool(commands.pool, None);
            }
            if let Some(commands) = state.pending_commands.take() {
                self.raw.destroy_command_pool(commands.pool, None);
            }

            for semaphore in state.wait_semaphores.drain(..) {
                self.raw.destroy_semaphore(semaphore, None);
            }

            state.renderpass_cache.drain(&self);

            ManuallyDrop::drop(&mut self.allocator);

            drop(state);

            log::debug!("destroying device: {:?}", self.raw.handle());
            self.raw.destroy_device(None);
        }
    }
}

impl DeviceState {
    fn tick(&mut self, device: &DeviceInner) -> Result<(), Error> {
        if log::log_enabled!(log::Level::Trace) {
            log::trace!("device.tick; last_submitted_serial: {:?}", self.last_submitted_serial);
            log::trace!("device.tick; last_completed_serial: {:?}", self.last_completed_serial);
            log::trace!("device.tick; unused_commands.len: {}", self.unused_commands.len());
            log::trace!("device.tick; commands_in_flight.len: {}", self.commands_in_flight.len());
            log::trace!("device.tick; fences_in_flight.len: {}", self.fences_in_flight.len());
            log::trace!("device.tick; unused_fences.len: {}", self.unused_fences.len());
            log::trace!("device.tick; wait_semaphores.len: {}", self.wait_semaphores.len());
        }
        self.check_passed_fences(device)?;
        self.recycle_completed_commands(device)?;
        // TODO: maprequest/uploader/allocator ticks
        self.fenced_deleter
            .tick(self.last_completed_serial, device, &device.allocator);
        let queue = &device.queue;
        self.submit_pending_commands(device, &queue)?;

        Ok(())
    }

    fn check_passed_fences(&mut self, device: &DeviceInner) -> Result<(), Error> {
        let mut last_completed_serial = None;
        let mut result = Ok(());
        for (fence, serial) in self.fences_in_flight.iter().cloned() {
            match unsafe { device.raw.get_fence_status(fence) } {
                Ok(true) => {
                    last_completed_serial = Some(serial);
                }
                Ok(false) | Err(vk::Result::NOT_READY) => {
                    break;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        if let Some(last_completed_serial) = last_completed_serial {
            let index = self.unused_fences.len();
            for (fence, serial) in self.fences_in_flight.drain_up_to(last_completed_serial) {
                log::trace!("recycling fence: {:?}, completed_serial: {:?}", fence, serial);
                self.unused_fences.push(fence);
            }
            let fences = &self.unused_fences[index..];
            log::trace!("resetting fences: {:?}", fences);
            unsafe {
                device.raw.reset_fences(fences)?;
            }
            self.last_completed_serial = last_completed_serial;
        }

        log::trace!(
            "last_completed_serial: {:?}, fences_in_flight: {:?}",
            self.last_completed_serial,
            self.fences_in_flight
        );

        Ok(result?)
    }

    fn recycle_completed_commands(&mut self, device: &DeviceInner) -> Result<(), Error> {
        let serial = self.last_completed_serial;
        for (commands, serial) in self.commands_in_flight.drain_up_to(serial) {
            unsafe {
                log::trace!("recycled command_pool: {:?}, serial: {:?}", commands.pool, serial);
                device
                    .raw
                    .reset_command_pool(commands.pool, vk::CommandPoolResetFlags::empty())?;
                self.unused_commands.push(commands);
            }
        }

        Ok(())
    }

    pub fn get_pending_command_buffer(&mut self, device: &DeviceInner) -> Result<vk::CommandBuffer, Error> {
        if self.pending_commands.is_none() {
            let pending_commands = self.get_unused_commands(device)?;
            let begin_info = vk::CommandBufferBeginInfo {
                flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
                ..Default::default()
            };
            unsafe {
                device
                    .raw
                    .begin_command_buffer(pending_commands.command_buffer, &begin_info)?;
            }
            self.pending_commands = Some(pending_commands);
        }

        self.pending_commands
            .map(|p| p.command_buffer)
            .ok_or_else(|| unreachable!())
    }

    pub fn get_fenced_deleter(&mut self) -> &mut FencedDeleter {
        &mut self.fenced_deleter
    }

    pub fn submit_pending_commands(&mut self, device: &DeviceInner, queue: &QueueInfo) -> Result<(), Error> {
        let pending_commands = match self.pending_commands.take() {
            None => {
                // NOTE: This no longer seems to be necessary (Device::drop had some changes),
                //       and it breaks the Fence implementation.
                // // If there are no pending commands and everything in flight has resolved,
                // // artificially increment the serials. This allows for pending deletes to
                // // resolve even if no new commands have been submitted.
                // if self.last_submitted_serial == self.last_completed_serial {
                //     self.last_submitted_serial.increment();
                //     self.last_completed_serial.increment();
                //     log::trace!(
                //         "all commands complete: incremented serials: {:?}",
                //         self.last_submitted_serial
                //     );
                // }
                return Ok(());
            }
            Some(pending_commands) => pending_commands,
        };

        unsafe {
            device.raw.end_command_buffer(pending_commands.command_buffer)?;
        }

        let wait_dst_stage_masks = vec![vk::PipelineStageFlags::ALL_COMMANDS; self.wait_semaphores.len()];
        let pending_command_buffers = [pending_commands.command_buffer];

        let fence = self.get_unused_fence(device)?;

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&self.wait_semaphores)
            .wait_dst_stage_mask(&wait_dst_stage_masks)
            .command_buffers(&pending_command_buffers);

        let serial = self.last_submitted_serial.increment();

        log::trace!("queue_submit: {:?}", self.last_submitted_serial);
        unsafe {
            device.raw.queue_submit(queue.handle, &[*submit_info], fence)?;
        }

        self.fences_in_flight.enqueue(fence, serial);
        self.commands_in_flight.enqueue(pending_commands, serial);

        self.delete_when_unused_wait_semaphores();

        Ok(())
    }

    pub fn get_wait_semaphores(&self) -> &[vk::Semaphore] {
        &self.wait_semaphores
    }

    // only exposed to allow for presentation to check for wait semaphores
    pub fn delete_when_unused_wait_semaphores(&mut self) {
        let next_pending_serial = self.get_next_pending_serial();
        for semaphore in self.wait_semaphores.iter().cloned() {
            self.fenced_deleter.delete_when_unused(semaphore, next_pending_serial);
        }
        self.wait_semaphores.clear();
    }

    fn get_unused_commands(&mut self, device: &DeviceInner) -> Result<CommandPoolAndBuffer, Error> {
        if !self.unused_commands.is_empty() {
            return self.unused_commands.pop().ok_or_else(|| unreachable!());
        }

        let mut commands = CommandPoolAndBuffer {
            pool: vk::CommandPool::null(),
            command_buffer: vk::CommandBuffer::null(),
        };

        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::TRANSIENT,
            queue_family_index: device.queue.queue_family_index,
            ..Default::default()
        };

        commands.pool = unsafe { device.raw.create_command_pool(&create_info, None)? };

        let allocate_info = vk::CommandBufferAllocateInfo {
            command_pool: commands.pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
            ..Default::default()
        };

        let result = unsafe {
            device.raw.fp_v1_0().allocate_command_buffers(
                device.raw.handle(),
                &allocate_info,
                &mut commands.command_buffer,
            )
        };

        if result != vk::Result::SUCCESS {
            return Err(Error::from(result));
        }

        Ok(commands)
    }

    fn get_unused_fence(&mut self, device: &DeviceInner) -> Result<vk::Fence, Error> {
        match self.unused_fences.pop() {
            Some(fence) => Ok(fence),
            None => {
                let create_info = vk::FenceCreateInfo::default();
                let fence = unsafe { device.raw.create_fence(&create_info, None)? };
                Ok(fence)
            }
        }
    }

    pub fn add_wait_semaphore(&mut self, semaphore: vk::Semaphore) {
        self.wait_semaphores.push(semaphore)
    }

    pub fn get_last_submitted_serial(&self) -> Serial {
        self.last_submitted_serial
    }

    pub fn get_last_completed_serial(&self) -> Serial {
        self.last_completed_serial
    }

    pub fn get_next_pending_serial(&self) -> Serial {
        self.last_submitted_serial.next()
    }

    pub fn get_render_pass(
        &mut self,
        query: RenderPassCacheQuery,
        device: &DeviceInner,
    ) -> Result<vk::RenderPass, Error> {
        self.renderpass_cache.get_render_pass(query, device)
    }
}

/// Recipe: _Selecting a queue family that supports presentation to a given surface_ (page `81`)
///
/// Selects a queue family with the requested `queue_flags` and support for surface presentation.
pub fn select_queue_family_index(
    adapter: &AdapterInner,
    queue_flags: vk::QueueFlags,
    surface: Option<&SurfaceInner>,
) -> Result<u32, Error> {
    for (queue_family_index, queue_family) in adapter.queue_family_properties.iter().enumerate() {
        let queue_family_index = queue_family_index as u32;
        if let Some(surface) = surface {
            if !adapter.get_surface_support(surface, queue_family_index)? {
                continue;
            }
        }
        if queue_family.queue_flags.contains(queue_flags) && queue_family.queue_count > 0 {
            return Ok(queue_family_index);
        }
    }

    Err(Error::from(vk::Result::ERROR_INCOMPATIBLE_DISPLAY_KHR))
}
