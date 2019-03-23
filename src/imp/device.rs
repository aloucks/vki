use ash::extensions::khr;
use ash::version::{DeviceV1_0, InstanceV1_0};
use ash::vk;
use parking_lot::{Mutex, ReentrantMutex, ReentrantMutexGuard};

use crate::error::SurfaceError;
use crate::imp::fenced_deleter::{DeleteWhenUnused, FencedDeleter};
use crate::imp::serial::{Serial, SerialQueue};
use crate::imp::{AdapterInner, DeviceExt, DeviceInner, QueueInner, SurfaceInner};
use crate::{Device, DeviceDescriptor, Limits, Queue, Swapchain, SwapchainDescriptor};

use std::fmt::{self, Debug};
use std::sync::Arc;

#[derive(Default)]
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

    deleter: FencedDeleter,
}

#[derive(Copy, Clone, Debug, Default)]
struct CommandPoolAndBuffer {
    pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
}

impl Device {
    pub(crate) fn new(adapter: Arc<AdapterInner>, descriptor: DeviceDescriptor) -> Result<Device, vk::Result> {
        let inner = DeviceInner::new(adapter, descriptor)?;
        Ok(inner.into())
    }

    pub fn create_swapchain(
        &self,
        descriptor: SwapchainDescriptor,
        old_swapchain: Option<&Swapchain>,
    ) -> Result<Swapchain, SurfaceError> {
        let swapchain = Swapchain::new(self.inner.clone(), descriptor, old_swapchain.map(|s| s.inner.clone()))?;

        self.tick()?;

        Ok(swapchain)
    }

    pub fn get_queue<'a>(&'a self) -> Queue<'a> {
        Queue {
            inner: self.inner.get_queue(),
        }
    }

    pub fn tick(&self) -> Result<(), vk::Result> {
        self.inner.tick()
    }
}

impl DeviceInner {
    fn new(adapter: Arc<AdapterInner>, descriptor: DeviceDescriptor) -> Result<DeviceInner, vk::Result> {
        let extension_names = [c_str!("VK_KHR_swapchain")];

        let surface = descriptor.surface_support.map(|v| v.inner.as_ref());
        let queue_flags = vk::QueueFlags::COMPUTE | vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER;
        let queue_family_index = select_queue_family_index(&adapter, queue_flags, surface)?;

        unsafe {
            assert!(adapter.queue_family_properties[queue_family_index as usize].queue_count > 0);
            let queue_priorities = [1.0];
            let queue_create_infos = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&queue_priorities)
                .build()];

            let create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&extension_names);

            let raw = adapter
                .instance
                .raw
                .create_device(adapter.physical_device, &create_info, None)?;

            let limits = Limits { max_bind_groups: 0 };
            let extensions = descriptor.extensions.clone();

            let queue_index = 0;
            let queue = ReentrantMutex::new(QueueInner {
                handle: raw.get_device_queue(queue_family_index, queue_index),
                queue_index,
                queue_family_index,
            });

            let swapchain = khr::Swapchain::new(&adapter.instance.raw, &raw);
            let raw_ext = DeviceExt { swapchain };

            let mut state = DeviceState::default();
            state.last_submitted_serial = Serial::one();

            let state = Mutex::new(state);

            let inner = DeviceInner {
                raw,
                raw_ext,
                extensions,
                limits,
                adapter,
                queue,
                state,
            };

            Ok(inner)
        }
    }

    pub fn get_queue<'a>(&'a self) -> ReentrantMutexGuard<'a, QueueInner> {
        self.queue.lock()
    }

    pub fn tick(&self) -> Result<(), vk::Result> {
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
            assert_eq!(0, state.fences_in_flight.len());

            // Increment the completed serial to account for any pending deletes
            let serial = state.last_completed_serial.increment();

            state.deleter.tick(serial, self);

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

            log::debug!("destroying device: {:?}", self.raw.handle());
            self.raw.destroy_device(None);
        }
    }
}

impl DeviceState {
    fn tick(&mut self, device: &DeviceInner) -> Result<(), vk::Result> {
        log::trace!("device.tick; unused_commands.len: {}", self.unused_commands.len());
        log::trace!("device.tick; commands_in_flight.len: {}", self.commands_in_flight.len());
        log::trace!("device.tick; fences_in_flight.len: {}", self.fences_in_flight.len());
        log::trace!("device.tick; unused_fences.len: {}", self.unused_fences.len());
        log::trace!("device.tick; wait_semaphores.len: {}", self.wait_semaphores.len());
        self.check_passed_fences(device)?;
        self.recycle_completed_commands(device)?;
        // TODO: maprequest/uploader/allocator ticks
        self.deleter.tick(self.last_completed_serial, device);
        let queue = device.queue.lock();
        self.submit_pending_commands(device, *queue)?;

        Ok(())
    }

    fn check_passed_fences(&mut self, device: &DeviceInner) -> Result<(), vk::Result> {
        let mut last_completed_serial = None;
        let mut result = Ok(());
        for (fence, serial) in self.fences_in_flight.iter().cloned() {
            match unsafe { device.raw.get_fence_status(fence) } {
                Ok(()) => {
                    log::trace!("resetting fence: {:?}, completed_serial: {:?}", fence, serial);
                    unsafe { device.raw.reset_fences(&[fence])? };
                    last_completed_serial = Some(serial);
                }
                Err(vk::Result::NOT_READY) => {
                    break;
                }
                e => {
                    result = e;
                    break;
                }
            }
        }
        if let Some(last_completed_serial) = last_completed_serial {
            for (fence, serial) in self.fences_in_flight.drain_up_to(last_completed_serial) {
                log::trace!("recycling fence: {:?}, completed_serial: {:?}", fence, serial);
                self.unused_fences.push(fence);
            }
            self.last_completed_serial = last_completed_serial;
        }

        log::trace!(
            "last_completed_serial: {:?}, fences_in_flight: {:?}",
            self.last_completed_serial,
            self.fences_in_flight
        );

        result
    }

    fn recycle_completed_commands(&mut self, device: &DeviceInner) -> Result<(), vk::Result> {
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

    pub fn get_pending_command_buffer(&mut self, device: &DeviceInner) -> Result<vk::CommandBuffer, vk::Result> {
        if self.pending_commands.is_none() {
            let pending_commands = self.get_unused_commands(device)?;
            let begin_info = vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
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
        &mut self.deleter
    }

    pub fn submit_pending_commands(&mut self, device: &DeviceInner, queue: QueueInner) -> Result<(), vk::Result> {
        let pending_commands = match self.pending_commands.take() {
            None => {
                // If there are no pending commands and everything in flight has resolved,
                // artificially increment the serials. This allows for pending deletes to
                // resolve even if no new commands have been submitted.
                if self.last_submitted_serial == self.last_completed_serial {
                    self.last_submitted_serial.increment();
                    self.last_completed_serial.increment();
                }
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

        unsafe {
            device.raw.queue_submit(queue.handle, &[*submit_info], fence)?;
        }

        let serial = self.last_submitted_serial.increment();
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
            self.deleter.delete_when_unused(semaphore, next_pending_serial);
        }
        self.wait_semaphores.clear();
    }

    fn get_unused_commands(&mut self, device: &DeviceInner) -> Result<CommandPoolAndBuffer, vk::Result> {
        if !self.unused_commands.is_empty() {
            return self.unused_commands.pop().ok_or_else(|| unreachable!());
        }

        let mut commands = CommandPoolAndBuffer {
            pool: vk::CommandPool::null(),
            command_buffer: vk::CommandBuffer::null(),
        };

        let create_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(device.queue.lock().queue_family_index);

        commands.pool = unsafe { device.raw.create_command_pool(&create_info, None)? };

        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(commands.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let result = unsafe {
            device.raw.fp_v1_0().allocate_command_buffers(
                device.raw.handle(),
                &*allocate_info,
                &mut commands.command_buffer,
            )
        };

        if result != vk::Result::SUCCESS {
            return Err(result);
        }

        Ok(commands)
    }

    fn get_unused_fence(&mut self, device: &DeviceInner) -> Result<vk::Fence, vk::Result> {
        match self.unused_fences.pop() {
            Some(fence) => return Ok(fence),
            None => {
                let create_info = vk::FenceCreateInfo::default();
                let fence = unsafe { device.raw.create_fence(&create_info, None) };
                fence
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
}

/// Recipe: _Selecting a queue family that supports presentation to a given surface_ (page `81`)
///
/// Selects a queue family with the requested `queue_flags` and support for surface presentation.
pub fn select_queue_family_index(
    adapter: &AdapterInner,
    queue_flags: vk::QueueFlags,
    surface: Option<&SurfaceInner>,
) -> Result<u32, vk::Result> {
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

    Err(vk::Result::SUCCESS)
}
