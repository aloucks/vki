#[macro_use]
extern crate memoffset;

pub mod util;

use cgmath::{
    EuclideanSpace, InnerSpace, Matrix, Matrix4, Point3, Quaternion, SquareMatrix, Vector1, Vector3, VectorSpace,
};

use crate::util::{App, EventHandlers};

use std::collections::{HashMap, HashSet};
use std::path::Path;

use smallvec::SmallVec;

use std::borrow::Cow;

use std::time::{Duration, Instant};
use vki::{
    AddressMode, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource,
    BindingType, BlendDescriptor, Buffer, BufferUsage, Color, ColorStateDescriptor, ColorWrite, CompareFunction,
    CullMode, DepthStencilStateDescriptor, FilterMode, FrontFace, IndexFormat, InputStepMode, LoadOp,
    PipelineLayoutDescriptor, PipelineStageDescriptor, PrimitiveTopology, PushConstantRange,
    RasterizationStateDescriptor, RenderPassColorAttachmentDescriptor, RenderPassDepthStencilAttachmentDescriptor,
    RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerDescriptor, ShaderModuleDescriptor, ShaderStage,
    StencilStateFaceDescriptor, StoreOp, SwapchainError, TextureFormat, TextureView, VertexAttributeDescriptor,
    VertexBufferLayoutDescriptor, VertexFormat, VertexStateDescriptor,
};

const MAX_MORPH_TARGETS: usize = 2;
const MAX_JOINTS: usize = 128;

fn mat4(mat: &Matrix4<f32>) -> &[[f32; 4]; 4] {
    mat.as_ref()
}

struct Import {
    doc: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}

impl Import {
    fn load<P: AsRef<Path>>(path: P) -> gltf::Result<Import> {
        let (doc, buffers, images) = gltf::import(path)?;
        Ok(Import { doc, buffers, images })
    }
}

struct TextureSampler {
    view: TextureView,
    sampler: Sampler,
}

struct Node {
    #[allow(dead_code)]
    name: Option<String>,
    local_transform: gltf::scene::Transform,
    children_indices: Vec<usize>,
    mesh_index: Option<usize>,
    parent_node_index: Option<usize>,
    animate_local_translation: Option<[f32; 3]>,
    animate_local_rotation: Option<[f32; 4]>,
    animate_local_scale: Option<[f32; 3]>,
    animate_morph_weights: Option<Vec<f32>>,
    skin_index: Option<usize>,
    is_joint: bool,
}

impl Node {
    fn get_local_transform(&self) -> [[f32; 4]; 4] {
        let local_transform = self.local_transform.clone();
        if self.animate_local_translation.is_none()
            && self.animate_local_rotation.is_none()
            && self.animate_local_scale.is_none()
        {
            local_transform.matrix()
        } else {
            let (local_translation, local_rotation, local_scale) = local_transform.decomposed();
            let translation = self.animate_local_translation.unwrap_or(local_translation);
            let rotation = self.animate_local_rotation.unwrap_or(local_rotation);
            let scale = self.animate_local_scale.unwrap_or(local_scale);
            let decomposed = gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            };
            decomposed.matrix()
        }
    }

    fn get_global_transform(&self, nodes: &[Node]) -> [[f32; 4]; 4] {
        let mut parent_index_stack = SmallVec::<[usize; 32]>::new();
        let mut child_node = Some(self);
        while let Some(parent_node_index) = child_node.and_then(|node| node.parent_node_index) {
            parent_index_stack.push(parent_node_index);
            child_node = nodes.get(parent_node_index);
        }

        let mut global_transform = Matrix4::from(self.get_local_transform());
        for parent_node_index in parent_index_stack.iter().cloned() {
            let parent_local_transform = Matrix4::from(nodes[parent_node_index].get_local_transform());
            global_transform = parent_local_transform * global_transform;
        }

        global_transform.into()
    }

    fn get_morph_weights<'a>(&'a self, meshes: &'a [Mesh]) -> Option<&'a [f32]> {
        self.animate_morph_weights
            .as_ref()
            .map(|v| &*v)
            .or_else(|| {
                self.mesh_index
                    .and_then(|mesh_index| meshes[mesh_index].morph_weights.as_ref())
            })
            .map(|v| v.as_slice())
    }

    fn get_joint_matrices(&self, skins: &[Skin], nodes: &[Node]) -> Option<Vec<[[f32; 4]; 4]>> {
        match self.skin_index.map(|skin_index| &skins[skin_index]) {
            Some(skin) => {
                let model = self.get_global_transform(&nodes);
                let mut joints: Vec<[[f32; 4]; 4]> = Vec::with_capacity(skin.joint_node_indices.len());
                for joint_node_index in skin.joint_node_indices.iter().cloned() {
                    let inverse_node_global_transform = Matrix4::from(model)
                        .invert()
                        .expect("failed to invert global transform");
                    let joint_node = &nodes[joint_node_index];
                    let joint_global_transform = Matrix4::from(joint_node.get_global_transform(&nodes));
                    let inverse_bind_matrix = Matrix4::from(skin.inverse_bind_matrices[joints.len()]);
                    joints.push((inverse_node_global_transform * joint_global_transform * inverse_bind_matrix).into());
                }
                Some(joints)
            }
            None => None,
        }
    }
}

#[derive(Debug)]
pub struct PbrMetallicRoughness {
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<TextureInfo<()>>,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub metallic_roughness_texture: Option<TextureInfo<()>>,
}

impl Default for PbrMetallicRoughness {
    fn default() -> PbrMetallicRoughness {
        PbrMetallicRoughness {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
        }
    }
}

impl<'a> From<gltf::material::PbrMetallicRoughness<'a>> for PbrMetallicRoughness {
    fn from(v: gltf::material::PbrMetallicRoughness<'a>) -> PbrMetallicRoughness {
        PbrMetallicRoughness {
            base_color_factor: v.base_color_factor(),
            base_color_texture: v.base_color_texture().map(TextureInfo::<()>::from),
            metallic_factor: v.metallic_factor(),
            roughness_factor: v.roughness_factor(),
            metallic_roughness_texture: v.metallic_roughness_texture().map(TextureInfo::<()>::from),
        }
    }
}

#[derive(Debug, Default)]
pub struct TextureInfo<T> {
    pub texture_index: usize,
    pub image_index: usize,
    pub sampler_index: Option<usize>,
    pub texcoord_set: u32,
    pub info: T,
}

impl<'a> From<gltf::texture::Info<'a>> for TextureInfo<()> {
    fn from(v: gltf::texture::Info<'a>) -> TextureInfo<()> {
        let t = v.texture();
        let s = t.sampler();
        TextureInfo {
            texture_index: t.index(),
            image_index: t.source().index(),
            texcoord_set: v.tex_coord(),
            sampler_index: s.index(),
            info: (),
        }
    }
}

#[derive(Debug, Default)]
pub struct NormalInfo {
    pub scale: f32,
}

impl<'a> From<gltf::material::NormalTexture<'a>> for TextureInfo<NormalInfo> {
    fn from(v: gltf::material::NormalTexture<'a>) -> TextureInfo<NormalInfo> {
        let t = v.texture();
        let s = t.sampler();
        TextureInfo {
            texture_index: t.index(),
            image_index: t.source().index(),
            texcoord_set: v.tex_coord(),
            sampler_index: s.index(),
            info: NormalInfo { scale: v.scale() },
        }
    }
}

#[derive(Debug, Default)]
pub struct OcclusionInfo {
    pub strength: f32,
}

impl<'a> From<gltf::material::OcclusionTexture<'a>> for TextureInfo<OcclusionInfo> {
    fn from(v: gltf::material::OcclusionTexture<'a>) -> TextureInfo<OcclusionInfo> {
        let t = v.texture();
        let s = t.sampler();
        TextureInfo {
            texture_index: t.index(),
            image_index: t.source().index(),
            texcoord_set: v.tex_coord(),
            sampler_index: s.index(),
            info: OcclusionInfo { strength: v.strength() },
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Primitive {
    material_index: Option<usize>,
    bounding_box: BoundingBox,
    vertex_count: usize,
    vertex_buffer_offset: usize,
    index_buffer_offset: Option<usize>,
    index_count: usize,
    mesh_pipeline_key: MeshPipelineKey,
    settings: PrimitiveSettings,
}

pub struct Mesh {
    #[allow(dead_code)]
    name: Option<String>,
    primitives: Vec<Primitive>,
    morph_weights: Option<Vec<f32>>,
    node_index: usize,
}

struct Skin {
    #[allow(dead_code)]
    name: Option<String>,
    inverse_bind_matrices: Vec<[[f32; 4]; 4]>,
    joint_node_indices: Vec<usize>,
}

/// Material attributes that are baked into the pipeline state and
/// require a unique pipeline for each permutation.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaterialPipelineKey {
    color_blend: BlendDescriptor,
    alpha_blend: BlendDescriptor,
    cull_mode: CullMode,
    write_mask: ColorWrite,
    depth_write_enabled: bool,
}

/// Mesh attributes that are baked into the pipeline state and
/// require a unique pipeline for each permutation.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshPipelineKey {
    primitive_topology: PrimitiveTopology,
    index_format: Option<IndexFormat>,
}

#[derive(Debug, Default)]
pub struct Material {
    #[allow(dead_code)]
    pub name: Option<String>,
    pub alpha_cutoff: f32,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    pub pbr_metallic_roughness: PbrMetallicRoughness,
    pub normal_texture: Option<TextureInfo<NormalInfo>>,
    pub occlusion_texture: Option<TextureInfo<OcclusionInfo>>,
    pub emissive_texture: Option<TextureInfo<()>>,
    pub emissive_factor: [f32; 3],
}

impl Material {
    fn material_pipeline_key(&self) -> MaterialPipelineKey {
        MaterialPipelineKey {
            alpha_blend: match self.alpha_mode {
                AlphaMode::Opaque => BlendDescriptor::OPAQUE,
                AlphaMode::Blend => BlendDescriptor::BLEND,
                AlphaMode::Mask => BlendDescriptor::BLEND,
            },
            color_blend: match self.alpha_mode {
                AlphaMode::Opaque => BlendDescriptor::OPAQUE,
                AlphaMode::Blend => BlendDescriptor::BLEND,
                AlphaMode::Mask => BlendDescriptor::BLEND,
            },
            cull_mode: if self.double_sided {
                CullMode::None
            } else {
                CullMode::Back
            },
            write_mask: ColorWrite::ALL,
            depth_write_enabled: true,
        }
    }

    fn settings(&self) -> MaterialSettings {
        let alpha_blend = if self.alpha_mode != AlphaMode::Opaque { 1.0 } else { 0.0 };
        let alpha_cutoff = if self.alpha_mode == AlphaMode::Mask {
            self.alpha_cutoff
        } else {
            // The alpha_cutoff is used in the fragment shader to indicate if we're masking or not.
            0.0
        };

        MaterialSettings {
            pbr_base_color_factor: self.pbr_metallic_roughness.base_color_factor,
            pbr_metallic_factor: self.pbr_metallic_roughness.metallic_factor,
            pbr_roughness_factor: self.pbr_metallic_roughness.roughness_factor,
            emissive_factor: self.emissive_factor,

            alpha_blend,
            alpha_cutoff,

            normal_scale: self.normal_texture.as_ref().map(|t| t.info.scale).unwrap_or(0.0),
            occlusion_strength: self.occlusion_texture.as_ref().map(|t| t.info.strength).unwrap_or(0.0),

            base_color_texcoord_set: self
                .pbr_metallic_roughness
                .base_color_texture
                .as_ref()
                .map(|t| t.texcoord_set)
                .unwrap_or(0),
            metallic_roughness_texcoord_set: self
                .pbr_metallic_roughness
                .metallic_roughness_texture
                .as_ref()
                .map(|t| t.texcoord_set)
                .unwrap_or(0),
            occlusion_texcoord_set: self.occlusion_texture.as_ref().map(|t| t.texcoord_set).unwrap_or(0),
            emissive_texcoord_set: self.emissive_texture.as_ref().map(|t| t.texcoord_set).unwrap_or(0),
            normal_texcoord_set: self.normal_texture.as_ref().map(|t| t.texcoord_set).unwrap_or(0),

            has_base_color_map: self.pbr_metallic_roughness.base_color_texture.is_some() as _,
            has_metallic_roughness_map: self.pbr_metallic_roughness.metallic_roughness_texture.is_some() as _,
            has_occlusion_map: self.occlusion_texture.is_some() as _,
            has_emissive_map: self.emissive_texture.is_some() as _,
            has_normal_map: self.normal_texture.is_some() as _,
        }
    }
}

impl<'a> From<gltf::material::Material<'a>> for Material {
    fn from(v: gltf::material::Material) -> Material {
        let name = v.name().map(|s| s.to_owned());
        let alpha_cutoff = v.alpha_cutoff();
        let alpha_mode = v.alpha_mode().into();
        let double_sided = v.double_sided();
        let pbr_metallic_roughness = PbrMetallicRoughness::from(v.pbr_metallic_roughness());
        let normal_texture = v.normal_texture().map(TextureInfo::<NormalInfo>::from);
        let occlusion_texture = v.occlusion_texture().map(TextureInfo::<OcclusionInfo>::from);
        let emissive_texture = v.emissive_texture().map(TextureInfo::<()>::from);
        let emissive_factor = v.emissive_factor();

        Material {
            name,
            alpha_cutoff,
            alpha_mode,
            double_sided,
            pbr_metallic_roughness,
            normal_texture,
            occlusion_texture,
            emissive_texture,
            emissive_factor,
        }
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl From<gltf::material::AlphaMode> for AlphaMode {
    fn from(gltf_alpha_mode: gltf::material::AlphaMode) -> AlphaMode {
        use gltf::material::AlphaMode::{Blend, Mask, Opaque};
        match gltf_alpha_mode {
            Opaque => AlphaMode::Opaque,
            Mask => AlphaMode::Mask,
            Blend => AlphaMode::Blend,
        }
    }
}

impl Default for AlphaMode {
    fn default() -> AlphaMode {
        AlphaMode::from(gltf::material::AlphaMode::default())
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl From<gltf::mesh::BoundingBox> for BoundingBox {
    fn from(v: gltf::mesh::BoundingBox) -> BoundingBox {
        BoundingBox { min: v.min, max: v.max }
    }
}

struct Animation {
    channels: Vec<Channel>,
    #[allow(dead_code)]
    name: Option<String>,
}

impl Animation {
    fn start(&mut self, now: Instant) {
        for channel in self.channels.iter_mut() {
            channel.start(now);
        }
    }

    fn stop(&mut self) {
        for channel in self.channels.iter_mut() {
            channel.stop();
        }
    }

    fn process(&mut self, now: Instant, nodes: &mut [Node], meshes: &[Mesh]) {
        for channel in self.channels.iter_mut() {
            channel.process(now, nodes, meshes);
        }
    }
}

#[derive(Debug)]
enum Output {
    /// vector: [x, y, z]
    Translations(Vec<[f32; 3]>),

    /// quaternion: [x, y, z, w]
    Rotations(Vec<[f32; 4]>),

    /// vector: [x, y, z]
    Scales(Vec<[f32; 3]>),

    /// weight for each morph target
    MorphTargetWeights(Vec<f32>),
}

struct Channel {
    node_index: usize,

    /// key frame times
    inputs: Vec<f32>,

    /// property and data to update
    outputs: Output,

    /// Total length of the animation
    duration_secs: f32,

    start_time: Option<Instant>,

    interpolation: gltf::animation::Interpolation,

    loop_playback: bool,
}

// https://github.com/rust-lang/rust/issues/54361
fn to_secs_f32(d: Duration) -> f32 {
    const NANOS_PER_SEC: u32 = 1_000_000_000;
    let secs = (d.as_secs() as f64) + (d.subsec_nanos() as f64) / (NANOS_PER_SEC as f64);
    secs as f32
}

impl Channel {
    /// Returns the current time for the animation loop. The value ranges between
    /// `0.0` and `duration_secs`. Returns `None` if the animation is not playing.
    fn time(&mut self, now: Instant) -> Option<f32> {
        match self.start_time {
            Some(start_time) => {
                let time_as_duration = now - start_time;
                let time_as_secs = to_secs_f32(time_as_duration);
                if !self.loop_playback && time_as_secs > self.duration_secs {
                    self.start_time = None;
                }
                let time = time_as_secs % self.duration_secs;
                if time.is_nan() {
                    eprintln!(
                        "warn: animation channel time was NaN: time_as_secs: {}, channel_duration_secs: {}",
                        time_as_secs, self.duration_secs
                    );
                    self.start_time = None;
                    return None;
                }
                Some(time)
            }
            None => None,
        }
    }

    fn stop(&mut self) {
        self.start_time = None;
    }

    fn start(&mut self, now: Instant) {
        self.start_time = Some(now);
    }

    fn process(&mut self, now: Instant, nodes: &mut [Node], meshes: &[Mesh]) {
        use gltf::animation::Interpolation;

        if let Some(time) = self.time(now) {
            let index = self
                .inputs
                .binary_search_by(|probe| probe.partial_cmp(&time).unwrap())
                .unwrap_or_else(|index| index.saturating_sub(1));

            let interpolation_value = {
                let t0 = self.inputs[index];
                let t1 = self.inputs.get(index + 1).cloned().unwrap_or(t0);
                let dt = time - t0;
                if dt < 0.0 {
                    0.0
                } else {
                    dt / (t1 - t0)
                }
            };

            if self.interpolation != Interpolation::Linear {
                eprintln!("warn: using linear interpolation instead of: {:?}", self.interpolation);
            }

            match self.outputs {
                Output::Translations(ref outputs) => {
                    let a = outputs
                        .get(index)
                        .cloned()
                        .expect("animation channel: missing translation");
                    let b = outputs.get(index + 1).cloned().unwrap_or(a);

                    let a = Vector3::from(a);
                    let b = Vector3::from(b);

                    let t = ((1.0 - interpolation_value) * a) + (interpolation_value * b);
                    let mut node = &mut nodes[self.node_index];
                    node.animate_local_translation = Some(t.into());
                }
                Output::Scales(ref outputs) => {
                    let a = outputs.get(index).cloned().expect("animation channel: missing scale");
                    let b = outputs.get(index + 1).cloned().unwrap_or(a);

                    let a = Vector3::from(a);
                    let b = Vector3::from(b);

                    let s = ((1.0 - interpolation_value) * a) + (interpolation_value * b);
                    let mut node = &mut nodes[self.node_index];
                    node.animate_local_scale = Some(s.into());
                }
                Output::Rotations(ref outputs) => {
                    let a = outputs
                        .get(index)
                        .cloned()
                        .expect("animation channel: missing rotation");
                    let b = outputs.get(index + 1).cloned().unwrap_or(a);

                    let a = Quaternion::new(a[3], a[0], a[1], a[2]);
                    let b = Quaternion::new(b[3], b[0], b[1], b[2]);

                    // https://stackoverflow.com/a/2887128
                    //
                    // Each rotation can actually be represented by two quaternions, q and -q. But the Slerp path
                    // from q to w will be different from the path from (-q) to w: one will go the long away around,
                    // the other the short away around. If the dot product is negative, replace your quaterions
                    // q1 and q2 with -q1 and q2 before performing Slerp.
                    let a = if a.dot(b) < 0.0 { -a } else { a };

                    let r = a.slerp(b, interpolation_value);
                    let mut node = &mut nodes[self.node_index];
                    node.animate_local_rotation = Some([r.v.x, r.v.y, r.v.z, r.s]);
                }
                Output::MorphTargetWeights(ref outputs) => {
                    let mut node = &mut nodes[self.node_index];
                    let mesh_index = node.mesh_index.expect("animation channel: missing mesh index for node");
                    let mesh = &meshes[mesh_index];

                    let num_weights = mesh.morph_weights.as_ref().map(|v| v.len()).unwrap_or(0);

                    let a = outputs
                        .chunks(num_weights)
                        .nth(index)
                        .expect("animation channel: missing weights");
                    let b = outputs.chunks(num_weights).nth(index + 1).unwrap_or(a);

                    let mut weights = Vec::with_capacity(num_weights);

                    for i in 0..a.len() {
                        let a = Vector1::new(a[i]);
                        let b = Vector1::new(b[i]);
                        weights.push(a.lerp(b, interpolation_value).x);
                    }

                    node.animate_morph_weights = Some(weights);
                }
            }
        }
    }
}

// http://vulkan.gpuinfo.org/displaydevicelimit.php?name=minUniformBufferOffsetAlignment
#[repr(align(256))]
#[derive(Copy, Clone, Debug, Default)]
struct CameraAndLightSettings {
    scale_diff_base_mr: [f32; 4],
    scale_fgd_spec: [f32; 4],
    scale_ibl_ambient: [f32; 4],

    camera_position: [f32; 3],
    _pad0: f32,

    light_direction: [f32; 3],
    _pad1: f32,

    light_color: [f32; 3],
    _pad2: f32,

    specular_env_mip_count: f32,
}

#[repr(align(256))]
#[derive(Copy, Clone, Debug, Default)]
struct MeshSettings {
    mvp_matrix: [[f32; 4]; 4],
    model_matrix: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],

    morph_weights: [f32; MAX_MORPH_TARGETS],
}

type Bool32 = u32;

#[derive(Copy, Clone, Debug, Default)]
struct PrimitiveSettings {
    has_positions: Bool32,
    has_normals: Bool32,
    has_tangents: Bool32,
    has_colors: Bool32,
    has_texcoords: [Bool32; 2],
    has_weights: Bool32,
    has_joints: Bool32,
    has_morph_positions: [Bool32; 2],
    has_morph_normals: [Bool32; 2],
    has_morph_tangents: [Bool32; 2],
}

#[repr(align(256))]
#[derive(Copy, Clone)]
struct SkinSettings {
    joint_matrix: [[[f32; 4]; 4]; MAX_JOINTS],
}

impl Default for SkinSettings {
    fn default() -> SkinSettings {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(align(256))]
#[derive(Copy, Clone, Debug, Default)]
struct MaterialSettings {
    pbr_base_color_factor: [f32; 4],
    emissive_factor: [f32; 3],

    pbr_metallic_factor: f32,
    pbr_roughness_factor: f32,

    alpha_blend: f32,
    alpha_cutoff: f32,

    normal_scale: f32,
    occlusion_strength: f32,

    base_color_texcoord_set: u32,
    metallic_roughness_texcoord_set: u32,
    occlusion_texcoord_set: u32,
    emissive_texcoord_set: u32,
    normal_texcoord_set: u32,

    has_base_color_map: Bool32,
    has_metallic_roughness_map: Bool32,
    has_occlusion_map: Bool32,
    has_emissive_map: Bool32,
    has_normal_map: Bool32,
}

#[derive(Default)]
struct State {
    animation_index: Option<usize>,
}

struct AnimationHandler;

impl util::EventHandler<State> for AnimationHandler {
    fn on_event(&mut self, app: &mut App<State>, event: &winit::event::Event<()>) -> bool {
        use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { input, .. },
            ..
        } = event
        {
            match input {
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key1),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(0);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key2),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(1);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key3),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(2);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key4),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(3);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key5),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(4);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key6),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(5);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key7),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(6);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key8),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(7);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key9),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(8);
                }
                KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Key0),
                    state: ElementState::Pressed,
                    ..
                } => {
                    app.state.animation_index = Some(9);
                }
                _ => {}
            }
        }
        false
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tangent: [f32; 4],
    texcoord0: [f32; 2],
    texcoord1: [f32; 2],
    color: [f32; 4],
    joint: [u16; 4],
    weight: [f32; 4],
    morph_position0: [f32; 3],
    morph_position1: [f32; 3],
    morph_normal0: [f32; 3],
    morph_normal1: [f32; 3],
    morph_tangent0: [f32; 3],
    morph_tangent1: [f32; 3],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = pretty_env_logger::try_init();

    let path = if let Some(path) = std::env::args().nth(1) {
        path
    } else {
        eprintln!(
            "usage: {} <FILE>",
            std::env::current_exe()?.file_name().unwrap().to_string_lossy()
        );
        return Ok(());
    };

    let mut event_handlers = EventHandlers::default_event_handlers();
    event_handlers.push(Box::new(AnimationHandler));

    let mut app: App<State> = App::init("gltf_viewer.rs", 800, 600, EventHandlers::Custom(event_handlers))?;

    app.set_sample_count(8)?;

    let window = &app.window;

    let monitor = window.current_monitor();
    let dpi_factor = window.scale_factor();
    let monitor_physical_size = monitor.size();
    let monitor_logical_size = monitor_physical_size.to_logical::<f32>(dpi_factor);
    let window_size = window.outer_size().to_logical::<f32>(dpi_factor);
    let pos_x = (monitor_logical_size.width / 2.0) - (window_size.width / 2.0);
    let pos_y = (monitor_logical_size.height / 2.0) - (window_size.height / 2.0);
    window.set_outer_position(winit::dpi::LogicalPosition::new(pos_x, pos_y));

    println!("Importing file: {}", path);
    let import = match Import::load(&path) {
        Ok(import) => import,
        Err(e) => {
            eprintln!("{:?}", e);
            eprintln!("Press enter to quit");
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf).ok();
            return Err(e)?;
        }
    };

    let mut encoder = app.device.create_command_encoder()?;

    let _buffers: HashMap<usize, Buffer> = HashMap::with_capacity(import.buffers.len());
    let mut images = Vec::with_capacity(import.doc.images().len());
    let mut samplers = Vec::with_capacity(import.doc.samplers().len());
    let mut textures = Vec::with_capacity(import.doc.textures().len());
    let mut nodes = Vec::with_capacity(import.doc.nodes().len());
    let mut skins = Vec::with_capacity(import.doc.skins().len());
    let mut materials = Vec::with_capacity(import.doc.materials().len());
    let mut meshes = Vec::with_capacity(import.doc.meshes().len());
    let mut mesh_index_to_node_index = HashMap::with_capacity(import.doc.meshes().len());
    let mut animations = Vec::with_capacity(import.doc.animations().len());
    let mut mesh_pipeline_keys = HashSet::new();

    let missing_texture_image = {
        use image::GenericImageView;
        let img = image::load_from_memory(include_bytes!("assets/missing_texture.png"))?;
        let format = match img {
            image::DynamicImage::ImageLuma8(_) => gltf::image::Format::R8,
            image::DynamicImage::ImageLumaA8(_) => gltf::image::Format::R8G8,
            image::DynamicImage::ImageRgb8(_) => gltf::image::Format::R8G8B8,
            image::DynamicImage::ImageRgba8(_) => gltf::image::Format::R8G8B8A8,
            image::DynamicImage::ImageBgr8(_) => gltf::image::Format::B8G8R8,
            image::DynamicImage::ImageBgra8(_) => gltf::image::Format::B8G8R8A8,

            image::DynamicImage::ImageLuma16(_)
            | image::DynamicImage::ImageLumaA16(_)
            | image::DynamicImage::ImageRgb16(_)
            | image::DynamicImage::ImageRgba16(_) => {
                panic!("unsupported gltf image format");
            }
        };
        let (width, height) = img.dimensions();
        let pixels = img.to_bytes();
        gltf::image::Data {
            format,
            width,
            height,
            pixels,
        }
    };

    println!("Loading images: {}", import.images.len());
    for image in import.images.iter().chain(Some(&missing_texture_image)) {
        use gltf::image::Format;
        use image::{buffer::ConvertBuffer, ImageBuffer, Rgb, Rgba};

        type RgbaImage = ImageBuffer<Rgba<u8>, Vec<u8>>;
        type BgraImage = ImageBuffer<Rgba<u8>, Vec<u8>>;

        type RgbImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
        type BgrImage = ImageBuffer<Rgb<u8>, Vec<u8>>;

        let (width, height) = (image.width, image.height);
        let maybe_pixels: Vec<u8>;
        let (format, data) = match image.format {
            Format::R8 => (TextureFormat::R8Unorm, &image.pixels),
            Format::R8G8 => (TextureFormat::R8G8Unorm, &image.pixels),
            // 24-bit formats are not widely supported (which is probably why they aren't supported by gpuweb)
            // http://vulkan.gpuinfo.org/listformats.php
            Format::R8G8B8 => {
                println!("converting image to RGBA8 from: {:?}", image.format);
                let rgba: RgbaImage = RgbImage::from_raw(width, height, image.pixels.clone())
                    .unwrap()
                    .convert();
                maybe_pixels = rgba.into_raw();
                (TextureFormat::R8G8B8A8Unorm, &maybe_pixels)
            }
            Format::B8G8R8 => {
                println!("converting image to RGBA8 from: {:?}", image.format);
                let bgra: BgraImage = BgrImage::from_raw(width, height, image.pixels.clone())
                    .unwrap()
                    .convert();
                maybe_pixels = bgra.into_raw();
                (TextureFormat::B8G8R8A8Unorm, &maybe_pixels)
            }
            Format::R8G8B8A8 => (TextureFormat::R8G8B8A8Unorm, &image.pixels),
            Format::B8G8R8A8 => (TextureFormat::B8G8R8A8Unorm, &image.pixels),
            _ => {
                panic!("Unsupported texture format: {:?}", image.format);
            }
        };

        let texture = util::create_texture_with_data(&app.device, &mut encoder, data, true, format, width, height)?;
        util::generate_mipmaps(&mut encoder, &texture)?;
        images.push(texture);
    }

    println!("Loading samplers: {}", import.doc.samplers().len());
    for sampler in import.doc.samplers() {
        use gltf::texture::MagFilter;
        use gltf::texture::MinFilter;
        use gltf::texture::WrappingMode;

        fn address_mode(wrap_mode: WrappingMode) -> AddressMode {
            match wrap_mode {
                WrappingMode::ClampToEdge => AddressMode::ClampToEdge,
                WrappingMode::Repeat => AddressMode::Repeat,
                WrappingMode::MirroredRepeat => AddressMode::MirrorRepeat,
            }
        }

        fn min_filter_mimap_filter(min_filter: MinFilter) -> (FilterMode, FilterMode) {
            match min_filter {
                MinFilter::Linear => (FilterMode::Linear, FilterMode::Linear),
                MinFilter::Nearest => (FilterMode::Nearest, FilterMode::Nearest),
                MinFilter::LinearMipmapLinear => (FilterMode::Linear, FilterMode::Linear),
                MinFilter::LinearMipmapNearest => (FilterMode::Linear, FilterMode::Nearest),
                MinFilter::NearestMipmapNearest => (FilterMode::Nearest, FilterMode::Nearest),
                MinFilter::NearestMipmapLinear => (FilterMode::Nearest, FilterMode::Linear),
            }
        }

        let (min_filter, mipmap_filter) =
            min_filter_mimap_filter(sampler.min_filter().unwrap_or(gltf::texture::MinFilter::Nearest));

        let mag_filter = match sampler.mag_filter().unwrap_or(gltf::texture::MagFilter::Nearest) {
            MagFilter::Nearest => FilterMode::Nearest,
            MagFilter::Linear => FilterMode::Linear,
        };

        samplers.push(app.device.create_sampler(SamplerDescriptor {
            address_mode_u: address_mode(sampler.wrap_s()),
            address_mode_v: address_mode(sampler.wrap_t()),
            address_mode_w: AddressMode::ClampToEdge,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1000.0,
            mag_filter,
            min_filter,
            mipmap_filter,
            compare_function: CompareFunction::Never,
        })?);
    }

    let default_sampler = app.device.create_sampler(SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        lod_max_clamp: 1000.0,
        lod_min_clamp: 0.0,
        mipmap_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        compare_function: CompareFunction::Never,
    })?;

    for texture in import.doc.textures() {
        let image_index = texture.source().index();
        let sampler_index = texture.sampler().index();
        textures.push(TextureSampler {
            view: images[image_index].create_default_view()?,
            sampler: sampler_index
                .map(|i| samplers[i].clone())
                .unwrap_or_else(|| default_sampler.clone()),
        });
    }

    textures.push(TextureSampler {
        view: images.last().as_ref().unwrap().create_default_view()?,
        sampler: default_sampler.clone(),
    });

    let missing_texture = textures.last().unwrap().clone();

    println!("Loading nodes: {}", import.doc.nodes().len());
    for node in import.doc.nodes() {
        nodes.push(Node {
            name: node.name().map(|name| name.to_owned()),
            local_transform: node.transform(),
            children_indices: node.children().map(|node| node.index()).collect(),
            mesh_index: node.mesh().map(|mesh| mesh.index()),
            parent_node_index: None,
            animate_local_translation: None,
            animate_local_rotation: None,
            animate_local_scale: None,
            animate_morph_weights: None,
            skin_index: node.skin().map(|skin| skin.index()),
            is_joint: false,
        });
        if let Some(ref mesh_index) = node.mesh().map(|mesh| mesh.index()) {
            mesh_index_to_node_index.insert(*mesh_index, node.index());
        }
    }

    println!("Loading node parents");
    for node_index in 0..nodes.len() {
        let children: Vec<usize> = nodes[node_index].children_indices.iter().cloned().collect();
        for child_node_index in children.iter().cloned() {
            nodes[child_node_index].parent_node_index = Some(node_index);
        }
    }

    println!("Loading materials: {}", import.doc.materials().len());
    for material in import.doc.materials() {
        let material_index = material.index().unwrap();
        debug_assert_eq!(materials.len(), material_index);
        materials.push(Material::from(material));
    }

    // TODO: Default material
    materials.push(Material::default());

    println!("Loading skins: {}", import.doc.skins().len());
    for skin in import.doc.skins() {
        let name = skin.name().map(|name| name.into());
        let joint_node_indices: Vec<usize> = skin.joints().map(|j| j.index()).collect();
        for node_index in joint_node_indices.iter().cloned() {
            nodes[node_index].is_joint = true;
        }
        let reader = skin.reader(|buffer| Some(&import.buffers[buffer.index()]));
        let inverse_bind_matrices = if let Some(inverse_bind_matrices) = reader.read_inverse_bind_matrices() {
            inverse_bind_matrices.collect()
        } else {
            let identity = *mat4(&cgmath::Matrix4::identity());
            vec![identity; joint_node_indices.len()]
        };

        println!(" Loaded {} joints for skin: {:?}", joint_node_indices.len(), name);

        assert_eq!(joint_node_indices.len(), inverse_bind_matrices.len());

        skins.push(Skin {
            joint_node_indices,
            inverse_bind_matrices,
            name,
        })
    }

    println!("Loading animations: {}", import.doc.animations().len());
    for animation in import.doc.animations() {
        println!("animation: {} ({:?})", animation.index(), animation.name());
        let mut channels = Vec::new();
        for channel in animation.channels() {
            let reader = channel.reader(|buffer| Some(&*import.buffers[buffer.index()]));
            match (reader.read_inputs(), reader.read_outputs()) {
                (Some(inputs), Some(outputs)) => {
                    let inputs: Vec<f32> = inputs.collect();
                    let duration_secs = inputs.last().cloned().unwrap_or(0.0);
                    let interpolation = channel.sampler().interpolation();
                    let node_index = channel.target().node().index();
                    let start_time = None;
                    use gltf::animation::util::ReadOutputs;
                    let loop_playback = true;
                    match outputs {
                        ReadOutputs::Translations(outputs) => {
                            let outputs = Output::Translations(outputs.collect());
                            channels.push(Channel {
                                node_index,
                                duration_secs,
                                inputs,
                                outputs,
                                interpolation,
                                start_time,
                                loop_playback,
                            });
                        }
                        ReadOutputs::Rotations(outputs) => {
                            let outputs = outputs.into_f32();
                            let outputs = Output::Rotations(outputs.collect());
                            channels.push(Channel {
                                node_index,
                                duration_secs,
                                inputs,
                                outputs,
                                interpolation,
                                start_time,
                                loop_playback,
                            });
                        }
                        ReadOutputs::Scales(outputs) => {
                            let outputs = Output::Scales(outputs.collect());
                            channels.push(Channel {
                                node_index,
                                duration_secs,
                                inputs,
                                outputs,
                                interpolation,
                                start_time,
                                loop_playback,
                            });
                        }
                        ReadOutputs::MorphTargetWeights(outputs) => {
                            let outputs = outputs.into_f32();
                            let outputs = Output::MorphTargetWeights(outputs.collect());
                            channels.push(Channel {
                                node_index,
                                duration_secs,
                                inputs,
                                outputs,
                                interpolation,
                                start_time,
                                loop_playback,
                            });
                        }
                    }
                }
                _ => {
                    eprintln!("missing animation channel input or output");
                }
            }
        }
        animations.push(Animation {
            channels,
            name: animation.name().map(|name| name.to_owned()),
        });
    }

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices_u16: Vec<u16> = Vec::new();
    let mut indices_u32: Vec<u32> = Vec::new();

    println!("Loading meshes: {}", import.doc.meshes().len());
    for mesh in import.doc.meshes() {
        let mut primitives = Vec::with_capacity(mesh.primitives().len());

        for primitive in mesh.primitives() {
            use gltf::mesh::Mode;
            let primitive_topology = match primitive.mode() {
                Mode::Triangles => PrimitiveTopology::TriangleList,
                Mode::TriangleStrip => PrimitiveTopology::TriangleStrip,
                Mode::Lines => PrimitiveTopology::LineList,
                Mode::LineStrip => PrimitiveTopology::LineStrip,
                Mode::Points => PrimitiveTopology::PointList,
                mode @ _ => panic!("unsupported primitive mode: {:?}", mode),
            };

            let reader = primitive.reader(|buffer| Some(&import.buffers[buffer.index()]));

            let mut index_count = 0;

            let index_info = reader.read_indices().map(|indices| {
                use gltf::mesh::util::ReadIndices;
                match indices {
                    ReadIndices::U8(iter) => {
                        index_count = iter.len();
                        let offset = indices_u16.len() * std::mem::size_of::<u16>();
                        indices_u16.extend(iter.map(|i| i as u16));
                        (offset, IndexFormat::U16)
                    }
                    ReadIndices::U16(iter) => {
                        index_count = iter.len();
                        let offset = indices_u16.len() * std::mem::size_of::<u16>();
                        indices_u16.extend(iter);
                        (offset, IndexFormat::U16)
                    }
                    ReadIndices::U32(iter) => {
                        index_count = iter.len();
                        let offset = indices_u32.len() * std::mem::size_of::<u32>();
                        indices_u32.extend(iter);
                        (offset, IndexFormat::U32)
                    }
                }
            });

            let vertices_offset = vertices.len();
            let mut primitive_settings = PrimitiveSettings::default();
            let mut vertex_count = 0;

            if let Some(iter) = reader.read_positions() {
                vertex_count = iter.len();
                primitive_settings.has_positions = 1;
                vertices.reserve(iter.len());
                vertices.extend(iter.map(|value| {
                    let mut vertex = Vertex::default();
                    vertex.position = value;
                    vertex
                }));
                if let Some(iter) = reader.read_normals() {
                    primitive_settings.has_normals = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].normal = value;
                    }
                }
                if let Some(iter) = reader.read_tangents() {
                    primitive_settings.has_tangents = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].tangent = value;
                    }
                }
                if let Some(iter) = reader.read_tex_coords(0) {
                    let iter = iter.into_f32();
                    primitive_settings.has_texcoords[0] = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].texcoord0 = value;
                    }
                }
                if let Some(iter) = reader.read_tex_coords(1) {
                    let iter = iter.into_f32();
                    primitive_settings.has_texcoords[1] = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].texcoord1 = value;
                    }
                }
                if let Some(iter) = reader.read_colors(0) {
                    let iter = iter.into_rgba_f32();
                    primitive_settings.has_colors = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].color = value;
                    }
                }
                if let Some(iter) = reader.read_joints(0) {
                    let iter = iter.into_u16();
                    primitive_settings.has_joints = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].joint = value;
                    }
                }
                if let Some(iter) = reader.read_weights(0) {
                    let iter = iter.into_f32();
                    primitive_settings.has_weights = 1;
                    for (i, value) in iter.enumerate() {
                        vertices[vertices_offset + i].weight = value;
                    }
                }
                for (morph_target, (morph_position, morph_normal, morph_tangent)) in
                    reader.read_morph_targets().enumerate()
                {
                    if morph_target >= MAX_MORPH_TARGETS {
                        eprintln!("ignoring remaining morph targets");
                        break;
                    }
                    if let Some(iter) = morph_position {
                        primitive_settings.has_morph_positions[morph_target] = 1;
                        for (i, value) in iter.enumerate() {
                            match morph_target {
                                0 => vertices[vertices_offset + i].morph_position0 = value,
                                1 => vertices[vertices_offset + i].morph_position1 = value,
                                //2 => vertices[vertices_offset + i].morph_position2 = value,
                                //3 => vertices[vertices_offset + i].morph_position3 = value,
                                _ => unreachable!("morph_target: {}", i),
                            }
                        }
                    }
                    if let Some(iter) = morph_normal {
                        primitive_settings.has_morph_normals[morph_target] = 1;
                        for (i, value) in iter.enumerate() {
                            match morph_target {
                                0 => vertices[vertices_offset + i].morph_normal0 = value,
                                1 => vertices[vertices_offset + i].morph_normal1 = value,
                                //2 => vertices[vertices_offset + i].morph_normal2 = value,
                                //3 => vertices[vertices_offset + i].morph_normal3 = value,
                                _ => unreachable!("morph_target: {}", i),
                            }
                        }
                    }
                    if let Some(iter) = morph_tangent {
                        primitive_settings.has_morph_tangents[morph_target] = 1;
                        for (i, value) in iter.enumerate() {
                            match morph_target {
                                0 => vertices[vertices_offset + i].morph_tangent0 = value,
                                1 => vertices[vertices_offset + i].morph_tangent1 = value,
                                //2 => vertices[vertices_offset + i].morph_tangent2 = value,
                                //3 => vertices[vertices_offset + i].morph_tangent3 = value,
                                _ => unreachable!("morph_target: {}", i),
                            }
                        }
                    }
                }
            }

            let mesh_pipeline_key = MeshPipelineKey {
                primitive_topology,
                index_format: index_info.map(|info| info.1),
            };

            mesh_pipeline_keys.insert(mesh_pipeline_key);

            primitives.push(Primitive {
                material_index: primitive.material().index(),
                bounding_box: primitive.bounding_box().into(),
                settings: primitive_settings,
                vertex_count,
                vertex_buffer_offset: (vertices_offset * std::mem::size_of::<Vertex>()),
                index_buffer_offset: index_info.map(|info| info.0),
                index_count,
                mesh_pipeline_key,
            });
        }

        let morph_weights = mesh.weights().map(|weights| {
            let morph_targets = weights.len().min(MAX_MORPH_TARGETS);
            weights.iter().take(morph_targets).cloned().collect::<Vec<f32>>()
        });

        meshes.push(Mesh {
            name: mesh.name().map(|name| name.to_owned()),
            primitives,
            morph_weights,
            node_index: mesh_index_to_node_index[&mesh.index()],
        });
    }

    // make sure both of these aren't empty
    indices_u16.push(0);
    indices_u32.push(0);

    let vertex_buffer = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsage::VERTEX, &vertices)?;
    let index_buffer_u16 = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsage::INDEX, &indices_u16)?;
    let index_buffer_u32 = util::create_buffer_with_data(&app.device, &mut encoder, BufferUsage::INDEX, &indices_u32)?;

    // set 0, binding 0
    let mut camera_and_light_settings = CameraAndLightSettings::default();
    let camera_and_light_settings_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsage::UNIFORM | BufferUsage::COPY_DST,
        &[camera_and_light_settings],
    )?;

    // set 1, binding 0
    let mut mesh_settings: Vec<MeshSettings> = meshes.iter().map(|_| MeshSettings::default()).collect();
    let mesh_settings_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsage::UNIFORM | BufferUsage::COPY_DST,
        &mesh_settings,
    )?;

    // set 1, binding 1
    let mut skin_settings: Vec<SkinSettings> = skins.iter().map(|_| SkinSettings::default()).collect();
    for node in nodes.iter() {
        if let Some(joint_matrices) = node.get_joint_matrices(&skins, &nodes) {
            let mut settings = SkinSettings::default();
            for i in 0..settings.joint_matrix.len().min(joint_matrices.len()) {
                settings.joint_matrix[i] = joint_matrices[i];
            }
            let skin_index = node.skin_index.unwrap();
            skin_settings[skin_index] = settings;
        }
    }
    // add an additional skin to bind for meshes that aren't skinned
    skin_settings.push(SkinSettings::default());
    let skin_settings_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsage::UNIFORM | BufferUsage::COPY_DST,
        &skin_settings,
    )?;

    // set 2, binding 0
    let material_settings: Vec<MaterialSettings> = materials.iter().map(|m| m.settings()).collect();
    let material_settings_buffer = util::create_buffer_with_data(
        &app.device,
        &mut encoder,
        BufferUsage::UNIFORM | BufferUsage::COPY_DST,
        &material_settings,
    )?;

    println!("materials.len(): {}", materials.len());

    let mut material_pipeline_keys: Vec<MaterialPipelineKey> =
        materials.iter().map(|m| m.material_pipeline_key()).collect();

    material_pipeline_keys.sort();
    material_pipeline_keys.dedup();

    // render opaque objects before transparent ones
    let mut material_indices_sorted_by_pipeline_key: Vec<usize> = (0..materials.len()).collect();
    material_indices_sorted_by_pipeline_key.sort_by(|a, b| {
        let a = &materials[*a].material_pipeline_key();
        let b = &materials[*b].material_pipeline_key();
        a.cmp(&b)
    });

    type PipelineKey = (MaterialPipelineKey, MeshPipelineKey);

    let mut pipeline_keys: Vec<PipelineKey> = Vec::new();

    let mut mesh_pipeline_keys = Vec::new();

    let mut primitive_count = 0;

    type MaterialIndex = usize;
    type MeshIndex = usize;
    type PrimitiveIndex = usize;

    let mut material_primitive_map: HashMap<MaterialIndex, Vec<(MeshIndex, PrimitiveIndex)>> =
        HashMap::with_capacity(materials.len());

    // For primitives that have no material set and use the default
    material_primitive_map.insert(materials.len() - 1, Vec::new());

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            primitive_count += 1;
            let mesh_pipeline_key = primitive.mesh_pipeline_key;
            mesh_pipeline_keys.push(mesh_pipeline_key);
            if let Some(material_index) = primitive.material_index.or_else(|| Some(materials.len() - 1)) {
                let material_pipeline_key = materials[material_index].material_pipeline_key();
                pipeline_keys.push((material_pipeline_key, mesh_pipeline_key));
                let mesh_primitives = material_primitive_map.entry(material_index).or_default();
                mesh_primitives.push((mesh_index, primitive_index));
            }
        }
    }

    mesh_pipeline_keys.sort();
    mesh_pipeline_keys.dedup();

    pipeline_keys.sort();
    pipeline_keys.dedup();

    println!("Mesh primitive count: {}", primitive_count);
    println!("Unique pipeline_keys: {:?}", pipeline_keys.len());
    println!("Unique material_pipeline_keys: {:?}", material_pipeline_keys.len());
    println!("Unique mesh_pipeline_key: {:?}", mesh_pipeline_keys.len());

    #[rustfmt::skip]
    let bind_group_0_layout = app.device.create_bind_group_layout(BindGroupLayoutDescriptor {
        entries: vec![
            // CameraAndLightSettings
            BindGroupLayoutEntry {
                binding: 0,
                binding_type: BindingType::UniformBuffer,
                visibility: ShaderStage::VERTEX | ShaderStage::FRAGMENT,
            }
        ]
    })?;

    #[rustfmt::skip]
    let bind_group_1_layout = app.device.create_bind_group_layout(BindGroupLayoutDescriptor {
        entries: vec![
            // MaterialSettings
            BindGroupLayoutEntry {
                binding: 0,
                binding_type: BindingType::DynamicUniformBuffer,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_BaseColorSampler
            BindGroupLayoutEntry {
                binding: 1,
                binding_type: BindingType::Sampler,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_BaseColorTexture
            BindGroupLayoutEntry {
                binding: 2,
                binding_type: BindingType::SampledTexture,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_MetallicRoughnessSampler
            BindGroupLayoutEntry {
                binding: 3,
                binding_type: BindingType::Sampler,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_MetallicRoughnessTexture
            BindGroupLayoutEntry {
                binding: 4,
                binding_type: BindingType::SampledTexture,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_NormalSampler
            BindGroupLayoutEntry {
                binding: 5,
                binding_type: BindingType::Sampler,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_NormalTexture
            BindGroupLayoutEntry {
                binding: 6,
                binding_type: BindingType::SampledTexture,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_OcclusionSampler
            BindGroupLayoutEntry {
                binding: 7,
                binding_type: BindingType::Sampler,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_OcclusionTexture
            BindGroupLayoutEntry {
                binding: 8,
                binding_type: BindingType::SampledTexture,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_EmissiveSampler
            BindGroupLayoutEntry {
                binding: 9,
                binding_type: BindingType::Sampler,
                visibility: ShaderStage::FRAGMENT,
            },
            // u_EmissiveTexture
            BindGroupLayoutEntry {
                binding: 10,
                binding_type: BindingType::SampledTexture,
                visibility: ShaderStage::FRAGMENT,
            },
        ]
    })?;

    #[rustfmt::skip]
    let bind_group_2_layout = app.device.create_bind_group_layout(BindGroupLayoutDescriptor {
        entries: vec![
            // MeshSettings
            BindGroupLayoutEntry {
                binding: 0,
                binding_type: BindingType::DynamicUniformBuffer,
                visibility: ShaderStage::VERTEX,
            },
            // SkinSettings
            BindGroupLayoutEntry {
                binding: 1,
                binding_type: BindingType::DynamicUniformBuffer,
                visibility: ShaderStage::VERTEX,
            }
        ]
    })?;

    println!("Creating camera and light bind group (0)");
    #[rustfmt::skip]
    let bind_group_0 = app.device.create_bind_group(BindGroupDescriptor {
        layout: bind_group_0_layout.clone(),
        entries: vec![
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(
                    camera_and_light_settings_buffer.clone(),
                    0..camera_and_light_settings_buffer.size()
                ),
            }
        ]
    })?;

    let mut bind_group_1 = Vec::with_capacity(materials.len());

    println!("Creating material bind groups (1): {}", materials.len());
    for material in materials.iter() {
        let mut bindings = Vec::with_capacity(9);
        bindings.push(BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(
                material_settings_buffer.clone(),
                0..std::mem::size_of::<MaterialSettings>(),
            ),
        });

        let mut add_texture_sampler = |sampler_binding: u32, texture_binding: u32, texture_sampler: &TextureSampler| {
            bindings.push(BindGroupEntry {
                binding: sampler_binding,
                resource: BindingResource::Sampler(texture_sampler.sampler.clone()),
            });
            bindings.push(BindGroupEntry {
                binding: texture_binding,
                resource: BindingResource::TextureView(texture_sampler.view.clone()),
            });
        };

        if let Some(texture_sampler) = material
            .pbr_metallic_roughness
            .base_color_texture
            .as_ref()
            .map(|info| &textures[info.texture_index])
            .or(Some(&missing_texture))
        {
            add_texture_sampler(1, 2, texture_sampler);
        }

        if let Some(texture_sampler) = material
            .pbr_metallic_roughness
            .metallic_roughness_texture
            .as_ref()
            .map(|info| &textures[info.texture_index])
            .or(Some(&missing_texture))
        {
            add_texture_sampler(3, 4, texture_sampler);
        }

        if let Some(texture_sampler) = material
            .normal_texture
            .as_ref()
            .map(|info| &textures[info.texture_index])
            .or(Some(&missing_texture))
        {
            add_texture_sampler(5, 6, texture_sampler);
        }

        if let Some(texture_sampler) = material
            .occlusion_texture
            .as_ref()
            .map(|info| &textures[info.texture_index])
            .or(Some(&missing_texture))
        {
            add_texture_sampler(7, 8, texture_sampler);
        }

        if let Some(texture_sampler) = material
            .emissive_texture
            .as_ref()
            .map(|info| &textures[info.texture_index])
            .or(Some(&missing_texture))
        {
            add_texture_sampler(9, 10, texture_sampler);
        }

        bind_group_1.push(app.device.create_bind_group(BindGroupDescriptor {
            layout: bind_group_1_layout.clone(),
            entries: bindings,
        })?);
    }

    println!("Creating mesh and skin bind group (2)");
    #[rustfmt::skip]
    let bind_group_2 = app.device.create_bind_group(BindGroupDescriptor {
        layout: bind_group_2_layout.clone(),
        entries: vec![
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(
                    mesh_settings_buffer.clone(),
                    0..std::mem::size_of::<MeshSettings>()
                ),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(
                    skin_settings_buffer.clone(),
                    0..std::mem::size_of::<SkinSettings>()
                ),
            }
        ]
    })?;

    let push_constant_range = PushConstantRange {
        offset: 0,
        size: std::mem::size_of::<PrimitiveSettings>(),
        stages: ShaderStage::VERTEX | ShaderStage::FRAGMENT,
    };

    let render_pipeline_layout = app.device.create_pipeline_layout(PipelineLayoutDescriptor {
        bind_group_layouts: vec![
            bind_group_0_layout.clone(),
            bind_group_1_layout.clone(),
            bind_group_2_layout.clone(),
        ],
        push_constant_ranges: vec![push_constant_range],
    })?;

    let mut pipelines = HashMap::with_capacity(pipeline_keys.len());

    let vs = app.device.create_shader_module(ShaderModuleDescriptor {
        code: include_bytes!("shaders/gltf_viewer.vert.spv"),
    })?;

    let fs = app.device.create_shader_module(ShaderModuleDescriptor {
        code: include_bytes!("shaders/gltf_viewer.frag.spv"),
    })?;

    println!("Creating pipelines: {}", pipeline_keys.len());
    for (material_pipeline_key, mesh_pipeline_key) in pipeline_keys.drain(..) {
        #[rustfmt::skip]
        let render_pipeline_descriptor = RenderPipelineDescriptor {
            layout: render_pipeline_layout.clone(),
            vertex_stage: PipelineStageDescriptor {
                module: vs.clone(),
                entry_point: Cow::Borrowed("main"),
            },
            fragment_stage: PipelineStageDescriptor {
                module: fs.clone(),
                entry_point: Cow::Borrowed("main"),
            },
            rasterization_state: RasterizationStateDescriptor {
                front_face: FrontFace::Ccw,
                cull_mode: material_pipeline_key.cull_mode,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            },
            primitive_topology: mesh_pipeline_key.primitive_topology,
            color_states: vec![
                ColorStateDescriptor {
                    format: util::DEFAULT_COLOR_FORMAT,
                    color_blend: material_pipeline_key.color_blend,
                    alpha_blend: material_pipeline_key.alpha_blend,
                    write_mask: material_pipeline_key.write_mask,
                }
            ],
            depth_stencil_state: Some(
                DepthStencilStateDescriptor {
                    format: util::DEFAULT_DEPTH_FORMAT,
                    depth_write_enabled: material_pipeline_key.depth_write_enabled,
                    depth_compare: CompareFunction::Less,
                    stencil_back: StencilStateFaceDescriptor::IGNORE,
                    stencil_front: StencilStateFaceDescriptor::IGNORE,
                    stencil_read_mask: 0,
                    stencil_write_mask: 0,
                }
            ),
            vertex_state: VertexStateDescriptor {
                index_format: mesh_pipeline_key.index_format.unwrap_or(IndexFormat::U16),
                vertex_buffers: vec![
                    VertexBufferLayoutDescriptor {
                        input_slot: 0,
                        step_mode: InputStepMode::Vertex,
                        stride: std::mem::size_of::<Vertex>(),
                        attributes: vec![
                            VertexAttributeDescriptor {
                                shader_location: 0,
                                offset: offset_of!(Vertex, position),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 1,
                                offset: offset_of!(Vertex, normal),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 2,
                                offset: offset_of!(Vertex, tangent),
                                format: VertexFormat::Float4,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 3,
                                offset: offset_of!(Vertex, texcoord0),
                                format: VertexFormat::Float2,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 4,
                                offset: offset_of!(Vertex, texcoord1),
                                format: VertexFormat::Float2,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 5,
                                offset: offset_of!(Vertex, color),
                                format: VertexFormat::Float4,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 6,
                                offset: offset_of!(Vertex, joint),
                                format: VertexFormat::UShort4,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 7,
                                offset: offset_of!(Vertex, weight),
                                format: VertexFormat::Float4,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 8,
                                offset: offset_of!(Vertex, morph_position0),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 9,
                                offset: offset_of!(Vertex, morph_position1),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 10,
                                offset: offset_of!(Vertex, morph_normal0),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 11,
                                offset: offset_of!(Vertex, morph_normal1),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 12,
                                offset: offset_of!(Vertex, morph_tangent0),
                                format: VertexFormat::Float3,
                            },
                            VertexAttributeDescriptor {
                                shader_location: 13,
                                offset: offset_of!(Vertex, morph_tangent1),
                                format: VertexFormat::Float3,
                            },
                        ],
                    }
                ],
            },
            sample_count: app.get_sample_count(),
        };

        let pipeline = app.device.create_render_pipeline(render_pipeline_descriptor)?;
        pipelines.insert((material_pipeline_key, mesh_pipeline_key), pipeline);
    }

    let max_point = Point3::new(10.0, 10.0, 10.0); // TODO

    camera_and_light_settings.camera_position = app.camera.eye.into();
    camera_and_light_settings.light_direction = (max_point - Point3::origin()).normalize().into();
    camera_and_light_settings.light_color = [1.0, 1.0, 1.0];
    camera_and_light_settings.scale_diff_base_mr = [0.0, 0.0, 0.0, 0.0];
    camera_and_light_settings.scale_fgd_spec = [0.0, 0.0, 0.0, 0.0];
    camera_and_light_settings.scale_ibl_ambient = [1.0, 1.0, 1.0, 1.0];
    camera_and_light_settings.specular_env_mip_count = 24.0;

    let command_buffer = encoder.finish()?;
    let queue = app.device.get_queue();
    queue.submit(&[command_buffer])?;

    app.run(move |app| {
        let now = Instant::now();
        let mut encoder = app.device.create_command_encoder()?;

        let frame = match app.swapchain.acquire_next_image() {
            Ok(frame) => frame,
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        };

        camera_and_light_settings.camera_position = app.camera.eye.into();

        util::copy_to_buffer(
            &app.device,
            &mut encoder,
            &[camera_and_light_settings],
            &camera_and_light_settings_buffer,
        )?;

        if let Some(animation_index) = app.state.animation_index.take() {
            for animation in animations.iter_mut() {
                animation.stop();
            }
            if let Some(animation) = animations.get_mut(animation_index) {
                animation.start(now);
            }
        }

        for animation in animations.iter_mut() {
            animation.process(now, &mut nodes, &meshes);
        }

        for (mesh_index, mesh) in meshes.iter().enumerate() {
            let node = &nodes[mesh.node_index];
            let model_matrix = node.get_global_transform(&nodes);
            let normal_matrix = Matrix4::from(model_matrix).invert().unwrap().transpose();
            let mvp_matrix = app.camera.projection * app.camera.view * Matrix4::from(model_matrix);
            let morph_weights = node.get_morph_weights(&meshes).unwrap_or(&[]);

            let settings = &mut mesh_settings[mesh_index];
            settings.model_matrix = model_matrix.into();
            settings.normal_matrix = normal_matrix.into();
            settings.mvp_matrix = mvp_matrix.into();
            settings.morph_weights[0..morph_weights.len()].copy_from_slice(&morph_weights);

            if let Some(joints) = node.get_joint_matrices(&skins, &nodes) {
                let skin_index = node.skin_index.unwrap();
                let num_joints = joints.len().min(MAX_JOINTS);
                if num_joints != joints.len() {
                    eprintln!("Truncated joints: {} != {}", joints.len(), num_joints);
                }
                skin_settings[skin_index].joint_matrix[0..num_joints].copy_from_slice(&joints[0..num_joints]);
            }
        }

        for node in nodes.iter_mut() {
            node.animate_local_translation = None;
            node.animate_local_scale = None;
            node.animate_local_rotation = None;
            node.animate_morph_weights = None;
        }

        util::copy_to_buffer(&app.device, &mut encoder, &mesh_settings, &mesh_settings_buffer)?;
        util::copy_to_buffer(&app.device, &mut encoder, &skin_settings, &skin_settings_buffer)?;

        let (attachment, resolve_target) = if app.get_sample_count() == 1 {
            (&frame.view, None)
        } else {
            (&app.color_view, Some(&frame.view))
        };

        #[rustfmt::skip]
        let mut render_pass = encoder.begin_render_pass(RenderPassDescriptor {
            color_attachments: &[
                RenderPassColorAttachmentDescriptor {
                    attachment,
                    resolve_target,
                    store_op: StoreOp::Store,
                    load_op: LoadOp::Clear,
                    clear_color: Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 },
                    //clear_color: Color { r: 0.2, g: 0.6, b: 0.8, a: 1.0 },
                }
            ],
            depth_stencil_attachment: Some(
                RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &app.depth_view,
                    clear_depth: 1.0,
                    clear_stencil: 0,
                    depth_load_op: LoadOp::Clear,
                    depth_store_op: StoreOp::Store,
                    stencil_load_op: LoadOp::Clear,
                    stencil_store_op: StoreOp::Store,
                }
            ),
        });

        let mut last_pipeline_key = None;

        render_pass.set_bind_group(0, &bind_group_0, None);

        for material_index in material_indices_sorted_by_pipeline_key.iter().cloned() {
            let mesh_primitives = &material_primitive_map[&material_index];
            let material_settings_offset = util::byte_stride(&material_settings) * material_index;
            let material = &materials[material_index];
            let material_name = material
                .name
                .as_ref()
                .map(|name| name.clone())
                .unwrap_or_else(|| format!("material-{}", material_index));
            render_pass.push_debug_group(&material_name);
            let material_pipeline_key = material.material_pipeline_key();
            let dynamic_offsets = &[material_settings_offset];
            render_pass.set_bind_group(1, &bind_group_1[material_index], Some(dynamic_offsets));

            for (mesh_index, primitive_index) in mesh_primitives.iter() {
                let mesh_index = *mesh_index;
                let primitive_index = *primitive_index;
                let mesh = &meshes[mesh_index];
                let mesh_name = mesh
                    .name
                    .as_ref()
                    .map(|name| name.clone())
                    .unwrap_or_else(|| format!("mesh-{}", mesh_index));
                render_pass.insert_debug_marker(&mesh_name);
                let primitive = &mesh.primitives[primitive_index];
                let mesh_settings_offset = util::byte_stride(&mesh_settings) * mesh_index;
                let mesh_pipeline_key = primitive.mesh_pipeline_key;
                let skin_index = nodes[mesh.node_index].skin_index.unwrap_or(skin_settings.len() - 1);
                let skin_settings_offset = util::byte_stride(&skin_settings) * skin_index;
                let dynamic_offsets = &[mesh_settings_offset, skin_settings_offset];
                render_pass.set_bind_group(2, &bind_group_2, Some(dynamic_offsets));

                let pipeline_key = (material_pipeline_key, mesh_pipeline_key);
                if last_pipeline_key != Some(pipeline_key) {
                    last_pipeline_key = Some(pipeline_key);
                    if let Some(pipeline) = pipelines.get(&pipeline_key) {
                        render_pass.set_pipeline(pipeline);
                    } else {
                        println!("Skipping primitive due to missing pipeline. TODO default material");
                        continue;
                    };
                }

                let stages = ShaderStage::VERTEX | ShaderStage::FRAGMENT;
                render_pass.set_push_constants(stages, 0, primitive.settings)?;
                render_pass.set_vertex_buffers(0, &[vertex_buffer.clone()], &[primitive.vertex_buffer_offset]);
                match primitive.index_buffer_offset {
                    Some(index_buffer_offset) => {
                        match primitive.mesh_pipeline_key.index_format.unwrap() {
                            IndexFormat::U16 => render_pass.set_index_buffer(&index_buffer_u16, index_buffer_offset),
                            IndexFormat::U32 => render_pass.set_index_buffer(&index_buffer_u32, index_buffer_offset),
                        }
                        let index_count = primitive.index_count as u32;
                        render_pass.draw_indexed(index_count, 1, 0, 0, 0);
                    }
                    None => {
                        let vertex_count = primitive.vertex_count as u32;
                        render_pass.draw(vertex_count, 1, 0, 0);
                    }
                }
            }
            render_pass.pop_debug_group();
        }

        render_pass.end_pass();

        let command_buffer = encoder.finish()?;
        let queue = app.device.get_queue();
        queue.submit(&[command_buffer])?;

        match queue.present(frame) {
            Ok(()) => {}
            Err(SwapchainError::OutOfDate) => return Ok(()),
            Err(e) => return Err(e)?,
        }

        Ok(())
    });
}
