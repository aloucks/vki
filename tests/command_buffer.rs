use std::time::Duration;
use vki::{BufferDescriptor, ShaderStageFlags, BufferUsageFlags, ComputePipelineDescriptor, PipelineStageDescriptor, ShaderModuleDescriptor, PipelineLayoutDescriptor, BindGroupLayoutDescriptor, BindGroupLayoutBinding, BindingType, BindGroupDescriptor, BindGroupBinding, BindingResource};
use std::borrow::Cow;

pub mod support;

#[test]
fn copy_buffer_with_compute_shader() {
    vki::validate(|| {
        let (instance, _adapter, device) = support::init()?;

        let compute_module = device.create_shader_module(ShaderModuleDescriptor {
            code: Cow::Borrowed(include_bytes!("shaders/command_buffer.copy_buffer_with_compute_shader.comp.spv"))
        })?;

        let bind_group_layout = device.create_bind_group_layout(BindGroupLayoutDescriptor {
            bindings: &[
                BindGroupLayoutBinding {
                    binding: 0,
                    visibility: ShaderStageFlags::COMPUTE,
                    binding_type: BindingType::StorageBuffer,
                },
                BindGroupLayoutBinding {
                    binding: 1,
                    visibility: ShaderStageFlags::COMPUTE,
                    binding_type: BindingType::StorageBuffer,
                }
            ]
        })?;

        let pipeline_layout = device.create_pipeline_layout(PipelineLayoutDescriptor {
            bind_group_layouts: vec![
                bind_group_layout.clone()
            ]
        })?;

        let pipeline = device.create_compute_pipeline(ComputePipelineDescriptor {
            compute_stage: PipelineStageDescriptor {
                entry_point: Cow::Borrowed("main"),
                module: compute_module
            },
            layout: pipeline_layout,
        })?;

        let mut encoder = device.create_command_encoder()?;

        let data: &[[f32; 4]] = &[
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ];
        let data_byte_size = std::mem::size_of::<[f32; 4]>() * data.len();
        let data_byte_size = data_byte_size as u64;

        let write_buffer = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsageFlags::MAP_WRITE | BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::STORAGE,
            size: data_byte_size,
        })?;

        let read_buffer = device.create_buffer_mapped(BufferDescriptor {
            usage: BufferUsageFlags::MAP_READ | BufferUsageFlags::TRANSFER_DST | BufferUsageFlags::STORAGE,
            size: data_byte_size,
        })?;

        let bind_group = device.create_bind_group(BindGroupDescriptor {
            layout: bind_group_layout,
            bindings: vec![
                BindGroupBinding {
                    binding: 0,
                    resource: BindingResource::Buffer(write_buffer.buffer(), 0..data_byte_size)
                },
                BindGroupBinding {
                    binding: 1,
                    resource: BindingResource::Buffer(read_buffer.buffer(), 0..data_byte_size)
                }
            ]
        })?;

        write_buffer.write(0, data)?;

        let mut compute_pass = encoder.begin_compute_pass();
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, None);
        compute_pass.dispatch(4, 1, 1);
        compute_pass.end_pass();

        let fence = device.create_fence()?;

        let queue = device.get_queue();
        queue.submit(encoder.finish()?)?;

        fence.wait(Duration::from_millis(1_000_000_000))?;

        let read: &[[f32; 4]] = read_buffer.read(0, data.len())?;
        assert_eq!(data, read);

        Ok(instance)
    });
}
