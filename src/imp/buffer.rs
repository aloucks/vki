use ash::version::DeviceV1_0;
use ash::vk;
use ash::vk::{DependencyFlags, MemoryPropertyFlags};

use vk_mem::{AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::{pipeline, texture, BufferInner, BufferState, BufferViewInner, DeviceInner};
use crate::{
    Buffer, BufferDescriptor, BufferUsage, BufferView, BufferViewDescriptor, BufferViewFormat, Error, MappedBuffer,
    WriteData,
};

use parking_lot::Mutex;

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicPtr;
use std::sync::Arc;
use std::{mem, ptr, slice};

pub fn read_only_buffer_usages() -> BufferUsage {
    BufferUsage::MAP_READ | BufferUsage::COPY_SRC | BufferUsage::INDEX | BufferUsage::VERTEX | BufferUsage::UNIFORM
}

pub fn writable_buffer_usages() -> BufferUsage {
    BufferUsage::MAP_WRITE | BufferUsage::COPY_DST | BufferUsage::STORAGE
}

/// https://gpuopen-librariesandsdks.github.io/VulkanMemoryAllocator/html/usage_patterns.html
pub fn memory_usage(usage: BufferUsage) -> MemoryUsage {
    assert_ne!(usage, BufferUsage::NONE, "BufferUsageFlags may not be NONE");

    // Note: `contains` here with the OR'd flags results in "must contain *both* flags".
    //       Although it's obvious when you sit and think about, but on first glance
    //       it may read something else entirely!

    // Staging resources that are used to transfer to the GPU
    if usage.contains(BufferUsage::MAP_WRITE | BufferUsage::COPY_SRC) {
        return MemoryUsage::CpuOnly;
    }

    // Staging resources that are used to transfer from the GPU
    if usage.contains(BufferUsage::MAP_READ | BufferUsage::COPY_DST) {
        return MemoryUsage::CpuOnly;
    }

    // Dynamic resources that are updated often by the CPU and read directly by the GPU
    if usage.contains(BufferUsage::MAP_WRITE) {
        return MemoryUsage::CpuToGpu;
    }

    // Readback resources that are written often by the GPU and read directly by the CPU
    if usage.contains(BufferUsage::MAP_READ) {
        return MemoryUsage::GpuToCpu;
    }

    MemoryUsage::GpuOnly
}

pub fn usage_flags(usage: BufferUsage) -> vk::BufferUsageFlags {
    let mut flags = vk::BufferUsageFlags::empty();

    if usage.intersects(BufferUsage::COPY_SRC) {
        flags |= vk::BufferUsageFlags::TRANSFER_SRC;
    }

    if usage.intersects(BufferUsage::COPY_DST) {
        flags |= vk::BufferUsageFlags::TRANSFER_DST;
    }

    if usage.intersects(BufferUsage::INDEX) {
        flags |= vk::BufferUsageFlags::INDEX_BUFFER;
    }

    if usage.intersects(BufferUsage::VERTEX) {
        flags |= vk::BufferUsageFlags::VERTEX_BUFFER;
    }

    if usage.intersects(BufferUsage::UNIFORM) {
        flags |= vk::BufferUsageFlags::UNIFORM_BUFFER;
    }

    if usage.intersects(BufferUsage::STORAGE) {
        flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
    }

    // TODO: The GpuWeb spec doesn't yet define texel storage
    if usage.intersects(BufferUsage::STORAGE) {
        flags |= vk::BufferUsageFlags::STORAGE_TEXEL_BUFFER;
    }

    if usage.intersects(BufferUsage::INDIRECT) {
        flags |= vk::BufferUsageFlags::INDIRECT_BUFFER;
    }

    flags
}

pub fn pipeline_stage(usage: BufferUsage) -> vk::PipelineStageFlags {
    let mut flags = vk::PipelineStageFlags::empty();

    if usage.intersects(BufferUsage::MAP_READ | BufferUsage::MAP_WRITE) {
        flags |= vk::PipelineStageFlags::HOST;
    }

    if usage.intersects(BufferUsage::COPY_SRC | BufferUsage::COPY_DST) {
        flags |= vk::PipelineStageFlags::TRANSFER;
    }

    if usage.intersects(BufferUsage::INDEX | BufferUsage::VERTEX) {
        flags |= vk::PipelineStageFlags::VERTEX_INPUT;
    }

    if usage.intersects(BufferUsage::UNIFORM | BufferUsage::STORAGE) {
        flags |= vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
    }

    if usage.intersects(BufferUsage::INDIRECT) {
        flags |= vk::PipelineStageFlags::DRAW_INDIRECT;
    }

    flags
}

pub fn access_flags(usage: BufferUsage) -> vk::AccessFlags {
    let mut flags = vk::AccessFlags::empty();

    if usage.intersects(BufferUsage::MAP_READ) {
        flags |= vk::AccessFlags::HOST_READ
    }

    if usage.intersects(BufferUsage::MAP_WRITE) {
        flags |= vk::AccessFlags::HOST_WRITE
    }

    if usage.intersects(BufferUsage::COPY_SRC) {
        flags |= vk::AccessFlags::TRANSFER_READ
    }

    if usage.intersects(BufferUsage::COPY_DST) {
        flags |= vk::AccessFlags::TRANSFER_WRITE
    }

    if usage.intersects(BufferUsage::INDEX) {
        flags |= vk::AccessFlags::INDEX_READ
    }

    if usage.intersects(BufferUsage::VERTEX) {
        flags |= vk::AccessFlags::VERTEX_ATTRIBUTE_READ
    }

    if usage.intersects(BufferUsage::UNIFORM) {
        flags |= vk::AccessFlags::UNIFORM_READ
    }

    if usage.intersects(BufferUsage::INDIRECT) {
        flags |= vk::AccessFlags::INDIRECT_COMMAND_READ
    }

    // TODO: The read-only and write-only flags should probably be considered here
    if usage.intersects(BufferUsage::STORAGE) {
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

        let allocator = &device.allocator;

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

        Ok(BufferInner {
            descriptor,
            allocation,
            allocation_info,
            device,
            last_usage: Mutex::new(BufferUsage::NONE),
            buffer_state: Mutex::new(BufferState::Unmapped),
            handle: buffer,
        })
    }

    pub fn transition_usage_now(&self, command_buffer: vk::CommandBuffer, usage: BufferUsage) -> Result<(), Error> {
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
        if *last_usage == BufferUsage::NONE {
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
                // TODO: Validation
                Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT))
            }
            BufferState::Unmapped => {
                let ptr = self.device.allocator.map_memory(&self.allocation).map_err(|e| {
                    log::error!("failed to map buffer memory: {:?}", e);
                    match e.kind() {
                        vk_mem::ErrorKind::Vulkan(e) => Error::from(*e),
                        // TODO: Better error handling
                        _ => Error::from(format!("map_memory error: {:?}", e)),
                    }
                })?;
                *buffer_state = BufferState::Mapped(AtomicPtr::new(ptr));
                Ok(ptr)
            }
        }
    }

    /// TODO: We should disconnect the buffer state from the actual memory mapping
    ///       so that the buffer can stay permanently mapped whenever it's created
    ///       with MAP_READ or MAP_WRITE. The buffer state will then become a user
    ///       only state to prevent modification while the buffer is in use.
    fn unmap_memory(&self) -> Result<(), Error> {
        let mut buffer_state = self.buffer_state.lock();
        match *buffer_state {
            BufferState::Mapped(_) => {
                self.device.allocator.unmap_memory(&self.allocation).map_err(|e| {
                    log::error!("failed to unmap buffer: {:?}", e);
                    match e.kind() {
                        vk_mem::ErrorKind::Vulkan(e) => Error::from(*e),
                        // TODO: Better error handling
                        _ => Error::from(format!("unmap_memory error: {:?}", e)),
                    }
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
        self.unmap_memory()
            .map_err(|e| log::error!("failed to unmap_memory: {:?}", e))
            .ok();
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state
            .get_fenced_deleter()
            .delete_when_unused((self.handle, self.allocation.clone()), serial);
    }
}

impl MappedBuffer {
    fn validate_mapping<T: Copy>(
        &self,
        element_offset: usize,
        element_count: usize,
        flags: BufferUsage,
    ) -> Result<(), Error> {
        let element_size = mem::size_of::<T>();
        let data_size = element_size * element_count;
        let buffer_size = self.inner.descriptor.size as usize;
        let offset_bytes = element_size * element_offset;
        if !self.inner.descriptor.usage.intersects(flags) {
            let msg = format!("missing required usage: {:?}", flags);
            return Err(Error::from(msg));
        }
        if buffer_size < offset_bytes + data_size {
            log::error!(
                "mapping range exceeds buffer size: offset_bytes: {}, data_size: {}, buffer_size: {}",
                offset_bytes,
                data_size,
                buffer_size
            );
            return Err(Error::from(vk::Result::ERROR_VALIDATION_FAILED_EXT));
        }
        Ok(())
    }

    pub fn write<T: Copy>(&mut self, element_offset: usize, element_count: usize) -> Result<WriteData<'_, T>, Error> {
        let element_size = mem::size_of::<T>();
        let offset_bytes = element_size * element_offset;

        self.validate_mapping::<T>(element_offset, element_count, BufferUsage::MAP_WRITE)?;

        Ok(WriteData {
            mapped: self,
            offset_bytes: offset_bytes as isize,
            element_count,
            _phantom: PhantomData,
        })
    }

    /// Fill the buffer with the provided slice
    pub fn copy_from_slice<T: Copy>(&self, data: &[T]) -> Result<(), Error> {
        let element_offset = 0;
        let element_count = data.len();
        let element_size = mem::size_of::<T>();
        let data_size = element_size * element_count;
        let buffer_size = self.inner.descriptor.size as usize;
        let offset_bytes = element_size * element_offset;

        self.validate_mapping::<T>(element_offset, element_count, BufferUsage::MAP_WRITE)?;

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
                .allocator
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
        let offset_bytes = element_size * element_offset;

        self.validate_mapping::<T>(element_offset, element_count, BufferUsage::MAP_READ)?;

        unsafe {
            let src_ptr = self.data.add(offset_bytes);
            let data = slice::from_raw_parts(src_ptr as *const T, element_count);
            self.inner
                .device
                .allocator
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
        self.inner
            .unmap_memory()
            .map_err(|e| log::error!("failed to unmap_memory: {:?}", e))
            .ok();
    }
}

impl<'a, T: Copy> Deref for WriteData<'a, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        // TODO: This message is a overly conservative. The `WriteData` may have been
        //       deref'd to use `len()` for example.
        // log::error!("buffer is mapped for write-only operations");
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

impl<'a, T> WriteData<'a, T> {
    fn _flush(&mut self) -> Result<(), Error> {
        let length_bytes = std::mem::size_of::<T>() * self.element_count as usize;
        let offset_bytes = self.offset_bytes as _;

        self.mapped.inner.device.allocator.flush_allocation(
            &self.mapped.inner.allocation,
            offset_bytes,
            length_bytes,
        )?;
        Ok(())
    }

    pub fn flush(mut self) -> Result<(), Error> {
        let result = self._flush();
        self.element_count = 0;
        result
    }

    /// Returns the number of elements `T` in the slice
    pub fn len(&self) -> usize {
        self.element_count
    }

    pub fn is_empty(&self) -> bool {
        self.len() > 0
    }
}

impl<'a, T> Drop for WriteData<'a, T> {
    fn drop(&mut self) {
        if self.element_count > 0 {
            self._flush()
                .map_err(|e| {
                    log::error!("failed to flush allocation: {:?}", e);
                })
                .ok();
        }
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
        if BufferUsage::COPY_DST != *self.inner.last_usage.lock() {
            self.inner.transition_usage_now(command_buffer, BufferUsage::COPY_DST)?;
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
    pub fn usage(&self) -> BufferUsage {
        self.inner.descriptor.usage
    }

    pub fn map_read(&self) -> Result<MappedBuffer, Error> {
        if !self.inner.descriptor.usage.contains(BufferUsage::MAP_READ) {
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
        if !self.inner.descriptor.usage.contains(BufferUsage::MAP_WRITE) {
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

impl From<BufferViewFormat> for vk::Format {
    fn from(f: BufferViewFormat) -> vk::Format {
        match f {
            BufferViewFormat::Texture(f) => texture::image_format(f),
            BufferViewFormat::Vertex(f) => pipeline::vertex_format(f),
        }
    }
}

impl BufferViewInner {
    pub fn new(buffer: Arc<BufferInner>, descriptor: BufferViewDescriptor) -> Result<BufferViewInner, Error> {
        let format = vk::Format::from(descriptor.format);
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

impl BufferView {
    /// Returns a handle to the associated `Buffer`.
    pub fn buffer(&self) -> Buffer {
        Buffer {
            inner: self.inner.buffer.clone(),
        }
    }
}
