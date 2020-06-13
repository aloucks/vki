use std::time::Duration;
use vki::{BufferDescriptor, BufferUsage};

pub mod support;

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    static ref LOCK: std::sync::Mutex::<()> = std::sync::Mutex::new(());
}

#[test]
fn create_buffer_vertex_transfer_dst() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_uniform_mapped_write() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsage::UNIFORM | BufferUsage::MAP_WRITE,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_write_staging() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsage::COPY_SRC | BufferUsage::MAP_WRITE,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_read_staging() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsage::COPY_DST | BufferUsage::MAP_READ,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_buffer_read_storage() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsage::STORAGE | BufferUsage::MAP_READ,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        drop(_buffer);

        Ok(instance)
    });
}

#[test]
fn create_buffer_mapped() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let mut encoder = device.create_command_encoder()?;

        let data: &[u32] = &[1, 2, 3, 4, 5];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();
        let data_byte_size = data_byte_size;

        let write_buffer_mapped = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsage::MAP_WRITE | BufferUsage::COPY_SRC,
            size: data_byte_size,
        })?;

        write_buffer_mapped.copy_from_slice(data)?;

        let read_buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsage::MAP_READ | BufferUsage::COPY_DST,
            size: data_byte_size,
        })?;

        encoder.copy_buffer_to_buffer(&write_buffer_mapped.unmap(), 0, &read_buffer, 0, data_byte_size);

        let queue = device.get_queue();

        queue.submit(&[encoder.finish()?])?;

        let fence = queue.create_fence()?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read_buffer_mapped = read_buffer.map_read()?;

        let read: &[u32] = read_buffer_mapped.read(0, data.len())?;
        assert_eq!(data, read);

        Ok(instance)
    });
}

#[test]
fn create_buffer_mapped_write_data() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let mut encoder = device.create_command_encoder()?;

        let data: &[u32] = &[1, 2, 3, 4, 5];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();
        let data_byte_size = data_byte_size;

        let mut write_buffer_mapped = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsage::MAP_WRITE | BufferUsage::COPY_SRC,
            size: data_byte_size,
        })?;

        let mut write_data = write_buffer_mapped.write::<u32>(0, data.len())?;

        assert_eq!(write_data.len(), data.len());
        write_data.copy_from_slice(data);

        write_data.flush()?;

        let read_buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsage::MAP_READ | BufferUsage::COPY_DST,
            size: data_byte_size,
        })?;

        encoder.copy_buffer_to_buffer(&write_buffer_mapped.unmap(), 0, &read_buffer, 0, data_byte_size);

        let queue = device.get_queue();

        queue.submit(&[encoder.finish()?])?;

        let fence = queue.create_fence()?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read_buffer_mapped = read_buffer.map_read()?;

        let read: &[u32] = read_buffer_mapped.read(0, data.len())?;
        assert_eq!(data, read);

        Ok(instance)
    });
}

#[test]
fn set_sub_data() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let data: &[u32] = &[1, 2, 3, 4];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();

        let read_buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsage::MAP_READ | BufferUsage::COPY_DST,
            size: data_byte_size,
        })?;

        read_buffer.set_sub_data(0, data)?;

        // TODO: set_sub_data records into the pending command buffer
        //       so the read below won't pick it up until the command
        //       buffer is submitted. When refactoring MappedBuffer,
        //       read and write mapping should probably handle this
        //       better.

        let queue = device.get_queue();

        let encoder = device.create_command_encoder()?;

        queue.submit(&[encoder.finish()?])?;

        let fence = queue.create_fence()?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read_buffer_mapped = read_buffer.map_read()?;

        let read_data = read_buffer_mapped.read::<u32>(0, 4)?;

        assert_eq!(data, read_data);

        Ok(instance)
    });
}

#[test]
fn set_sub_data_offset() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let data: &[u32] = &[1, 2, 3, 4];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();

        let read_buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsage::MAP_READ | BufferUsage::COPY_DST,
            size: (2 * data_byte_size) as _,
        })?;

        read_buffer.set_sub_data(0, data)?;
        read_buffer.set_sub_data(data.len(), data)?;

        let queue = device.get_queue();

        let encoder = device.create_command_encoder()?;

        queue.submit(&[encoder.finish()?])?;

        let fence = queue.create_fence()?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read_buffer_mapped = read_buffer.map_read()?;

        let read_data = read_buffer_mapped.read::<u32>(0, 4)?;
        assert_eq!(data, read_data);

        let read_data = read_buffer_mapped.read::<u32>(4, 4)?;
        assert_eq!(data, read_data);

        let read_data = read_buffer_mapped.read::<u32>(2, 4)?;
        assert_eq!(&[3, 4, 1, 2], read_data);

        Ok(instance)
    });
}

#[test]
fn mapping_twice_should_fail() {
    #[cfg(target_os = "linux")]
    let _guard = LOCK.lock().unwrap();

    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let data: &[u32] = &[1, 2, 3, 4];
        let data_byte_size = std::mem::size_of::<u32>() * data.len();

        let buffer = device.create_buffer(BufferDescriptor {
            usage: BufferUsage::MAP_READ | BufferUsage::COPY_DST,
            size: data_byte_size,
        })?;

        let _mapped = buffer.map_read()?;

        assert_eq!(true, buffer.map_read().is_err(), "mapping twice should fail");

        Ok(instance)
    });
}
