use ash::version::DeviceV1_0;
use ash::vk;

use std::ffi::CString;
use std::sync::Arc;

use crate::error::Error;
use crate::imp::fenced_deleter::DeleteWhenUnused;
use crate::imp::render_pass::{self, ColorInfo, DepthStencilInfo, RenderPassCacheQuery};
use crate::imp::{binding, sampler};
use crate::imp::{ComputePipelineInner, DeviceInner, PipelineLayoutInner, RenderPipelineInner};
use crate::{
    BlendFactor, BlendOperation, ColorStateDescriptor, ColorWriteFlags, CompareFunction, ComputePipeline,
    ComputePipelineDescriptor, CullMode, DepthStencilStateDescriptor, FrontFace, InputStepMode, LoadOp, PipelineLayout,
    PipelineLayoutDescriptor, PrimitiveTopology, RasterizationStateDescriptor, RenderPipeline,
    RenderPipelineDescriptor, StencilOperation, StencilStateFaceDescriptor, TextureFormat, VertexAttributeDescriptor,
    VertexFormat, VertexInputDescriptor,
};

pub const MAX_PUSH_CONSTANTS_SIZE: usize = 128;

impl PipelineLayoutInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: PipelineLayoutDescriptor) -> Result<PipelineLayoutInner, Error> {
        let push_constant_ranges: Vec<_> = descriptor
            .push_constant_ranges
            .iter()
            .map(|r| vk::PushConstantRange {
                offset: r.offset as _,
                size: r.size as _,
                stage_flags: binding::shader_stage_flags(r.stages),
            })
            .collect();

        let descriptor_set_layouts: Vec<_> = descriptor
            .bind_group_layouts
            .iter()
            .map(|bind_group_layout| bind_group_layout.inner.handle)
            .collect();

        let create_info = vk::PipelineLayoutCreateInfo::builder()
            .push_constant_ranges(&push_constant_ranges)
            .set_layouts(&descriptor_set_layouts)
            .build();

        let handle = unsafe { device.raw.create_pipeline_layout(&create_info, None)? };

        Ok(PipelineLayoutInner {
            handle,
            device,
            descriptor,
        })
    }
}

impl Into<PipelineLayout> for PipelineLayoutInner {
    fn into(self) -> PipelineLayout {
        PipelineLayout { inner: Arc::new(self) }
    }
}

impl Drop for PipelineLayoutInner {
    fn drop(&mut self) {
        let mut state = self.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}

impl ComputePipelineInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: ComputePipelineDescriptor) -> Result<ComputePipelineInner, Error> {
        // TODO: inspect push constants

        let entry_point = CString::new(&*descriptor.compute_stage.entry_point).map_err(|e| {
            log::error!("invalid entry point: {:?}", e);
            vk::Result::ERROR_VALIDATION_FAILED_EXT
        })?;

        let create_info = vk::ComputePipelineCreateInfo {
            layout: descriptor.layout.inner.handle,
            base_pipeline_handle: vk::Pipeline::null(),
            base_pipeline_index: -1,
            stage: vk::PipelineShaderStageCreateInfo::builder()
                .name(entry_point.as_c_str())
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(descriptor.compute_stage.module.inner.handle)
                .build(),
            ..Default::default()
        };

        let pipeline_cache = vk::PipelineCache::null();
        let mut handle = vk::Pipeline::null();

        unsafe {
            device.raw.fp_v1_0().create_compute_pipelines(
                device.raw.handle(),
                pipeline_cache,
                1,
                &create_info,
                std::ptr::null(),
                &mut handle,
            )
        };

        let layout = descriptor.layout.inner.clone();

        Ok(ComputePipelineInner { handle, layout })
    }
}

impl Into<ComputePipeline> for ComputePipelineInner {
    fn into(self) -> ComputePipeline {
        ComputePipeline { inner: Arc::new(self) }
    }
}

impl Drop for ComputePipelineInner {
    fn drop(&mut self) {
        let mut state = self.layout.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
    }
}

pub fn primitive_topology(primitive: PrimitiveTopology) -> vk::PrimitiveTopology {
    match primitive {
        PrimitiveTopology::LineList => vk::PrimitiveTopology::LINE_LIST,
        PrimitiveTopology::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
        PrimitiveTopology::PointList => vk::PrimitiveTopology::POINT_LIST,
        PrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
        PrimitiveTopology::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
    }
}

pub fn blend_factor(blend_factor: BlendFactor) -> vk::BlendFactor {
    match blend_factor {
        BlendFactor::Zero => vk::BlendFactor::ZERO,
        BlendFactor::One => vk::BlendFactor::ONE,
        BlendFactor::SrcColor => vk::BlendFactor::SRC_COLOR,
        BlendFactor::OneMinusSrcColor => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
        BlendFactor::SrcAlpha => vk::BlendFactor::SRC_ALPHA,
        BlendFactor::OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        BlendFactor::DstColor => vk::BlendFactor::DST_COLOR,
        BlendFactor::OneMinusDstColor => vk::BlendFactor::ONE_MINUS_DST_COLOR,
        BlendFactor::DstAlpha => vk::BlendFactor::DST_ALPHA,
        BlendFactor::OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
        BlendFactor::BlendColor => vk::BlendFactor::CONSTANT_COLOR,
        BlendFactor::OneMinusBlendColor => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
        BlendFactor::SrcAlphaSaturated => vk::BlendFactor::SRC_ALPHA_SATURATE,
    }
}

pub fn blend_operation(operation: BlendOperation) -> vk::BlendOp {
    match operation {
        BlendOperation::Add => vk::BlendOp::ADD,
        BlendOperation::Subtract => vk::BlendOp::SUBTRACT,
        BlendOperation::ReverseSubtract => vk::BlendOp::REVERSE_SUBTRACT,
        BlendOperation::Min => vk::BlendOp::MIN,
        BlendOperation::Max => vk::BlendOp::MAX,
    }
}

pub fn color_write_mask(color: ColorWriteFlags) -> vk::ColorComponentFlags {
    let mut flags = vk::ColorComponentFlags::empty();

    if color.intersects(ColorWriteFlags::RED) {
        flags |= vk::ColorComponentFlags::R;
    }

    if color.intersects(ColorWriteFlags::GREEN) {
        flags |= vk::ColorComponentFlags::G;
    }

    if color.intersects(ColorWriteFlags::BLUE) {
        flags |= vk::ColorComponentFlags::B;
    }

    if color.intersects(ColorWriteFlags::ALPHA) {
        flags |= vk::ColorComponentFlags::A;
    }

    flags
}

#[rustfmt::skip]
pub fn blend_enabled(descriptor: &ColorStateDescriptor) -> bool {
    descriptor.alpha_blend.operation  != BlendOperation::Add ||
    descriptor.alpha_blend.src_factor != BlendFactor::One ||
    descriptor.alpha_blend.dst_factor != BlendFactor::Zero ||
    descriptor.color_blend.operation  != BlendOperation::Add ||
    descriptor.color_blend.src_factor != BlendFactor::One ||
    descriptor.color_blend.dst_factor != BlendFactor::Zero
}

#[rustfmt::skip]
pub fn stencil_test_enabled(descriptor: &DepthStencilStateDescriptor) -> bool {
    descriptor.stencil_back.compare != CompareFunction::Always ||
    descriptor.stencil_back.fail_op != StencilOperation::Keep ||
    descriptor.stencil_back.depth_fail_op != StencilOperation::Keep ||
    descriptor.stencil_back.pass_op != StencilOperation::Keep ||
    descriptor.stencil_front.compare != CompareFunction::Always ||
    descriptor.stencil_front.fail_op != StencilOperation::Keep ||
    descriptor.stencil_front.depth_fail_op != StencilOperation::Keep ||
    descriptor.stencil_front.pass_op != StencilOperation::Keep
}

pub fn stencil_op(operation: StencilOperation) -> vk::StencilOp {
    match operation {
        StencilOperation::Keep => vk::StencilOp::KEEP,
        StencilOperation::Zero => vk::StencilOp::ZERO,
        StencilOperation::Replace => vk::StencilOp::REPLACE,
        StencilOperation::IncrementClamp => vk::StencilOp::INCREMENT_AND_CLAMP,
        StencilOperation::DecrementClamp => vk::StencilOp::DECREMENT_AND_CLAMP,
        StencilOperation::Invert => vk::StencilOp::INVERT,
        StencilOperation::IncrementWrap => vk::StencilOp::INCREMENT_AND_WRAP,
        StencilOperation::DecrementWrap => vk::StencilOp::DECREMENT_AND_WRAP,
    }
}

pub fn depth_test_enabled(descriptor: &DepthStencilStateDescriptor) -> bool {
    // Dawn and WGPU have conflicting implementations for enabling depth test.
    // This follows the WGPU implementation which seems correct.
    descriptor.depth_compare != CompareFunction::Always || descriptor.depth_write_enabled
}

pub fn disable_depth_stencil_test() -> DepthStencilStateDescriptor {
    DepthStencilStateDescriptor {
        format: TextureFormat::D32FloatS8Uint,
        depth_write_enabled: false,
        depth_compare: CompareFunction::Always,
        stencil_front: StencilStateFaceDescriptor {
            compare: CompareFunction::Always,
            depth_fail_op: StencilOperation::Keep,
            fail_op: StencilOperation::Keep,
            pass_op: StencilOperation::Keep,
        },
        stencil_back: StencilStateFaceDescriptor {
            compare: CompareFunction::Always,
            depth_fail_op: StencilOperation::Keep,
            fail_op: StencilOperation::Keep,
            pass_op: StencilOperation::Keep,
        },
        stencil_read_mask: 0,
        stencil_write_mask: 0,
    }
}

pub fn depth_stencil_state_create_info(
    descriptor: DepthStencilStateDescriptor,
) -> vk::PipelineDepthStencilStateCreateInfo {
    vk::PipelineDepthStencilStateCreateInfo {
        p_next: std::ptr::null(),
        s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
        flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
        depth_test_enable: depth_test_enabled(&descriptor) as vk::Bool32,
        depth_write_enable: descriptor.depth_write_enabled as vk::Bool32,
        depth_compare_op: sampler::compare_op(descriptor.depth_compare),
        depth_bounds_test_enable: vk::FALSE,
        min_depth_bounds: 0.0,
        max_depth_bounds: 1.0,
        stencil_test_enable: stencil_test_enabled(&descriptor) as vk::Bool32,
        front: vk::StencilOpState {
            fail_op: stencil_op(descriptor.stencil_front.fail_op),
            pass_op: stencil_op(descriptor.stencil_front.pass_op),
            depth_fail_op: stencil_op(descriptor.stencil_front.depth_fail_op),
            compare_op: sampler::compare_op(descriptor.stencil_front.compare),
            compare_mask: descriptor.stencil_read_mask,
            write_mask: descriptor.stencil_write_mask,
            reference: 0,
        },
        back: vk::StencilOpState {
            fail_op: stencil_op(descriptor.stencil_back.fail_op),
            pass_op: stencil_op(descriptor.stencil_back.pass_op),
            depth_fail_op: stencil_op(descriptor.stencil_back.depth_fail_op),
            compare_op: sampler::compare_op(descriptor.stencil_back.compare),
            compare_mask: descriptor.stencil_read_mask,
            write_mask: descriptor.stencil_write_mask,
            reference: 0,
        },
    }
}

pub fn rasterization_state_create_info(
    descriptor: &RasterizationStateDescriptor,
) -> vk::PipelineRasterizationStateCreateInfo {
    vk::PipelineRasterizationStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: vk::PipelineRasterizationStateCreateFlags::empty(),
        depth_clamp_enable: vk::FALSE,
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode: vk::PolygonMode::FILL,
        cull_mode: cull_mode(descriptor.cull_mode),
        front_face: front_face(descriptor.front_face),
        depth_bias_enable: (descriptor.depth_bias != 0) as vk::Bool32,
        depth_bias_clamp: descriptor.depth_bias_clamp,
        depth_bias_slope_factor: descriptor.depth_bias_slope_scale,
        depth_bias_constant_factor: descriptor.depth_bias as f32,
        line_width: 1.0,
    }
}

pub fn cull_mode(mode: CullMode) -> vk::CullModeFlags {
    match mode {
        CullMode::Back => vk::CullModeFlags::BACK,
        CullMode::Front => vk::CullModeFlags::FRONT,
        CullMode::None => vk::CullModeFlags::NONE,
    }
}

pub fn front_face(face: FrontFace) -> vk::FrontFace {
    match face {
        FrontFace::Ccw => vk::FrontFace::COUNTER_CLOCKWISE,
        FrontFace::Cw => vk::FrontFace::CLOCKWISE,
    }
}

pub fn color_blend_attachment_state(descriptor: &ColorStateDescriptor) -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        blend_enable: blend_enabled(&descriptor) as vk::Bool32,
        src_color_blend_factor: blend_factor(descriptor.color_blend.src_factor),
        dst_color_blend_factor: blend_factor(descriptor.color_blend.dst_factor),
        color_blend_op: blend_operation(descriptor.color_blend.operation),
        src_alpha_blend_factor: blend_factor(descriptor.alpha_blend.src_factor),
        dst_alpha_blend_factor: blend_factor(descriptor.alpha_blend.dst_factor),
        alpha_blend_op: blend_operation(descriptor.alpha_blend.operation),
        color_write_mask: color_write_mask(descriptor.write_mask),
    }
}

pub fn vertex_format(format: VertexFormat) -> vk::Format {
    match format {
        VertexFormat::UChar2 => vk::Format::R8G8_UINT,
        VertexFormat::UChar4 => vk::Format::R8G8B8A8_UINT,
        VertexFormat::Char2 => vk::Format::R8G8_SINT,
        VertexFormat::Char4 => vk::Format::R8G8B8A8_SINT,
        VertexFormat::UChar2Norm => vk::Format::R8G8_UNORM,
        VertexFormat::UChar4Norm => vk::Format::R8G8B8A8_UNORM,
        VertexFormat::Char2Norm => vk::Format::R8G8_SNORM,
        VertexFormat::Char4Norm => vk::Format::R8G8B8A8_SNORM,

        VertexFormat::UShort2 => vk::Format::R16G16_UINT,
        VertexFormat::UShort4 => vk::Format::R16G16B16A16_UINT,
        VertexFormat::Short2 => vk::Format::R16G16_SINT,
        VertexFormat::Short4 => vk::Format::R16G16B16A16_SINT,
        VertexFormat::UShort2Norm => vk::Format::R16G16_UNORM,
        VertexFormat::UShort4Norm => vk::Format::R16G16B16A16_UNORM,
        VertexFormat::Short2Norm => vk::Format::R16G16_SNORM,
        VertexFormat::Short4Norm => vk::Format::R16G16B16A16_SNORM,

        VertexFormat::Half2 => vk::Format::R16G16_SFLOAT,
        VertexFormat::Half4 => vk::Format::R16G16B16A16_SFLOAT,
        VertexFormat::Float => vk::Format::R32_SFLOAT,
        VertexFormat::Float2 => vk::Format::R32G32_SFLOAT,
        VertexFormat::Float3 => vk::Format::R32G32B32_SFLOAT,
        VertexFormat::Float4 => vk::Format::R32G32B32A32_SFLOAT,

        VertexFormat::UInt => vk::Format::R32_UINT,
        VertexFormat::UInt2 => vk::Format::R32G32_UINT,
        VertexFormat::UInt3 => vk::Format::R32G32B32_UINT,
        VertexFormat::UInt4 => vk::Format::R32G32B32A32_UINT,
        VertexFormat::Int => vk::Format::R32_SINT,
        VertexFormat::Int2 => vk::Format::R32G32_SINT,
        VertexFormat::Int3 => vk::Format::R32G32B32_SINT,
        VertexFormat::Int4 => vk::Format::R32G32B32A32_SINT,
    }
}

pub fn input_rate(mode: InputStepMode) -> vk::VertexInputRate {
    match mode {
        InputStepMode::Instance => vk::VertexInputRate::INSTANCE,
        InputStepMode::Vertex => vk::VertexInputRate::VERTEX,
    }
}

pub fn vertex_input_attribute_description(
    descriptor: &VertexAttributeDescriptor,
) -> vk::VertexInputAttributeDescription {
    vk::VertexInputAttributeDescription {
        format: vertex_format(descriptor.format),
        binding: descriptor.input_slot,
        offset: descriptor.offset,
        location: descriptor.shader_location,
    }
}

pub fn vertex_input_binding_description(descriptor: &VertexInputDescriptor) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
        binding: descriptor.input_slot,
        stride: descriptor.stride as u32,
        input_rate: input_rate(descriptor.step_mode),
    }
}

impl RenderPipelineInner {
    pub fn new(device: Arc<DeviceInner>, descriptor: RenderPipelineDescriptor) -> Result<RenderPipelineInner, Error> {
        // TODO: inspect push constants

        let vertex_entry_point = CString::new(&*descriptor.vertex_stage.entry_point).map_err(|e| {
            log::error!("invalid vertex entry point: {:?}", e);
            vk::Result::ERROR_VALIDATION_FAILED_EXT
        })?;

        let fragment_entry_point = CString::new(&*descriptor.fragment_stage.entry_point).map_err(|e| {
            log::error!("invalid fragment entry point: {:?}", e);
            vk::Result::ERROR_VALIDATION_FAILED_EXT
        })?;

        let shader_stages_create_info = &[
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: descriptor.vertex_stage.module.inner.handle,
                p_name: vertex_entry_point.as_ptr(),
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: descriptor.fragment_stage.module.inner.handle,
                p_name: fragment_entry_point.as_ptr(),
                ..Default::default()
            },
        ];

        let input_assembly_state_create_info = vk::PipelineInputAssemblyStateCreateInfo {
            topology: primitive_topology(descriptor.primitive_topology),
            // Dawn notes that this must always be enabled because of Metal
            primitive_restart_enable: vk::TRUE,
            ..Default::default()
        };

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D { width: 1, height: 1 },
        };

        let viewport_state_create_info = vk::PipelineViewportStateCreateInfo::builder()
            .scissors(&[scissor])
            .viewports(&[viewport])
            .build();

        let multisample_state_create_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(render_pass::sample_count(descriptor.sample_count)?)
            .build();

        let depth_stencil_state_create_info = depth_stencil_state_create_info(
            descriptor
                .depth_stencil_state
                .unwrap_or_else(disable_depth_stencil_test),
        );

        let rasterization_state_create_info = rasterization_state_create_info(&descriptor.rasterization_state);

        let color_blend_attachment_states: Vec<vk::PipelineColorBlendAttachmentState> = descriptor
            .color_states
            .iter()
            .map(color_blend_attachment_state)
            .collect();

        let color_blend_state_create_info = vk::PipelineColorBlendStateCreateInfo {
            logic_op_enable: vk::FALSE,
            logic_op: vk::LogicOp::CLEAR,
            p_attachments: color_blend_attachment_states.as_ptr(),
            attachment_count: color_blend_attachment_states.len() as u32,
            blend_constants: [0.0, 0.0, 0.0, 0.0], // dummy values
            ..Default::default()
        };

        let vertex_attribute_descriptions: Vec<vk::VertexInputAttributeDescription> = descriptor
            .input_state
            .attributes
            .iter()
            .map(vertex_input_attribute_description)
            .collect();

        let vertex_binding_descriptions: Vec<vk::VertexInputBindingDescription> = descriptor
            .input_state
            .inputs
            .iter()
            .map(vertex_input_binding_description)
            .collect();

        let vertex_input_state_create_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .build();

        let dynamic_states = &[
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
            vk::DynamicState::LINE_WIDTH,
            vk::DynamicState::DEPTH_BIAS,
            vk::DynamicState::BLEND_CONSTANTS,
            vk::DynamicState::DEPTH_BOUNDS,
            vk::DynamicState::STENCIL_REFERENCE,
        ];

        let dynamic_state_create_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(dynamic_states)
            .build();

        let mut query = RenderPassCacheQuery::new();

        query.set_sample_count(descriptor.sample_count);

        for color_state_info in descriptor.color_states.iter() {
            query.add_color(ColorInfo {
                load_op: LoadOp::Load,
                format: color_state_info.format,
                // TODO: Should has_resolve_target default to true when sample_count > 1?
                // https://www.khronos.org/registry/vulkan/specs/1.1/html/chap7.html#renderpass-compatibility
                // Dawn sets this to `false`, presumably because render passes are still considered compatible
                // when they have differing numbers of attachments, as long as the corresponding attachments
                // are compatible.
                has_resolve_target: false,
            });
        }

        if let Some(ref depth_stencil_state) = descriptor.depth_stencil_state {
            query.set_depth_stencil(DepthStencilInfo {
                format: depth_stencil_state.format,
                depth_load_op: LoadOp::Load,
                stencil_load_op: LoadOp::Load,
            });
        }

        let render_pass = { device.state.lock().get_render_pass(query, &device)? };

        let create_info = vk::GraphicsPipelineCreateInfo::builder()
            .layout(descriptor.layout.inner.handle)
            .render_pass(render_pass)
            .stages(shader_stages_create_info)
            .vertex_input_state(&vertex_input_state_create_info)
            .input_assembly_state(&input_assembly_state_create_info)
            .viewport_state(&viewport_state_create_info)
            .rasterization_state(&rasterization_state_create_info)
            .multisample_state(&multisample_state_create_info)
            .depth_stencil_state(&depth_stencil_state_create_info)
            .color_blend_state(&color_blend_state_create_info)
            .dynamic_state(&dynamic_state_create_info)
            .base_pipeline_handle(vk::Pipeline::null())
            .base_pipeline_index(-1)
            .build();

        let pipeline_cache = vk::PipelineCache::null();

        let mut handle = vk::Pipeline::null();

        unsafe {
            device.raw.fp_v1_0().create_graphics_pipelines(
                device.raw.handle(),
                pipeline_cache,
                1,
                &create_info,
                std::ptr::null(),
                &mut handle,
            )
        };

        let layout = descriptor.layout.inner.clone();

        Ok(RenderPipelineInner {
            handle,
            layout,
            descriptor,
        })
    }
}

impl Into<RenderPipeline> for RenderPipelineInner {
    fn into(self) -> RenderPipeline {
        RenderPipeline { inner: Arc::new(self) }
    }
}

impl Drop for RenderPipelineInner {
    fn drop(&mut self) {
        let mut state = self.layout.device.state.lock();
        let serial = state.get_next_pending_serial();
        state.get_fenced_deleter().delete_when_unused(self.handle, serial);
        // TODO: What about the render pass?
    }
}
