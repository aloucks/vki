use vki::{BufferDescriptor, BufferUsageFlags};

mod support;

#[test]
fn create_buffer_vertex_transfer_dst() {
    vki::validate(|| {
        let (_event_loop, _window, instance, _surface, _adapter, device, _swapchain) = support::init()?;

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
        let (_event_loop, _window, instance, _surface, _adapter, device, _swapchain) = support::init()?;

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
        let (_event_loop, _window, instance, _surface, _adapter, device, _swapchain) = support::init()?;

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
        let (_event_loop, _window, instance, _surface, _adapter, device, _swapchain) = support::init()?;

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
        let (_event_loop, _window, instance, _surface, _adapter, device, _swapchain) = support::init()?;

        let descriptor = BufferDescriptor {
            usage: BufferUsageFlags::STORAGE | BufferUsageFlags::MAP_READ,
            size: 1024,
        };

        let _buffer = device.create_buffer(descriptor)?;

        Ok(instance)
    });
}
