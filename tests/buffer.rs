use std::time::Duration;
use vki::{BufferDescriptor, BufferUsageFlags};

pub mod support;

#[test]
fn create_buffer_vertex_transfer_dst() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsageFlags::VERTEX | BufferUsageFlags::TRANSFER_DST,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_uniform_mapped_write() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsageFlags::UNIFORM | BufferUsageFlags::MAP_WRITE,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_write_staging() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::MAP_WRITE,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_read_staging() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsageFlags::TRANSFER_DST | BufferUsageFlags::MAP_READ,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_read_storage() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsageFlags::STORAGE | BufferUsageFlags::MAP_READ,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        drop(_buffer);

        Ok(instance)
    });
}

#[test]
fn create_buffer_mapped() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let mut encoder = device.create_command_encoder()?;

        let data: &[u32] = &[1, 2, 3, 4, 5];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();
        let data_byte_size = data_byte_size as u64;

        let write_buffer = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsageFlags::MAP_WRITE | BufferUsageFlags::TRANSFER_SRC,
            size: data_byte_size,
        })?;

        let read_buffer = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsageFlags::MAP_READ | BufferUsageFlags::TRANSFER_DST,
            size: data_byte_size,
        })?;

        write_buffer.write(0, data)?;

        encoder.copy_buffer_to_buffer(write_buffer.buffer(), 0, read_buffer.buffer(), 0, data_byte_size);

        let fence = device.create_fence()?;

        let queue = device.get_queue();
        queue.submit(encoder.finish()?)?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read: &[u32] = read_buffer.read(0, data.len())?;
        assert_eq!(data, read);

        Ok(instance)
    });
}

#[test]
fn set_sub_data() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let data: &[u32] = &[1, 2, 3, 4];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();

        let read_buffer = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsageFlags::MAP_READ | BufferUsageFlags::TRANSFER_DST,
            size: data_byte_size as _,
        })?;

        read_buffer.buffer().set_sub_data(0, data)?;

        // TODO: set_sub_data records into the pending command buffer
        //       so the read below won't pick it up until the command
        //       buffer is submitted. When refactoring MappedBuffer,
        //       read and write mapping should probably handle this
        //       better.

        let fence = device.create_fence()?;

        let encoder = device.create_command_encoder()?;
        device.get_queue().submit(encoder.finish()?)?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read_data = read_buffer.read::<u32>(0, 4)?;

        assert_eq!(data, read_data);

        Ok(instance)
    });
}

#[test]
fn set_sub_data_offset() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let data: &[u32] = &[1, 2, 3, 4];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();

        let read_buffer = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsageFlags::MAP_READ | BufferUsageFlags::TRANSFER_DST,
            size: (2 * data_byte_size) as _,
        })?;

        read_buffer.buffer().set_sub_data(0, data)?;
        read_buffer.buffer().set_sub_data(data.len(), data)?;

        let fence = device.create_fence()?;

        let encoder = device.create_command_encoder()?;
        device.get_queue().submit(encoder.finish()?)?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read_data = read_buffer.read::<u32>(0, 4)?;
        assert_eq!(data, read_data);

        let read_data = read_buffer.read::<u32>(4, 4)?;
        assert_eq!(data, read_data);

        let read_data = read_buffer.read::<u32>(2, 4)?;
        assert_eq!(&[3, 4, 1, 2], read_data);

        Ok(instance)
    });
}
