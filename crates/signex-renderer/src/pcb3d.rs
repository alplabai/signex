//! PCB 3D runtime GLB ingest contract and validation hooks.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: glTF 2.0 GLB container specification, serde_json public docs.

use serde_json::Value;
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
    pub byte_len: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeGlbModel {
    pub model_id: String,
    pub transform: ModelTransform,
    pub material_policy: RuntimeMaterialPolicy,
    pub metadata: RuntimeGlbMetadata,
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
    let metadata = validate_glb_payload(&request.model_id, &bytes)?;

    Ok(RuntimeGlbModel {
        model_id: request.model_id,
        transform: request.transform,
        material_policy: request.material_policy,
        metadata,
    })
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

fn validate_glb_payload(
    model_id: &str,
    bytes: &[u8],
) -> Result<RuntimeGlbMetadata, RuntimeGlbIngestError> {
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

    let json_chunk = json_chunk.ok_or_else(|| invalid_glb(model_id, "missing GLB JSON chunk"))?;
    let (asset_version, scene_count, node_count, mesh_count) =
        extract_scene_counts(json_chunk).map_err(|reason| invalid_glb(model_id, reason))?;

    Ok(RuntimeGlbMetadata {
        asset_version,
        scene_count,
        node_count,
        mesh_count,
        byte_len: bytes.len(),
    })
}

fn invalid_glb(model_id: &str, reason: impl Into<String>) -> RuntimeGlbIngestError {
    RuntimeGlbIngestError::InvalidGlb {
        model_id: model_id.to_string(),
        reason: reason.into(),
    }
}

fn extract_scene_counts(json_chunk: &[u8]) -> Result<(String, usize, usize, usize), String> {
    let json_text = std::str::from_utf8(json_chunk)
        .map_err(|_| "JSON chunk is not valid UTF-8".to_string())?
        .trim_matches(char::from(0))
        .trim();

    let root: Value = serde_json::from_str(json_text)
        .map_err(|err| format!("JSON chunk parse failed: {err}"))?;

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

    let scene_count = root
        .get("scenes")
        .and_then(Value::as_array)
        .map_or(0, |v| v.len());
    let node_count = root
        .get("nodes")
        .and_then(Value::as_array)
        .map_or(0, |v| v.len());
    let mesh_count = root
        .get("meshes")
        .and_then(Value::as_array)
        .map_or(0, |v| v.len());

    if node_count == 0 {
        return Err("node graph is empty (nodes array missing or empty)".to_string());
    }

    if mesh_count == 0 {
        return Err("mesh count is zero (meshes array missing or empty)".to_string());
    }

    let has_scene_nodes = root
        .get("scenes")
        .and_then(Value::as_array)
        .is_some_and(|scenes| {
            scenes.iter().any(|scene| {
                scene
                    .get("nodes")
                    .and_then(Value::as_array)
                    .is_some_and(|nodes| !nodes.is_empty())
            })
        });

    if !has_scene_nodes {
        return Err("scene graph has no scene with node references".to_string());
    }

    Ok((asset_version.to_string(), scene_count, node_count, mesh_count))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}
