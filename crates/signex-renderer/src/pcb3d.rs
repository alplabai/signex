//! PCB 3D runtime GLB ingest contract and validation hooks.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: glTF 2.0 GLB container specification, serde_json public docs.

use crate::theme::ResolvedTheme;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::Scene;
use signex_gfx::style::ColorSlot;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const GLB_MAGIC: u32 = 0x46546C67;
const GLB_VERSION_2: u32 = 2;
const GLB_HEADER_LEN: usize = 12;
const GLB_CHUNK_HEADER_LEN: usize = 8;
const GLB_JSON_CHUNK_TYPE: u32 = 0x4E4F534A;

#[derive(Clone, Debug, PartialEq)]
pub enum GlbSource {
    FilePath(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelTransform {
    pub offset_xyz_mm: [f32; 3],
    pub rotation_xyz_deg: [f32; 3],
    pub scale_xyz: [f32; 3],
}

impl Default for ModelTransform {
    fn default() -> Self {
        Self {
            offset_xyz_mm: [0.0; 3],
            rotation_xyz_deg: [0.0; 3],
            scale_xyz: [1.0; 3],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RuntimeMaterialPolicy {
    #[default]
    PreserveEmbedded,
    OverrideByTheme,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeGlbIngestRequest {
    pub model_id: String,
    pub glb_source: GlbSource,
    pub transform: ModelTransform,
    pub material_policy: RuntimeMaterialPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeGlbMetadata {
    pub asset_version: String,
    pub scene_count: usize,
    pub node_count: usize,
    pub mesh_count: usize,
    pub mesh_primitive_count: usize,
    pub opaque_instance_count: usize,
    pub byte_len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeOpaquePrimitive {
    pub scene_index: usize,
    pub node_index: usize,
    pub mesh_index: usize,
    pub primitive_index: usize,
    pub material_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RuntimeMeshStaging {
    pub opaque_primitives: Vec<RuntimeOpaquePrimitive>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeGlbModel {
    pub model_id: String,
    pub transform: ModelTransform,
    pub material_policy: RuntimeMaterialPolicy,
    pub metadata: RuntimeGlbMetadata,
    pub mesh_staging: RuntimeMeshStaging,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpaquePassLayout {
    pub origin_mm: [f32; 2],
    pub tile_size_mm: f32,
    pub tile_gap_mm: f32,
    pub columns: usize,
    pub stroke_width_mm: f32,
}

impl Default for OpaquePassLayout {
    fn default() -> Self {
        Self {
            origin_mm: [0.0; 2],
            tile_size_mm: 1.2,
            tile_gap_mm: 0.35,
            columns: 8,
            stroke_width_mm: 0.06,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeGlbIngestError {
    UnsupportedSourceFormat {
        model_id: String,
        path: PathBuf,
        expected_extension: &'static str,
    },
    MissingGlbCacheEntry {
        model_id: String,
        path: PathBuf,
    },
    IoReadFailed {
        model_id: String,
        path: PathBuf,
        message: String,
    },
    InvalidGlb {
        model_id: String,
        reason: String,
    },
}

impl fmt::Display for RuntimeGlbIngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSourceFormat {
                model_id,
                path,
                expected_extension,
            } => write!(
                f,
                "model {model_id} rejected source {:?}: expected .{expected_extension}",
                path
            ),
            Self::MissingGlbCacheEntry { model_id, path } => {
                write!(f, "model {model_id} missing cached GLB at {:?}", path)
            }
            Self::IoReadFailed {
                model_id,
                path,
                message,
            } => {
                write!(
                    f,
                    "model {model_id} failed to read GLB at {:?}: {message}",
                    path
                )
            }
            Self::InvalidGlb { model_id, reason } => {
                write!(f, "model {model_id} invalid GLB payload: {reason}")
            }
        }
    }
}

impl std::error::Error for RuntimeGlbIngestError {}

pub fn ingest_runtime_glb(
    request: RuntimeGlbIngestRequest,
) -> Result<RuntimeGlbModel, RuntimeGlbIngestError> {
    let bytes = load_glb_bytes(&request.model_id, &request.glb_source)?;
    let (metadata, mesh_staging) = validate_and_stage_glb_payload(&request.model_id, &bytes)?;

    Ok(RuntimeGlbModel {
        model_id: request.model_id,
        transform: request.transform,
        material_policy: request.material_policy,
        metadata,
        mesh_staging,
    })
}

pub fn emit_opaque_pass_preview(
    model: &RuntimeGlbModel,
    theme: &ResolvedTheme,
    scene: &mut Scene,
    layout: OpaquePassLayout,
) {
    scene.polygons.clear();
    scene
        .polygons
        .reserve(model.mesh_staging.opaque_primitives.len());

    let base_fill = with_alpha_mul(theme.color(ColorSlot::Pin), 0.82);
    let alt_fill = with_alpha_mul(theme.color(ColorSlot::Ghost), 0.78);
    let stroke_color = theme.color(ColorSlot::SymbolBody);
    let columns = layout.columns.max(1);
    let tile_size = layout.tile_size_mm.max(0.1);
    let tile_gap = layout.tile_gap_mm.max(0.0);
    let step = tile_size + tile_gap;
    let stroke_width = layout.stroke_width_mm.max(0.01);

    for (index, primitive) in model.mesh_staging.opaque_primitives.iter().enumerate() {
        let row = (index / columns) as f32;
        let col = (index % columns) as f32;
        let x = layout.origin_mm[0] + col * step;
        let y = layout.origin_mm[1] + row * step;

        let fill_color = if primitive.mesh_index % 2 == 0 {
            base_fill
        } else {
            alt_fill
        };

        scene.polygons.push(GpuPolygon {
            vertices: square_vertices([x, y], tile_size),
            fill_color,
            stroke_color: Some(stroke_color),
            stroke_width,
        });
    }
}

fn load_glb_bytes(model_id: &str, source: &GlbSource) -> Result<Vec<u8>, RuntimeGlbIngestError> {
    match source {
        GlbSource::FilePath(path) => {
            if !is_glb_path(path) {
                return Err(RuntimeGlbIngestError::UnsupportedSourceFormat {
                    model_id: model_id.to_string(),
                    path: path.clone(),
                    expected_extension: "glb",
                });
            }

            if !path.exists() {
                return Err(RuntimeGlbIngestError::MissingGlbCacheEntry {
                    model_id: model_id.to_string(),
                    path: path.clone(),
                });
            }

            fs::read(path).map_err(|err| RuntimeGlbIngestError::IoReadFailed {
                model_id: model_id.to_string(),
                path: path.clone(),
                message: err.to_string(),
            })
        }
        GlbSource::Bytes(bytes) => Ok(bytes.clone()),
    }
}

fn is_glb_path(path: &PathBuf) -> bool {
    path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("glb"))
        .unwrap_or(false)
}

fn validate_and_stage_glb_payload(
    model_id: &str,
    bytes: &[u8],
) -> Result<(RuntimeGlbMetadata, RuntimeMeshStaging), RuntimeGlbIngestError> {
    if bytes.len() < GLB_HEADER_LEN {
        return Err(invalid_glb(model_id, "payload shorter than GLB header"));
    }

    let magic = read_u32_le(bytes, 0)
        .ok_or_else(|| invalid_glb(model_id, "missing GLB magic bytes"))?;
    if magic != GLB_MAGIC {
        return Err(invalid_glb(model_id, "invalid GLB magic"));
    }

    let version = read_u32_le(bytes, 4)
        .ok_or_else(|| invalid_glb(model_id, "missing GLB version"))?;
    if version != GLB_VERSION_2 {
        return Err(invalid_glb(
            model_id,
            format!("unsupported GLB container version {version}"),
        ));
    }

    let declared_len = read_u32_le(bytes, 8)
        .ok_or_else(|| invalid_glb(model_id, "missing GLB declared length"))?
        as usize;

    if declared_len != bytes.len() {
        return Err(invalid_glb(
            model_id,
            format!(
                "declared length {declared_len} does not match payload length {}",
                bytes.len()
            ),
        ));
    }

    let json_chunk = locate_glb_json_chunk(model_id, bytes)?;
    let root = parse_json_root(json_chunk).map_err(|reason| invalid_glb(model_id, reason))?;

    let asset_version = extract_asset_version(&root).map_err(|reason| invalid_glb(model_id, reason))?;
    let nodes = collect_nodes(&root).map_err(|reason| invalid_glb(model_id, reason))?;
    let scenes = collect_scenes(&root).map_err(|reason| invalid_glb(model_id, reason))?;
    let mesh_layouts = collect_mesh_layouts(&root).map_err(|reason| invalid_glb(model_id, reason))?;
    let opaque_primitives =
        stage_opaque_primitives(nodes, scenes, &mesh_layouts).map_err(|reason| invalid_glb(model_id, reason))?;

    let mesh_primitive_count = mesh_layouts
        .iter()
        .map(|layout| layout.material_indices.len())
        .sum();

    let metadata = RuntimeGlbMetadata {
        asset_version,
        scene_count: scenes.len(),
        node_count: nodes.len(),
        mesh_count: mesh_layouts.len(),
        mesh_primitive_count,
        opaque_instance_count: opaque_primitives.len(),
        byte_len: bytes.len(),
    };

    Ok((
        metadata,
        RuntimeMeshStaging {
            opaque_primitives,
        },
    ))
}

fn invalid_glb(model_id: &str, reason: impl Into<String>) -> RuntimeGlbIngestError {
    RuntimeGlbIngestError::InvalidGlb {
        model_id: model_id.to_string(),
        reason: reason.into(),
    }
}

fn locate_glb_json_chunk<'a>(model_id: &str, bytes: &'a [u8]) -> Result<&'a [u8], RuntimeGlbIngestError> {
    let mut offset = GLB_HEADER_LEN;
    let mut json_chunk: Option<&[u8]> = None;

    while offset < bytes.len() {
        if offset + GLB_CHUNK_HEADER_LEN > bytes.len() {
            return Err(invalid_glb(model_id, "truncated GLB chunk header"));
        }

        let chunk_len = read_u32_le(bytes, offset)
            .ok_or_else(|| invalid_glb(model_id, "failed to decode GLB chunk length"))?
            as usize;
        let chunk_type = read_u32_le(bytes, offset + 4)
            .ok_or_else(|| invalid_glb(model_id, "failed to decode chunk type"))?;

        offset += GLB_CHUNK_HEADER_LEN;

        let chunk_end = offset
            .checked_add(chunk_len)
            .ok_or_else(|| invalid_glb(model_id, "chunk length overflow"))?;

        if chunk_end > bytes.len() {
            return Err(invalid_glb(model_id, "GLB chunk overruns payload length"));
        }

        if chunk_type == GLB_JSON_CHUNK_TYPE && json_chunk.is_none() {
            json_chunk = Some(&bytes[offset..chunk_end]);
        }

        offset = chunk_end;
    }

    json_chunk.ok_or_else(|| invalid_glb(model_id, "missing GLB JSON chunk"))
}

fn parse_json_root(json_chunk: &[u8]) -> Result<Value, String> {
    let json_text = std::str::from_utf8(json_chunk)
        .map_err(|_| "JSON chunk is not valid UTF-8".to_string())?
        .trim_matches(char::from(0))
        .trim();

    serde_json::from_str(json_text).map_err(|err| format!("JSON chunk parse failed: {err}"))
}

fn extract_asset_version(root: &Value) -> Result<String, String> {
    let asset_version = root
        .get("asset")
        .and_then(|asset| asset.get("version"))
        .and_then(Value::as_str)
        .ok_or_else(|| "missing asset.version".to_string())?;

    if !asset_version.starts_with('2') {
        return Err(format!(
            "unsupported asset.version {asset_version}; expected glTF 2.x"
        ));
    }

    Ok(asset_version.to_string())
}

fn collect_nodes(root: &Value) -> Result<&Vec<Value>, String> {
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "node graph is empty (nodes array missing or empty)".to_string())?;

    if nodes.is_empty() {
        return Err("node graph is empty (nodes array missing or empty)".to_string());
    }

    Ok(nodes)
}

fn collect_scenes(root: &Value) -> Result<&Vec<Value>, String> {
    let scenes = root
        .get("scenes")
        .and_then(Value::as_array)
        .ok_or_else(|| "scene graph has no scene with node references".to_string())?;

    if scenes.is_empty() {
        return Err("scene graph has no scene with node references".to_string());
    }

    Ok(scenes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MeshLayout {
    material_indices: Vec<Option<usize>>,
}

fn collect_mesh_layouts(root: &Value) -> Result<Vec<MeshLayout>, String> {
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "mesh count is zero (meshes array missing or empty)".to_string())?;

    if meshes.is_empty() {
        return Err("mesh count is zero (meshes array missing or empty)".to_string());
    }

    let mut layouts = Vec::with_capacity(meshes.len());

    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
            return Err(format!("mesh[{mesh_index}] missing primitives array"));
        };

        if primitives.is_empty() {
            return Err(format!("mesh[{mesh_index}] has empty primitives array"));
        }

        let mut material_indices = Vec::with_capacity(primitives.len());

        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let material_index = primitive
                .get("material")
                .and_then(Value::as_u64)
                .map(|raw| {
                    usize::try_from(raw).map_err(|_| {
                        format!(
                            "mesh[{mesh_index}].primitives[{primitive_index}] material index overflow"
                        )
                    })
                })
                .transpose()?;

            material_indices.push(material_index);
        }

        layouts.push(MeshLayout { material_indices });
    }

    Ok(layouts)
}

fn stage_opaque_primitives(
    nodes: &[Value],
    scenes: &[Value],
    mesh_layouts: &[MeshLayout],
) -> Result<Vec<RuntimeOpaquePrimitive>, String> {
    let mut staged = Vec::new();

    for (scene_index, scene) in scenes.iter().enumerate() {
        let Some(scene_nodes) = scene.get("nodes").and_then(Value::as_array) else {
            continue;
        };

        for scene_node in scene_nodes {
            let node_index = parse_index(
                scene_node,
                nodes.len(),
                format!("scene[{scene_index}].nodes index"),
            )?;

            stage_node_tree(scene_index, node_index, nodes, mesh_layouts, &mut staged)?;
        }
    }

    if staged.is_empty() {
        return Err("scene graph has no scene with node references".to_string());
    }

    Ok(staged)
}

fn stage_node_tree(
    scene_index: usize,
    root_node: usize,
    nodes: &[Value],
    mesh_layouts: &[MeshLayout],
    staged: &mut Vec<RuntimeOpaquePrimitive>,
) -> Result<(), String> {
    let mut stack = vec![root_node];
    let mut visited = HashSet::new();

    while let Some(node_index) = stack.pop() {
        if !visited.insert(node_index) {
            continue;
        }

        let node = nodes
            .get(node_index)
            .ok_or_else(|| format!("node index {node_index} out of range"))?;

        if let Some(mesh_value) = node.get("mesh") {
            let mesh_index = parse_index(
                mesh_value,
                mesh_layouts.len(),
                format!("nodes[{node_index}].mesh index"),
            )?;

            for (primitive_index, material_index) in mesh_layouts[mesh_index]
                .material_indices
                .iter()
                .copied()
                .enumerate()
            {
                staged.push(RuntimeOpaquePrimitive {
                    scene_index,
                    node_index,
                    mesh_index,
                    primitive_index,
                    material_index,
                });
            }
        }

        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children.iter().rev() {
                let child_index = parse_index(
                    child,
                    nodes.len(),
                    format!("nodes[{node_index}].children index"),
                )?;
                stack.push(child_index);
            }
        }
    }

    Ok(())
}

fn parse_index(value: &Value, upper_bound: usize, label: String) -> Result<usize, String> {
    let raw = value
        .as_u64()
        .ok_or_else(|| format!("{label} must be an unsigned integer"))?;

    let index = usize::try_from(raw).map_err(|_| format!("{label} overflows usize"))?;

    if index >= upper_bound {
        return Err(format!(
            "{label} {index} out of range (max {})",
            upper_bound.saturating_sub(1)
        ));
    }

    Ok(index)
}

// ---------------------------------------------------------------------------
// Projection texture pass
// ---------------------------------------------------------------------------

/// Footprint-space or UV-space rectangular bounds used by the projection pass.
///
/// For `ProjectionPassConfig.footprint_bounds`: coordinates are in millimetres.
/// For `ProjectionPassConfig.uv_bounds`: coordinates are normalized [0.0, 1.0].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionBounds {
    pub min_mm: [f32; 2],
    pub max_mm: [f32; 2],
}

impl Default for ProjectionBounds {
    fn default() -> Self {
        Self {
            min_mm: [0.0; 2],
            max_mm: [1.0; 2],
        }
    }
}

impl ProjectionBounds {
    fn width(&self) -> f32 {
        self.max_mm[0] - self.min_mm[0]
    }

    fn height(&self) -> f32 {
        self.max_mm[1] - self.min_mm[1]
    }
}

/// Configuration for the projection texture pass.
///
/// `footprint_bounds` defines the board-layer footprint extent in mm.
/// `uv_bounds` defines normalized UV coverage within that footprint; both
/// components must be in `[0.0, 1.0]` with `min < max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionPassConfig {
    pub footprint_bounds: ProjectionBounds,
    pub uv_bounds: ProjectionBounds,
    pub tile_columns: usize,
    pub fill_alpha: f32,
    pub stroke_width_mm: f32,
}

impl Default for ProjectionPassConfig {
    fn default() -> Self {
        Self {
            footprint_bounds: ProjectionBounds::default(),
            uv_bounds: ProjectionBounds::default(),
            tile_columns: 8,
            fill_alpha: 0.72,
            stroke_width_mm: 0.05,
        }
    }
}

/// Errors produced by [`check_projection_alignment`] and [`emit_projection_pass`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionAlignmentError {
    ZeroAreaFootprint {
        model_id: String,
    },
    UvBoundsOutOfRange {
        model_id: String,
        axis: &'static str,
        which: &'static str,
    },
    UvBoundsInverted {
        model_id: String,
        axis: &'static str,
    },
}

impl fmt::Display for ProjectionAlignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroAreaFootprint { model_id } => {
                write!(f, "model {model_id}: footprint bounds have zero area")
            }
            Self::UvBoundsOutOfRange {
                model_id,
                axis,
                which,
            } => {
                write!(
                    f,
                    "model {model_id}: UV bounds {axis}.{which} is outside [0.0, 1.0]"
                )
            }
            Self::UvBoundsInverted { model_id, axis } => {
                write!(
                    f,
                    "model {model_id}: UV bounds {axis}.min >= {axis}.max"
                )
            }
        }
    }
}

impl std::error::Error for ProjectionAlignmentError {}

/// Validate that `config` is geometrically consistent for projection.
///
/// Returns `Ok(())` when:
/// - The footprint bounds have positive area on both axes.
/// - All UV bound values are in `[0.0, 1.0]`.
/// - UV min is strictly less than UV max on both axes.
pub fn check_projection_alignment(
    model_id: &str,
    config: &ProjectionPassConfig,
) -> Result<(), ProjectionAlignmentError> {
    let fb = &config.footprint_bounds;
    if fb.width() <= 0.0 || fb.height() <= 0.0 {
        return Err(ProjectionAlignmentError::ZeroAreaFootprint {
            model_id: model_id.to_string(),
        });
    }

    let uv = &config.uv_bounds;
    let components = [
        ("x", uv.min_mm[0], uv.max_mm[0]),
        ("y", uv.min_mm[1], uv.max_mm[1]),
    ];

    for (axis, uv_min, uv_max) in components {
        if !(0.0..=1.0).contains(&uv_min) {
            return Err(ProjectionAlignmentError::UvBoundsOutOfRange {
                model_id: model_id.to_string(),
                axis,
                which: "min",
            });
        }
        if !(0.0..=1.0).contains(&uv_max) {
            return Err(ProjectionAlignmentError::UvBoundsOutOfRange {
                model_id: model_id.to_string(),
                axis,
                which: "max",
            });
        }
        if uv_min >= uv_max {
            return Err(ProjectionAlignmentError::UvBoundsInverted {
                model_id: model_id.to_string(),
                axis,
            });
        }
    }

    Ok(())
}

/// Emit one overlay polygon per staged opaque primitive into `scene.overlay_polygons`.
///
/// Ordering boundary: this function writes to `scene.overlay_polygons` only.
/// Callers must call [`emit_opaque_pass_preview`] first (which writes to
/// `scene.polygons`) to maintain the expected render order: opaque → projection.
///
/// Returns `Err` if alignment validation fails; `scene.overlay_polygons` is not
/// modified in that case.
pub fn emit_projection_pass(
    model: &RuntimeGlbModel,
    theme: &ResolvedTheme,
    scene: &mut Scene,
    config: ProjectionPassConfig,
) -> Result<(), ProjectionAlignmentError> {
    check_projection_alignment(&model.model_id, &config)?;

    let fb = &config.footprint_bounds;
    let uv = &config.uv_bounds;

    let proj_min_x = fb.min_mm[0] + uv.min_mm[0] * fb.width();
    let proj_min_y = fb.min_mm[1] + uv.min_mm[1] * fb.height();
    let proj_max_x = fb.min_mm[0] + uv.max_mm[0] * fb.width();
    let proj_max_y = fb.min_mm[1] + uv.max_mm[1] * fb.height();

    let proj_w = proj_max_x - proj_min_x;
    let proj_h = proj_max_y - proj_min_y;

    let columns = config.tile_columns.max(1);
    let count = model.mesh_staging.opaque_primitives.len();
    let rows = (count + columns - 1).max(1);
    let tile_w = (proj_w / columns as f32).max(0.01);
    let tile_h = (proj_h / rows as f32).max(0.01);

    let fill_base = with_alpha_mul(theme.color(ColorSlot::LassoFill), config.fill_alpha);
    let stroke_color = theme.color(ColorSlot::LassoStroke);
    let stroke_width = config.stroke_width_mm.max(0.01);

    scene.overlay_polygons.clear();
    scene.overlay_polygons.reserve(count);

    for (index, _primitive) in model.mesh_staging.opaque_primitives.iter().enumerate() {
        let col = (index % columns) as f32;
        let row = (index / columns) as f32;
        let x = proj_min_x + col * tile_w;
        let y = proj_min_y + row * tile_h;

        scene.overlay_polygons.push(GpuPolygon {
            vertices: rect_vertices([x, y], tile_w, tile_h),
            fill_color: fill_base,
            stroke_color: Some(stroke_color),
            stroke_width,
        });
    }

    Ok(())
}

fn rect_vertices(origin: [f32; 2], width: f32, height: f32) -> Vec<[f32; 2]> {
    let w = width.max(0.01);
    let h = height.max(0.01);
    vec![
        [origin[0], origin[1]],
        [origin[0] + w, origin[1]],
        [origin[0] + w, origin[1] + h],
        [origin[0], origin[1] + h],
    ]
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn square_vertices(origin: [f32; 2], size: f32) -> Vec<[f32; 2]> {
    let width = size.max(0.05);

    vec![
        [origin[0], origin[1]],
        [origin[0] + width, origin[1]],
        [origin[0] + width, origin[1] + width],
        [origin[0], origin[1] + width],
    ]
}

fn with_alpha_mul(mut color: [f32; 4], alpha_mul: f32) -> [f32; 4] {
    color[3] = (color[3] * alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}
