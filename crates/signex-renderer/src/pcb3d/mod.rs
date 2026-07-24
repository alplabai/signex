//! PCB 3D runtime GLB ingest contract and validation hooks.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: glTF 2.0 GLB container specification, serde_json public docs.

use crate::theme::ResolvedTheme;
use serde_json::Value;
use signex_3d_model_importer::{
    ImportWarning as ModelImportWarning, ModelImportRequest, import_model as import_to_glb,
};
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::Scene;
use signex_gfx::style::ColorSlot;
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

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeModelSource {
    FilePath(PathBuf),
    GlbBytes(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeModelBridgeRequest {
    pub model_id: String,
    pub source: RuntimeModelSource,
    pub cache_dir: PathBuf,
    pub converter_version: &'static str,
    pub transform: ModelTransform,
    pub material_policy: RuntimeMaterialPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeModelBridgeWarning {
    TextureMissing {
        uri: String,
    },
    EmptyPrimitive {
        mesh_index: usize,
        primitive_index: usize,
    },
    UnsupportedGeometry {
        entity_type: String,
    },
    UnsupportedGltfExtension {
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeModelBridgeResult {
    pub model: RuntimeGlbModel,
    pub resolved_glb_path: Option<PathBuf>,
    pub conversion_performed: bool,
    pub cache_hit: bool,
    pub warnings: Vec<RuntimeModelBridgeWarning>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeModelBridgeError {
    ImportFailed {
        model_id: String,
        path: PathBuf,
        reason: String,
    },
    IngestFailed(RuntimeGlbIngestError),
}

impl fmt::Display for RuntimeModelBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImportFailed {
                model_id,
                path,
                reason,
            } => write!(
                f,
                "model {model_id} failed import bridge for {:?}: {reason}",
                path
            ),
            Self::IngestFailed(err) => write!(f, "runtime GLB ingest failed: {err}"),
        }
    }
}

impl std::error::Error for RuntimeModelBridgeError {}

mod glb;
mod projection;

pub use glb::*;
pub use projection::*;

pub(super) fn rect_vertices(origin: [f32; 2], width: f32, height: f32) -> Vec<[f32; 2]> {
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

pub(super) fn square_vertices(origin: [f32; 2], size: f32) -> Vec<[f32; 2]> {
    let width = size.max(0.05);

    vec![
        [origin[0], origin[1]],
        [origin[0] + width, origin[1]],
        [origin[0] + width, origin[1] + width],
        [origin[0], origin[1] + width],
    ]
}

pub(super) fn with_alpha_mul(mut color: [f32; 4], alpha_mul: f32) -> [f32; 4] {
    color[3] = (color[3] * alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

pub(super) fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}
