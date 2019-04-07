use ash::version::DeviceV1_0;
use ash::vk;
use ash::vk::{DependencyFlags, MemoryPropertyFlags};

use vk_mem::{AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::imp::{BufferInner, BufferState, DeviceInner};
use crate::{Buffer, BufferDescriptor, BufferUsageFlags, MappedBuffer};

use crate::imp::fenced_deleter::DeleteWhenUnused;
use parking_lot::Mutex;
use std::sync::Arc;

pub fn read_only_buffer_usages() -> BufferUsageFlags {
    BufferUsageFlags::MAP_READ
        | BufferUsageFlags::TRANSFER_SRC
        | BufferUsageFlags::INDEX
        | BufferUsageFlags::VERTEX
        | BufferUsageFlags::UNIFORM
}

pub fn writable_buffer_usages() -> BufferUsageFlags {
    BufferUsageFlags::MAP_WRITE | BufferUsageFlags::TRANSFER_DST | BufferUsageFlags::STORAGE
}

pub fn memory_usage(usage: BufferUsageFlags) -> MemoryUsage {
    assert_ne!(usage, BufferUsageFlags::NONE, "BufferUsageFlags may not be NONE");

    if usage.intersects(BufferUsageFlags::MAP_WRITE | BufferUsageFlags::TRANSFER_SRC) {
        return MemoryUsage::CpuOnly;
    }

    if usage.intersects(BufferUsageFlags::MAP_READ | BufferUsageFlags::TRANSFER_DST) {
        return MemoryUsage::CpuOnly;
    }

    if usage.intersects(BufferUsageFlags::MAP_WRITE) {
        return MemoryUsage::CpuToGpu;
    }

    if usage.intersects(BufferUsageFlags::MAP_READ) {
        return MemoryUsage::GpuToCpu;
    }

    MemoryUsage::GpuOnly
}

pub fn usage_flags(usage: BufferUsageFlags) -> vk::BufferUsageFlags {
    let mut flags = vk::BufferUsageFlags::empty();

    if usage.intersects(BufferUsageFlags::TRANSFER_SRC) {
        flags |= vk::BufferUsageFlags::TRANSFER_SRC;
    }

    if usage.intersects(BufferUsageFlags::TRANSFER_DST) {
        flags |= vk::BufferUsageFlags::TRANSFER_DST;
    }

    if usage.intersects(BufferUsageFlags::INDEX) {
        flags |= vk::BufferUsageFlags::INDEX_BUFFER;
    }

    if usage.intersects(BufferUsageFlags::VERTEX) {
        flags |= vk::BufferUsageFlags::VERTEX_BUFFER;
    }

    if usage.intersects(BufferUsageFlags::UNIFORM) {
        flags |= vk::BufferUsageFlags::UNIFORM_BUFFER;
    }

    if usage.intersects(BufferUsageFlags::STORAGE) {
        flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
    }

    flags
}

pub fn pipeline_stage(usage: BufferUsageFlags) -> vk::PipelineStageFlags {
    let mut flags = vk::PipelineStageFlags::empty();

    if usage.intersects(BufferUsageFlags::MAP_READ | BufferUsageFlags::MAP_WRITE) {
        flags |= vk::PipelineStageFlags::HOST;
    }

    if usage.intersects(BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::TRANSFER_DST) {
        flags |= vk::PipelineStageFlags::TRANSFER;
    }

    if usage.intersects(BufferUsageFlags::INDEX | BufferUsageFlags::VERTEX) {
        flags |= vk::PipelineStageFlags::VERTEX_INPUT;
    }

    if usage.intersects(BufferUsageFlags::UNIFORM | BufferUsageFlags::STORAGE) {
        flags |= vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
    }

    flags
}

pub fn access_flags(usage: BufferUsageFlags) -> vk::AccessFlags {
    let mut flags = vk::AccessFlags::empty();

    if usage.intersects(BufferUsageFlags::MAP_READ) {
        flags |= vk::AccessFlags::HOST_READ
    }

    if usage.intersects(BufferUsageFlags::MAP_WRITE) {
        flags |= vk::AccessFlags::HOST_WRITE
    }

    if usage.intersects(BufferUsageFlags::TRANSFER_SRC) {
        flags |= vk::AccessFlags::TRANSFER_READ
    }

    if usage.intersects(BufferUsageFlags::TRANSFER_DST) {
        flags |= vk::AccessFlags::TRANSFER_WRITE
    }

    if usage.intersects(BufferUsageFlags::INDEX) {
        flags |= vk::AccessFlags::INDEX_READ
    }

    if usage.intersects(BufferUsageFlags::VERTEX) {
        flags |= vk::AccessFlags::VERTEX_ATTRIBUTE_READ
    }

    if usage.intersects(BufferUsageFlags::UNIFORM) {
        flags |= vk::AccessFlags::UNIFORM_READ
    }

    if usage.intersects(BufferUsageFlags::STORAGE) {
        flags |= vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE
    }

    flags
}

impl BufferInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: BufferDescriptor) -> Result<BufferInner, vk::Result> {
        let create_info = vk::BufferCreateInfo {
            size: descriptor.size,
            usage: usage_flags(descriptor.usage),
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let allocation_create_info = AllocationCreateInfo {
            usage: memory_usage(descriptor.usage),
            preferred_flags: MemoryPropertyFlags::empty(),
            required_flags: MemoryPropertyFlags::empty(),
            flags: AllocationCreateFlags::NONE,
            user_data: None,
            pool: None,
            memory_type_bits: 0,
        };

        log::trace!(
            "buffer create_info: {:?}, allocation_create_info: {:?}",
            create_info,
            allocation_create_info
        );

        let mut state = device.state.lock();
        let allocator = state.allocator_mut();

        let result = allocator.create_buffer(&create_info, &allocation_create_info);

        // See TextureInner::new
        if let Err(ref e) = &result {
            if let vk_mem::ErrorKind::Vulkan(vk::Result::ERROR_VALIDATION_FAILED_EXT) = e.kind() {
                unsafe {
                    let dummy = device.raw.create_buffer(&create_info, None)?;
                    device.raw.destroy_buffer(dummy, None);
                    return Err(vk::Result::ERROR_VALIDATION_FAILED_EXT);
                }
            }
        }

        let (buffer, allocation, allocation_info) = result.expect("failed to create buffer"); // TODO

        log::trace!("created buffer: {:?}, allocation_info: {:?}", buffer, allocation_info);

        drop(state);

        Ok(BufferInner {
            descriptor,
            allocation,
            allocation_info,
            device,
            last_usage: Mutex::new(BufferUsageFlags::NONE),
            buffer_state: Mutex::new(BufferState::Unmapped),
            handle: buffer,
        })
    }

    pub fn transition_usage_now(
        &self,
        command_buffer: vk::CommandBuffer,
        usage: BufferUsageFlags,
    ) -> Result<(), vk::Result> {
        let mut last_usage = self.last_usage.lock();
        let last_includes_target = (*last_usage & usage) == usage;
        let last_read_only = (*last_usage & read_only_buffer_usages()) == *last_usage;

        if last_includes_target && last_read_only {
            return Ok(());
        }

        // initial transition
        if *last_usage == BufferUsageFlags::NONE {
            *last_usage = usage;
            return Ok(());
        }

        let src_stage_mask = pipeline_stage(*last_usage);
        let dst_stage_mask = pipeline_stage(usage);

        let src_access_mask = access_flags(*last_usage);
        let dst_access_mask = access_flags(usage);

        log::trace!(
            "usage: {:?}, last_usage: {:?}, src_stage_mask: {}, src_access_mask: {}",
            usage,
            *last_usage,
            src_stage_mask,
            src_access_mask
        );
        log::trace!(
            "usage: {:?}, last_usage: {:?}, dst_stage_mask: {}, dst_access_mask: {}",
            usage,
            *last_usage,
            dst_stage_mask,
            dst_access_mask
        );

        let buffer_memory_barrier = vk::BufferMemoryBarrier {
            src_access_mask,
            dst_access_mask,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            buffer: self.handle,
            offset: 0,
            size: self.descriptor.size,
            ..Default::default()
        };

        let dependency_flags = DependencyFlags::empty();
        let memory_barriers = &[];
        let buffer_memory_barriers = &[buffer_memory_barrier];
        let image_memory_barriers = &[];

        unsafe {
            self.device.raw.cmd_pipeline_barrier(
                command_buffer,
                src_stage_mask,
                dst_stage_mask,
                dependency_flags,
                memory_barriers,
                buffer_memory_barriers,
                image_memory_barriers,
            );
        }

        Ok(())
    }

    pub fn get_mapped_ptr(&self) -> Result<*mut u8, vk::Result> {
        let mut buffer_state = self.buffer_state.lock();
        match *buffer_state {
            BufferState::Mapped(ptr) => Ok(ptr),
            BufferState::Unmapped => {
                let mut state = self.device.state.lock();
                let ptr = state.allocator_mut().map_memory(&self.allocation).map_err(|e| {
                    log::error!("failed to map buffer: {:?}", e);
                    vk::Result::ERROR_VALIDATION_FAILED_EXT // TODO
                })?;
                *buffer_state = BufferState::Mapped(ptr);
                Ok(ptr)
            }
        }
    }

    fn unmap(&self) -> Result<(), vk::Result> {
        let mut buffer_state = self.buffer_state.lock();
        match *buffer_state {
            BufferState::Mapped(_) => {
                let mut state = self.device.state.lock();
                state.allocator_mut().unmap_memory(&self.allocation).map_err(|e| {
                    log::error!("failed to unmap buffer: {:?}", e);
                    vk::Result::ERROR_VALIDATION_FAILED_EXT // TODO
                })?;
                *buffer_state = BufferState::Unmapped;
                Ok(())
            }
            BufferState::Unmapped => Ok(()),
        }
    }
}

impl Into<Buffer> for BufferInner {
    fn into(self) -> Buffer {
        Buffer { inner: Arc::new(self) }
    }
}

impl Drop for BufferInner {
    fn drop(&mut self) {
        self.unmap()
            .map_err(|e| log::warn!("failed to unmap_memory: {:?}", e))
            .ok();
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state
            .get_fenced_deleter()
            .delete_when_unused((self.handle, self.allocation.clone()), serial);
    }
}

impl MappedBuffer {
    pub fn write<T: Copy>(&self, offset: usize, data: &[T]) -> Result<(), vk::Result> {
        use std::{mem, ptr};
        let count = data.len();
        let data_size = mem::size_of::<T>() * count;
        let buffer_size = self.inner.allocation_info.get_size();
        debug_assert!(
            self.inner.descriptor.usage.intersects(BufferUsageFlags::MAP_WRITE),
            "buffer not write mapped"
        );
        debug_assert!(offset + data_size <= buffer_size, "write data exceeds buffer size");
        log::trace!(
            "map write data_size: {}, offset: {}, buffer_size: {}",
            data_size,
            offset,
            buffer_size
        );
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr() as *const u8, self.data, data_size);
            self.inner
                .device
                .state
                .lock()
                .allocator_mut()
                .flush_allocation(&self.inner.allocation, offset, data_size)
                .map_err(|e| {
                    log::error!("failed to flush allocation: {:?}", e);
                    vk::Result::ERROR_VALIDATION_FAILED_EXT // TODO
                })
        }
    }

    pub fn read<T: Copy>(&self, offset: usize, count: usize) -> Result<&[T], vk::Result> {
        use std::{mem, slice};
        let data_size = mem::size_of::<T>() * count;
        let buffer_size = self.inner.allocation_info.get_size();
        debug_assert!(
            self.inner.descriptor.usage.intersects(BufferUsageFlags::MAP_READ),
            "buffer not read mapped"
        );
        debug_assert!(offset + data_size <= buffer_size, "read data exceeds buffer size");
        log::trace!(
            "map read data_size: {}, offset: {}, buffer_size: {}",
            data_size,
            offset,
            buffer_size
        );
        unsafe {
            let data = slice::from_raw_parts(self.data as *const T, count);
            self.inner
                .device
                .state
                .lock()
                .allocator_mut()
                .invalidate_allocation(&self.inner.allocation, offset, data_size)
                .map_err(|e| {
                    log::error!("failed to invalidate allocation: {:?}", e);
                    vk::Result::ERROR_VALIDATION_FAILED_EXT // TODO
                })?;
            Ok(data)
        }
    }

    pub fn buffer(&self) -> Buffer {
        Buffer {
            inner: self.inner.clone(),
        }
    }
}
