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
