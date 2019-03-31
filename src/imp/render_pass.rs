use ash::version::DeviceV1_0;
use ash::vk;

use smallvec::SmallVec;

use crate::imp::texture;
use crate::{LoadOp, TextureFormat};

use crate::imp::DeviceInner;

use std::collections::HashMap;
use std::ptr;

pub const MAX_COLOR_ATTACHMENTS: usize = 4;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct DepthStencilInfo {
    pub format: TextureFormat,
    pub depth_load_op: LoadOp,
    pub stencil_load_op: LoadOp,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ColorInfo {
    pub format: TextureFormat,
    pub load_op: LoadOp,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct RenderPassCacheQuery {
    color: SmallVec<[ColorInfo; MAX_COLOR_ATTACHMENTS]>,
    depth_stencil: Option<DepthStencilInfo>,
}

impl RenderPassCacheQuery {
    pub fn new() -> RenderPassCacheQuery {
        RenderPassCacheQuery::default()
    }

    pub fn add_color(&mut self, color_info: ColorInfo) {
        self.color.push(color_info);
    }

    //    pub fn with_color(mut self, color_info: ColorInfo) -> RenderPassCacheQuery {
    //        self.add_color(color_info);
    //        self
    //    }

    pub fn set_depth_stencil(&mut self, depth_stencil_info: DepthStencilInfo) {
        self.depth_stencil = Some(depth_stencil_info);
    }

    //    pub fn with_depth_stencil(mut self, depth_stencil_info: DepthStencilInfo) -> RenderPassCacheQuery {
    //        self.set_depth_stencil(depth_stencil_info);
    //        self
    //    }
}

pub fn attachment_load_op(op: LoadOp) -> vk::AttachmentLoadOp {
    match op {
        LoadOp::Clear => vk::AttachmentLoadOp::CLEAR,
        LoadOp::Load => vk::AttachmentLoadOp::LOAD,
    }
}

#[derive(Debug, Default)]
pub struct RenderPassCache {
    cache: HashMap<RenderPassCacheQuery, vk::RenderPass>,
}

pub fn color_attachment_reference(attachment: u32) -> vk::AttachmentReference {
    vk::AttachmentReference {
        attachment,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }
}

pub fn color_attachment_description(color_info: ColorInfo) -> vk::AttachmentDescription {
    vk::AttachmentDescription {
        flags: vk::AttachmentDescriptionFlags::empty(),
        format: texture::image_format(color_info.format),
        samples: vk::SampleCountFlags::TYPE_1,
        load_op: attachment_load_op(color_info.load_op),
        store_op: vk::AttachmentStoreOp::STORE,
        initial_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        ..Default::default()
    }
}

pub fn depth_stencil_attachment_reference(attachment: u32) -> vk::AttachmentReference {
    vk::AttachmentReference {
        attachment,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    }
}

pub fn depth_stencil_attachment_description(depth_stencil_info: &DepthStencilInfo) -> vk::AttachmentDescription {
    vk::AttachmentDescription {
        flags: vk::AttachmentDescriptionFlags::empty(),
        format: texture::image_format(depth_stencil_info.format),
        samples: vk::SampleCountFlags::TYPE_1,
        load_op: attachment_load_op(depth_stencil_info.depth_load_op),
        store_op: vk::AttachmentStoreOp::STORE,
        stencil_load_op: attachment_load_op(depth_stencil_info.stencil_load_op),
        stencil_store_op: vk::AttachmentStoreOp::STORE,
        initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    }
}

impl RenderPassCache {
    pub fn get_render_pass(
        &mut self,
        query: RenderPassCacheQuery,
        device: &DeviceInner,
    ) -> Result<vk::RenderPass, vk::Result> {
        if let Some(handle) = self.cache.get(&query).cloned() {
            Ok(handle)
        } else {
            self.create_render_pass(query, device)
        }
    }

    fn create_render_pass(
        &mut self,
        query: RenderPassCacheQuery,
        device: &DeviceInner,
    ) -> Result<vk::RenderPass, vk::Result> {
        let mut attachment_descriptions = query
            .color
            .iter()
            .map(|color_info| color_attachment_description(*color_info))
            .collect::<SmallVec<[vk::AttachmentDescription; MAX_COLOR_ATTACHMENTS + 1]>>();

        let color_attachments = query
            .color
            .iter()
            .enumerate()
            .map(|(attachment, _)| color_attachment_reference(attachment as u32))
            .collect::<SmallVec<[vk::AttachmentReference; MAX_COLOR_ATTACHMENTS]>>();

        let color_attachment_count = color_attachments.len() as u32;

        let mut depth_stencil_attachment = None;

        if let Some(ref depth_stencil_info) = query.depth_stencil {
            let attachment = color_attachment_count;
            attachment_descriptions.push(depth_stencil_attachment_description(depth_stencil_info));
            depth_stencil_attachment = Some(depth_stencil_attachment_reference(attachment));
        }

        let depth_stencil_attachment_ptr = depth_stencil_attachment
            .as_ref()
            .map(|v| v as *const _)
            .unwrap_or_else(ptr::null);

        let subpass_description = vk::SubpassDescription {
            flags: vk::SubpassDescriptionFlags::empty(),
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count,
            p_color_attachments: color_attachments.as_ptr(),
            p_depth_stencil_attachment: depth_stencil_attachment_ptr,
            ..Default::default()
        };

        debug_assert_eq!(
            attachment_descriptions.len(),
            color_attachments.len() + depth_stencil_attachment.map(|_| 1).unwrap_or(0)
        );

        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachment_descriptions)
            .subpasses(&[subpass_description])
            .build();

        let handle = unsafe { device.raw.create_render_pass(&create_info, None)? };

        self.cache.insert(query, handle);

        Ok(handle)
    }

    pub fn drain(&mut self, device: &DeviceInner) {
        for (_, handle) in self.cache.drain() {
            unsafe {
                device.raw.destroy_render_pass(handle, None);
            }
        }
    }
}

impl Drop for RenderPassCache {
    fn drop(&mut self) {
        if !self.cache.is_empty() {
            log::error!("RenderPassCache dropped without being drained")
        }
    }
}
