use ash::version::DeviceV1_0;
use ash::vk;
use ash::vk::{DependencyFlags, MemoryPropertyFlags};

use vk_mem::{AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::error::Error;
use crate::imp::{pipeline, texture, BufferInner, BufferState, BufferViewInner, DeviceInner};
use crate::{Buffer, BufferDescriptor, BufferUsageFlags, BufferView, BufferViewDescriptor, MappedBuffer, WriteData};

use crate::imp::fenced_deleter::DeleteWhenUnused;
use parking_lot::Mutex;

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicPtr;
use std::sync::Arc;
use std::{mem, ptr, slice};

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

    // TODO: The GpuWeb spec doesn't yet define texel storage
    if usage.intersects(BufferUsageFlags::STORAGE) {
        flags |= vk::BufferUsageFlags::STORAGE_TEXEL_BUFFER;
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
    pub fn new(device: Arc<DeviceInner>, descriptor: BufferDescriptor) -> Result<BufferInner, Error> {
        let create_info = vk::BufferCreateInfo {
            size: descriptor.size as u64,
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
                    return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
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
    ) -> Result<(), Error> {
        let mut last_usage = self.last_usage.lock();

        log::trace!(
            "transition_usage_now buffer: {:?}, last_usage: {:?}, usage: {:?}",
            self.handle,
            *last_usage,
            usage
        );

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
            "usage: {:?}, last_usage: {:?}, src_stage_mask: {:?}, src_access_mask: {:?}",
            usage,
            *last_usage,
            src_stage_mask,
            src_access_mask
        );
        log::trace!(
            "usage: {:?}, last_usage: {:?}, dst_stage_mask: {:?}, dst_access_mask: {:?}",
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
            size: self.descriptor.size as u64,
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

        *last_usage = usage;

        Ok(())
    }

    pub unsafe fn get_mapped_ptr(&self) -> Result<*mut u8, Error> {
        let mut buffer_state = self.buffer_state.lock();
        match *buffer_state {
            BufferState::Mapped(_) => {
                log::warn!("buffer already mapped: {:?}", self.handle);
                Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT))
            }
            BufferState::Unmapped => {
                let mut state = self.device.state.lock();
                let ptr = state.allocator_mut().map_memory(&self.allocation).map_err(|e| {
                    log::error!("failed to map buffer memory: {:?}", e);
                    vk::Result::ERROR_MEMORY_MAP_FAILED // TODO
                })?;
                *buffer_state = BufferState::Mapped(AtomicPtr::new(ptr));
                Ok(ptr)
            }
        }
    }

    fn unmap(&self) -> Result<(), Error> {
        let mut buffer_state = self.buffer_state.lock();
        match *buffer_state {
            BufferState::Mapped(_) => {
                let mut state = self.device.state.lock();
                state.allocator_mut().unmap_memory(&self.allocation).map_err(|e| {
                    log::error!("failed to unmap buffer: {:?}", e);
                    Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT) // TODO
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
    fn validate_write_mapping<T: Copy>(&self, element_offset: usize, element_count: usize) -> Result<(), Error> {
        let element_size = mem::size_of::<T>();
        let data_size = element_size * element_count;
        let buffer_size = self.inner.descriptor.size as usize;
        let offset_bytes = element_size * element_offset;
        if !self.inner.descriptor.usage.intersects(BufferUsageFlags::MAP_WRITE) {
            log::error!("buffer not write mapped: {:?}", self.inner.handle);
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        if buffer_size < offset_bytes + data_size {
            log::error!(
                "write data range exceeds buffer size: offset_bytes: {}, data_size: {}, buffer_size: {}",
                offset_bytes,
                data_size,
                buffer_size
            );
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        Ok(())
    }

    pub fn write_data<T: Copy>(
        &mut self,
        element_offset: usize,
        element_count: usize,
    ) -> Result<WriteData<'_, T>, Error> {
        let element_size = mem::size_of::<T>();
        let offset_bytes = element_size * element_offset;

        self.validate_write_mapping::<T>(element_offset, element_count)?;

        Ok(WriteData {
            mapped: self,
            offset_bytes: offset_bytes as isize,
            element_count,
            _phantom: PhantomData,
        })
    }

    pub fn write<T: Copy>(&self, element_offset: usize, data: &[T]) -> Result<(), Error> {
        let count = data.len();
        let element_size = mem::size_of::<T>();
        let data_size = element_size * count;
        let buffer_size = self.inner.descriptor.size as usize;
        let offset_bytes = element_size * element_offset;
        if !self.inner.descriptor.usage.intersects(BufferUsageFlags::MAP_WRITE) {
            log::error!("buffer not write mapped: {:?}", self.inner.handle);
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        if buffer_size < offset_bytes + data_size {
            log::error!(
                "write data range exceeds buffer size: offset_bytes: {}, data_size: {}, buffer_size: {}",
                offset_bytes,
                data_size,
                buffer_size
            );
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        log::trace!(
            "map write data_size: offset_bytes: {}, {}, buffer_size: {}",
            offset_bytes,
            data_size,
            buffer_size
        );
        unsafe {
            let dst_ptr = self.data.add(offset_bytes);
            ptr::copy_nonoverlapping(data.as_ptr() as *const u8, dst_ptr, data_size);
            self.inner
                .device
                .state
                .lock()
                .allocator_mut()
                .flush_allocation(&self.inner.allocation, offset_bytes, data_size)
                .map_err(|e| {
                    log::error!("failed to flush allocation: {:?}", e);
                    Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT) // TODO
                })
        }
    }

    pub fn read<T: Copy>(&self, element_offset: usize, element_count: usize) -> Result<&[T], Error> {
        let element_size = mem::size_of::<T>();
        let data_size = element_size * element_count;
        let buffer_size = self.inner.descriptor.size as usize;
        let offset_bytes = element_size * element_offset;
        if !self.inner.descriptor.usage.intersects(BufferUsageFlags::MAP_READ) {
            log::error!("buffer not read mapped: {:?}", self.inner.handle);
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        if buffer_size < offset_bytes + data_size {
            log::error!(
                "read data range exceeds buffer size: offset_bytes: {}, data_size: {}, buffer_size: {}",
                offset_bytes,
                data_size,
                buffer_size
            );
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        unsafe {
            let src_ptr = self.data.add(offset_bytes);
            let data = slice::from_raw_parts(src_ptr as *const T, element_count);
            self.inner
                .device
                .state
                .lock()
                .allocator_mut()
                .invalidate_allocation(&self.inner.allocation, offset_bytes, data_size)
                .map_err(|e| {
                    log::error!("failed to invalidate allocation: {:?}", e);
                    vk::Result::ERROR_VALIDATION_FAILED_EXT // TODO
                })?;
            Ok(data)
        }
    }

    pub fn unmap(self) -> Buffer {
        Buffer {
            inner: self.inner.clone(),
        }
    }
}

impl Drop for MappedBuffer {
    fn drop(&mut self) {
        *self.inner.buffer_state.lock() = BufferState::Unmapped;
    }
}

impl<'a, T: Copy> Deref for WriteData<'a, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe {
            let data = self.mapped.data.offset(self.offset_bytes);
            std::slice::from_raw_parts(data as *const T, self.element_count)
        }
    }
}

impl<'a, T: Copy> DerefMut for WriteData<'a, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            let data = self.mapped.data.offset(self.offset_bytes);
            std::slice::from_raw_parts_mut(data as *mut T, self.element_count)
        }
    }
}

impl<'a, T> Drop for WriteData<'a, T> {
    fn drop(&mut self) {
        let length_bytes = self.element_count as _;
        let offset_bytes = self.offset_bytes as _;
        self.mapped
            .inner
            .device
            .state
            .lock()
            .allocator_mut()
            .invalidate_allocation(&self.mapped.inner.allocation, offset_bytes, length_bytes)
            .map_err(|e| {
                log::error!("WriteData::drop: failed to invalidate allocation: {:?}", e);
            })
            .ok();
    }
}

impl Buffer {
    /// Uploads all elements of `data` into the buffer. The buffer `offset` is in units of `T`.
    /// The buffer requires the `TRANSFER_DST` usage flag to be set.
    ///
    /// ## Implementation Note
    ///
    /// The content of `data` is read immediately, but the upload is deferred until the next
    /// command buffer submission.
    pub fn set_sub_data<T: Copy>(&self, offset: usize, data: &[T]) -> Result<(), Error> {
        // Dawn uses a ring buffer of staging buffers to perform the copy, but this is easier for now

        let element_size = std::mem::size_of::<T>();
        let data_size = element_size * data.len();
        let offset_bytes = element_size * offset;
        if data_size > std::u16::MAX as usize {
            log::error!(
                "set_sub_data can not be used to copy more than {} bytes; data_size: {}",
                std::u16::MAX,
                data_size
            );
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        let buffer_size = self.inner.descriptor.size as usize;
        if offset_bytes + data_size > buffer_size {
            log::error!(
                "set_sub_data range exceeds buffer size; offset: {}, data_size: {:?}, buffer_size: {:?}",
                offset,
                data_size,
                buffer_size
            );
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }

        let mut state = self.inner.device.state.lock();

        let command_buffer = state.get_pending_command_buffer(&self.inner.device)?;
        if BufferUsageFlags::TRANSFER_DST != *self.inner.last_usage.lock() {
            self.inner
                .transition_usage_now(command_buffer, BufferUsageFlags::TRANSFER_DST)?;
        }
        unsafe {
            let offset_bytes = offset_bytes as u64;
            let data: &[u8] = slice::from_raw_parts(data.as_ptr() as *const u8, data_size);
            self.inner
                .device
                .raw
                .cmd_update_buffer(command_buffer, self.inner.handle, offset_bytes, data);
            Ok(())
        }
    }

    /// Returns the size of the buffer in bytes.
    pub fn size(&self) -> usize {
        self.inner.descriptor.size as usize
    }

    /// Returns the usage flags declared when the buffer was created.
    pub fn usage(&self) -> BufferUsageFlags {
        self.inner.descriptor.usage
    }

    pub fn map_read(&self) -> Result<MappedBuffer, Error> {
        if !self.inner.descriptor.usage.contains(BufferUsageFlags::MAP_READ) {
            log::warn!("buffer not created with MAP_READ");
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        let data = unsafe { self.inner.get_mapped_ptr()? };
        Ok(MappedBuffer {
            inner: Arc::clone(&self.inner),
            data,
        })
    }

    pub fn map_write(&self) -> Result<MappedBuffer, Error> {
        if !self.inner.descriptor.usage.contains(BufferUsageFlags::MAP_WRITE) {
            log::warn!("buffer not created with MAP_WRITE");
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        let data = unsafe { self.inner.get_mapped_ptr()? };
        Ok(MappedBuffer {
            inner: Arc::clone(&self.inner),
            data,
        })
    }

    pub fn create_view(&self, descriptor: BufferViewDescriptor) -> Result<BufferView, Error> {
        let buffer_view = BufferViewInner::new(self.inner.clone(), descriptor)?;
        Ok(buffer_view.into())
    }
}

impl BufferViewInner {
    pub fn new(buffer: Arc<BufferInner>, descriptor: BufferViewDescriptor) -> Result<BufferViewInner, Error> {
        let format = descriptor.format.either(texture::image_format, pipeline::vertex_format);
        let create_info = vk::BufferViewCreateInfo {
            buffer: buffer.handle,
            offset: descriptor.offset as u64,
            range: descriptor.size as u64,
            format,
            ..Default::default()
        };
        let handle = unsafe { buffer.device.raw.create_buffer_view(&create_info, None)? };
        Ok(BufferViewInner { handle, buffer })
    }
}

impl Into<BufferView> for BufferViewInner {
    fn into(self) -> BufferView {
        BufferView { inner: Arc::new(self) }
    }
}

impl Drop for BufferViewInner {
    fn drop(&mut self) {
        let mut state = self.buffer.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}
