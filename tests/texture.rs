use vki::{
    BufferCopyView, BufferDescriptor, BufferUsageFlags, Extent3D, FilterMode, Origin3D, TextureAspectFlags,
    TextureBlitView, TextureCopyView, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags,
    TextureViewDescriptor, TextureViewDimension,
};

pub mod support;

#[test]
fn create_texture() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = TextureDescriptor {
            usage: TextureUsageFlags::SAMPLED,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
        };

        let _texture = device.create_texture(&descriptor)?;

        Ok(instance)
    });
}

#[test]
fn create_default_texture_view() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = TextureDescriptor {
            usage: TextureUsageFlags::SAMPLED,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
        };

        let texture = device.create_texture(&descriptor)?;
        let _texture_view = texture.create_default_view()?;

        Ok(instance)
    });
}

#[test]
fn create_texture_and_cube_view() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let array_layer_count = 6;

        let descriptor = TextureDescriptor {
            usage: TextureUsageFlags::SAMPLED,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
            array_layer_count,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8G8B8A8Unorm,
        };

        let texture = device.create_texture(&descriptor)?;

        let texture_view_descriptor = TextureViewDescriptor {
            dimension: TextureViewDimension::Cube,
            aspect: TextureAspectFlags::COLOR,
            base_array_layer: 0,
            array_layer_count: descriptor.array_layer_count,
            base_mip_level: 0,
            mip_level_count: descriptor.mip_level_count,
            format: descriptor.format,
        };

        let _texture_view = texture.create_view(texture_view_descriptor)?;

        Ok(instance)
    });
}

#[test]
fn copy_texture_to_texture() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let (width, height, depth) = (1024, 1024, 1);

        let size = Extent3D { width, height, depth };

        let texture1 = device.create_texture(&TextureDescriptor {
            usage: TextureUsageFlags::TRANSFER_SRC,
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            dimension: TextureDimension::D2,
            size,
            array_layer_count: 1,
            mip_level_count: 1,
        })?;

        let texture2 = device.create_texture(&TextureDescriptor {
            usage: TextureUsageFlags::SAMPLED | TextureUsageFlags::TRANSFER_DST,
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            dimension: TextureDimension::D2,
            size,
            array_layer_count: 1,
            mip_level_count: 1,
        })?;

        let src = TextureCopyView {
            texture: &texture1,
            mip_level: 0,
            array_layer: 0,
            origin: Origin3D { x: 0, y: 0, z: 0 },
        };

        let dst = TextureCopyView {
            texture: &texture2,
            mip_level: 0,
            array_layer: 0,
            origin: Origin3D { x: 0, y: 0, z: 0 },
        };

        let mut encoder = device.create_command_encoder(None)?;

        encoder.copy_texture_to_texture(src, dst, size);

        let command_buffers = &[encoder.finish()?];

        let queue = device.get_queue();

        queue.submit(command_buffers)?;

        // TODO: Verification

        // Submitting twice isn't necessary, but this helps catch issues with subresource tracking
        queue.submit(command_buffers)?;

        Ok(instance)
    })
}

#[test]
fn blit_texture_to_texture_generate_mipmaps() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let (width, height, depth) = (1024, 1024, 1);

        let mip_level_count = (width.max(height) as f32).log2().floor() as u32 + 1;

        let texture = device.create_texture(&TextureDescriptor {
            usage: TextureUsageFlags::SAMPLED | TextureUsageFlags::TRANSFER_SRC | TextureUsageFlags::TRANSFER_DST,
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            dimension: TextureDimension::D2,
            size: Extent3D { width, height, depth },
            array_layer_count: 1,
            mip_level_count,
        })?;

        let mut encoder = device.create_command_encoder(None)?;

        let mut mip_width = width;
        let mut mip_height = height;

        for i in 1..mip_level_count {
            let src = TextureBlitView {
                texture: &texture,
                mip_level: i - 1,
                array_layer: 0,
                bounds: [
                    Origin3D { x: 0, y: 0, z: 0 },
                    Origin3D {
                        x: mip_width as i32,
                        y: mip_height as i32,
                        z: 1,
                    },
                ],
            };

            if mip_width > 1 {
                mip_width = mip_width / 2;
            }

            if mip_height > 1 {
                mip_height = mip_height / 2;
            }

            let dst = TextureBlitView {
                texture: &texture,
                mip_level: i,
                array_layer: 0,
                bounds: [
                    Origin3D { x: 0, y: 0, z: 0 },
                    Origin3D {
                        x: mip_width as i32,
                        y: mip_height as i32,
                        z: 1,
                    },
                ],
            };

            encoder.blit_texture_to_texture(src, dst, FilterMode::Linear);
        }

        let command_buffers = &[encoder.finish()?];

        let queue = device.get_queue();

        queue.submit(command_buffers)?;

        // TODO: Verification

        // Submitting twice isn't necessary, but this helps catch issues with subresource tracking
        queue.submit(command_buffers)?;

        Ok(instance)
    })
}

#[test]
fn create_depth_texture_and_view() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = TextureDescriptor {
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::D32Float,
        };

        let texture = device.create_texture(&descriptor)?;

        let _texture_view = texture.create_default_view()?;

        Ok(instance)
    });
}

#[test]
fn create_depth_stencil_texture_and_view() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let descriptor = TextureDescriptor {
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::D32FloatS8Uint,
        };

        let texture = device.create_texture(&descriptor)?;

        let _texture_view = texture.create_default_view()?;

        Ok(instance)
    });
}

#[test]
fn copy_buffer_to_texture() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let (width, height, depth) = (1024, 1024, 1);
        let size = Extent3D { width, height, depth };

        let buffer1 = device.create_buffer(&BufferDescriptor {
            size: (width * height) as usize * std::mem::size_of::<f32>(),
            usage: BufferUsageFlags::TRANSFER_SRC,
        })?;

        let texture1 = device.create_texture(&TextureDescriptor {
            usage: TextureUsageFlags::TRANSFER_DST,
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            dimension: TextureDimension::D2,
            size,
            array_layer_count: 1,
            mip_level_count: 1,
        })?;

        let src = BufferCopyView {
            buffer: &buffer1,
            row_length: width, // TODO row_pitch
            image_height: height,
            offset: 0,
        };

        let dst = TextureCopyView {
            texture: &texture1,
            mip_level: 0,
            array_layer: 0,
            origin: Origin3D { x: 0, y: 0, z: 0 },
        };

        let mut encoder = device.create_command_encoder(None)?;

        encoder.copy_buffer_to_texture(src, dst, size);

        let command_buffers = &[encoder.finish()?];

        let queue = device.get_queue();

        queue.submit(command_buffers)?;

        // TODO: Verification

        // Submitting twice isn't necessary, but this helps catch issues with subresource tracking
        queue.submit(command_buffers)?;

        Ok(instance)
    })
}

#[test]
fn copy_texture_to_buffer() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let (width, height, depth) = (1024, 1024, 1);
        let size = Extent3D { width, height, depth };

        let buffer1 = device.create_buffer(&BufferDescriptor {
            size: (width * height) as usize * std::mem::size_of::<f32>(),
            usage: BufferUsageFlags::TRANSFER_DST,
        })?;

        let texture1 = device.create_texture(&TextureDescriptor {
            usage: TextureUsageFlags::TRANSFER_SRC,
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            dimension: TextureDimension::D2,
            size,
            array_layer_count: 1,
            mip_level_count: 1,
        })?;

        let dst = BufferCopyView {
            buffer: &buffer1,
            row_length: width, // TODO row_pitch
            image_height: height,
            offset: 0,
        };

        let src = TextureCopyView {
            texture: &texture1,
            mip_level: 0,
            array_layer: 0,
            origin: Origin3D { x: 0, y: 0, z: 0 },
        };

        let mut encoder = device.create_command_encoder(None)?;

        encoder.copy_texture_to_buffer(src, dst, size);

        let command_buffers = &[encoder.finish()?];

        let queue = device.get_queue();

        queue.submit(command_buffers)?;

        // TODO: Verification

        // Submitting twice isn't necessary, but this helps catch issues with subresource tracking
        queue.submit(command_buffers)?;

        Ok(instance)
    })
}
