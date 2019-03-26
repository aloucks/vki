use ash::version::DeviceV1_0;
use ash::vk;
use ash::vk::{DependencyFlags, MemoryPropertyFlags};

use vk_mem::{AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::imp::{BufferInner, BufferState, DeviceInner};
use crate::{Buffer, BufferDescriptor, BufferUsageFlags};

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

    if usage.contains(BufferUsageFlags::MAP_WRITE | BufferUsageFlags::TRANSFER_SRC) {
        return MemoryUsage::CpuOnly;
    }

    if usage.contains(BufferUsageFlags::MAP_READ | BufferUsageFlags::TRANSFER_DST) {
        return MemoryUsage::CpuOnly;
    }

    if usage.contains(BufferUsageFlags::MAP_WRITE) {
        return MemoryUsage::CpuToGpu;
    }

    if usage.contains(BufferUsageFlags::MAP_READ) {
        return MemoryUsage::GpuToCpu;
    }

    MemoryUsage::GpuOnly
}

pub fn usage_flags(usage: BufferUsageFlags) -> vk::BufferUsageFlags {
    let mut flags = vk::BufferUsageFlags::empty();

    if usage.contains(BufferUsageFlags::TRANSFER_SRC) {
        flags |= vk::BufferUsageFlags::TRANSFER_SRC;
    }

    if usage.contains(BufferUsageFlags::TRANSFER_DST) {
        flags |= vk::BufferUsageFlags::TRANSFER_DST;
    }

    if usage.contains(BufferUsageFlags::INDEX) {
        flags |= vk::BufferUsageFlags::INDEX_BUFFER;
    }

    if usage.contains(BufferUsageFlags::VERTEX) {
        flags |= vk::BufferUsageFlags::VERTEX_BUFFER;
    }

    if usage.contains(BufferUsageFlags::UNIFORM) {
        flags |= vk::BufferUsageFlags::UNIFORM_BUFFER;
    }

    if usage.contains(BufferUsageFlags::STORAGE) {
        flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
    }

    flags
}

pub fn pipeline_stage(usage: BufferUsageFlags) -> vk::PipelineStageFlags {
    let mut flags = vk::PipelineStageFlags::empty();

    if usage.contains(BufferUsageFlags::MAP_READ | BufferUsageFlags::MAP_WRITE) {
        flags |= vk::PipelineStageFlags::HOST;
    }

    if usage.contains(BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::TRANSFER_DST) {
        flags |= vk::PipelineStageFlags::TRANSFER;
    }

    if usage.contains(BufferUsageFlags::INDEX | BufferUsageFlags::VERTEX) {
        flags |= vk::PipelineStageFlags::VERTEX_INPUT;
    }

    if usage.contains(BufferUsageFlags::UNIFORM | BufferUsageFlags::STORAGE) {
        flags |= vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
    }

    flags
}

pub fn access_flags(usage: BufferUsageFlags) -> vk::AccessFlags {
    let mut flags = vk::AccessFlags::empty();

    if usage.contains(BufferUsageFlags::MAP_READ) {
        flags |= vk::AccessFlags::HOST_READ
    }

    if usage.contains(BufferUsageFlags::MAP_WRITE) {
        flags |= vk::AccessFlags::HOST_WRITE
    }

    if usage.contains(BufferUsageFlags::TRANSFER_SRC) {
        flags |= vk::AccessFlags::TRANSFER_READ
    }

    if usage.contains(BufferUsageFlags::TRANSFER_DST) {
        flags |= vk::AccessFlags::TRANSFER_WRITE
    }

    if usage.contains(BufferUsageFlags::INDEX) {
        flags |= vk::AccessFlags::INDEX_READ
    }

    if usage.contains(BufferUsageFlags::VERTEX) {
        flags |= vk::AccessFlags::VERTEX_ATTRIBUTE_READ
    }

    if usage.contains(BufferUsageFlags::UNIFORM) {
        flags |= vk::AccessFlags::UNIFORM_READ
    }

    if usage.contains(BufferUsageFlags::STORAGE) {
        flags |= vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE
    }

    flags
}

impl BufferInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: BufferDescriptor) -> Result<BufferInner, vk::Result> {
        let create_info = vk::BufferCreateInfo::builder()
            .size(descriptor.size)
            .usage(usage_flags(descriptor.usage))
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build();

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
            match e.kind() {
                &vk_mem::ErrorKind::Vulkan(vk::Result::ERROR_VALIDATION_FAILED_EXT) => unsafe {
                    let dummy = device.raw.create_buffer(&create_info, None)?;
                    device.raw.destroy_buffer(dummy, None);
                    return Err(vk::Result::ERROR_VALIDATION_FAILED_EXT);
                },
                _ => {}
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

    pub fn transition_usage(
        &self,
        command_buffer: vk::CommandBuffer,
        usage: BufferUsageFlags,
    ) -> Result<(), vk::Result> {
        let mut last_usage = self.last_usage.lock();
        let last_includes_target = (*last_usage & self.descriptor.usage) == usage;
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

        let buffer_memory_barrier = vk::BufferMemoryBarrier::builder()
            .src_access_mask(src_access_mask)
            .dst_access_mask(dst_access_mask)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .buffer(self.handle)
            .offset(0)
            .size(self.descriptor.size)
            .build();

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
}

impl Into<Buffer> for BufferInner {
    fn into(self) -> Buffer {
        Buffer { inner: Arc::new(self) }
    }
}

impl Drop for BufferInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state
            .get_fenced_deleter()
            .delete_when_unused((self.handle, self.allocation.clone()), serial);
    }
}
