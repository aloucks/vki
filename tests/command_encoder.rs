use vki::{
    Color, Extent3D, LoadOp, RenderPassColorAttachmentDescriptor, RenderPassDescriptor, StoreOp, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsageFlags,
};

pub mod support;

#[test]
fn create_command_encoder() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let mut command_encoder = device.create_command_encoder(None)?;

        let compute_pass = command_encoder.begin_compute_pass();
        compute_pass.end_pass();

        let texture = device.create_texture(&TextureDescriptor {
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
            mip_level_count: 1,
            dimension: TextureDimension::D2,
            array_layer_count: 1,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
        })?;

        let texture_view = texture.create_default_view()?;

        let render_pass = command_encoder.begin_render_pass(RenderPassDescriptor {
            color_attachments: &[RenderPassColorAttachmentDescriptor {
                attachment: &texture_view,
                resolve_target: None,
                load_op: LoadOp::Clear,
                store_op: StoreOp::Store,
                clear_color: Color {
                    r: 0.2,
                    g: 0.2,
                    b: 0.2,
                    a: 1.0,
                },
            }],
            depth_stencil_attachment: None,
        });
        render_pass.end_pass();

        let _command_buffer = command_encoder.finish()?;

        Ok(instance)
    });
}

#[test]
fn submit_command_buffer() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let mut command_encoder = device.create_command_encoder(None)?;

        let compute_pass = command_encoder.begin_compute_pass();
        compute_pass.end_pass();

        let texture = device.create_texture(&TextureDescriptor {
            sample_count: 1,
            format: TextureFormat::R8G8B8A8Unorm,
            usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
            mip_level_count: 1,
            dimension: TextureDimension::D2,
            array_layer_count: 1,
            size: Extent3D {
                width: 1024,
                height: 1024,
                depth: 1,
            },
        })?;

        let texture_view = texture.create_default_view()?;

        let render_pass = command_encoder.begin_render_pass(RenderPassDescriptor {
            color_attachments: &[RenderPassColorAttachmentDescriptor {
                attachment: &texture_view,
                resolve_target: None,
                load_op: LoadOp::Clear,
                store_op: StoreOp::Store,
                clear_color: Color {
                    r: 0.2,
                    g: 0.2,
                    b: 0.2,
                    a: 1.0,
                },
            }],
            depth_stencil_attachment: None,
        });
        render_pass.end_pass();

        let command_buffer = command_encoder.finish()?;

        let queue = device.get_queue();

        queue.submit(&[command_buffer])?;

        Ok(instance)
    });
}
