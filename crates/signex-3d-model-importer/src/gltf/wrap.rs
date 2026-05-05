use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::{json, Value};

use crate::error::{ImportWarning, ModelImportError};

pub struct GltfWrapResult {
    pub json_bytes: Vec<u8>,
    pub bin_bytes: Vec<u8>,
    pub warnings: Vec<ImportWarning>,
    pub mesh_count: usize,
    pub primitive_count: usize,
}

/// Wrap a `.gltf` JSON payload into GLB chunks (JSON + BIN).
///
/// This function keeps geometry indices/accessors intact while collapsing
/// multiple external buffers into a single BIN chunk and rewriting
/// `bufferViews[*].buffer`/`byteOffset` accordingly.
pub fn wrap_gltf(
    source: &str,
    source_path: &PathBuf,
    converter_version: &str,
) -> Result<GltfWrapResult, ModelImportError> {
    let mut root: Value = serde_json::from_str(source).map_err(|err| ModelImportError::GltfParseFailed {
        path: source_path.clone(),
        reason: format!("JSON parse failed: {err}"),
    })?;

    validate_asset_version(&root, source_path)?;

    let base_dir = source_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut warnings = Vec::new();

    let mut merged_bin = Vec::<u8>::new();
    let mut buffer_offsets = Vec::<usize>::new();

    let original_buffers = root
        .get("buffers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for (index, buffer) in original_buffers.iter().enumerate() {
        align4(&mut merged_bin, 0u8);
        let offset = merged_bin.len();
        let payload = read_buffer_payload(source_path, &base_dir, index, buffer)?;
        merged_bin.extend_from_slice(&payload);
        buffer_offsets.push(offset);
    }

    rewrite_buffer_views(&mut root, &buffer_offsets, source_path)?;
    embed_image_uris(&mut root, &base_dir, &mut warnings);
    rewrite_asset_extras(&mut root, source_path, converter_version);

    root["buffers"] = json!([{ "byteLength": merged_bin.len() }]);

    let mesh_count = root
        .get("meshes")
        .and_then(Value::as_array)
        .map(|arr| arr.len())
        .unwrap_or(0);

    let primitive_count = root
        .get("meshes")
        .and_then(Value::as_array)
        .map(|meshes| {
            meshes
                .iter()
                .map(|mesh| {
                    mesh.get("primitives")
                        .and_then(Value::as_array)
                        .map(|p| p.len())
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0);

    let json_bytes = serde_json::to_vec(&root).map_err(|err| ModelImportError::GltfParseFailed {
        path: source_path.clone(),
        reason: format!("JSON serialization failed: {err}"),
    })?;

    Ok(GltfWrapResult {
        json_bytes,
        bin_bytes: merged_bin,
        warnings,
        mesh_count,
        primitive_count,
    })
}

fn validate_asset_version(root: &Value, source_path: &PathBuf) -> Result<(), ModelImportError> {
    let version = root
        .get("asset")
        .and_then(|asset| asset.get("version"))
        .and_then(Value::as_str)
        .ok_or_else(|| ModelImportError::GltfParseFailed {
            path: source_path.clone(),
            reason: "missing asset.version".to_owned(),
        })?;

    if !version.starts_with('2') {
        return Err(ModelImportError::GltfParseFailed {
            path: source_path.clone(),
            reason: format!("unsupported asset.version {version}; expected 2.x"),
        });
    }

    Ok(())
}

fn read_buffer_payload(
    source_path: &PathBuf,
    base_dir: &Path,
    index: usize,
    buffer: &Value,
) -> Result<Vec<u8>, ModelImportError> {
    let uri = buffer.get("uri").and_then(Value::as_str).unwrap_or_default();

    if uri.is_empty() {
        return Ok(Vec::new());
    }

    if uri.starts_with("data:") {
        return decode_data_uri(uri).ok_or_else(|| ModelImportError::GltfParseFailed {
            path: source_path.clone(),
            reason: format!("buffers[{index}] has invalid data URI"),
        });
    }

    let buffer_path = base_dir.join(uri);
    std::fs::read(&buffer_path).map_err(|err| ModelImportError::GltfParseFailed {
        path: source_path.clone(),
        reason: format!("buffers[{index}] missing external resource {:?}: {err}", buffer_path),
    })
}

fn rewrite_buffer_views(
    root: &mut Value,
    buffer_offsets: &[usize],
    source_path: &PathBuf,
) -> Result<(), ModelImportError> {
    let Some(buffer_views) = root.get_mut("bufferViews").and_then(Value::as_array_mut) else {
        return Ok(());
    };

    for (index, view) in buffer_views.iter_mut().enumerate() {
        let old_buffer_index = view
            .get("buffer")
            .and_then(Value::as_u64)
            .and_then(|raw| usize::try_from(raw).ok())
            .unwrap_or(0);

        let base_offset = buffer_offsets.get(old_buffer_index).copied().ok_or_else(|| {
            ModelImportError::GltfParseFailed {
                path: source_path.clone(),
                reason: format!(
                    "bufferViews[{index}] references missing buffer index {old_buffer_index}"
                ),
            }
        })?;

        let old_offset = view
            .get("byteOffset")
            .and_then(Value::as_u64)
            .and_then(|raw| usize::try_from(raw).ok())
            .unwrap_or(0);

        view["buffer"] = json!(0);
        view["byteOffset"] = json!(base_offset + old_offset);
    }

    Ok(())
}

fn embed_image_uris(root: &mut Value, base_dir: &Path, warnings: &mut Vec<ImportWarning>) {
    let Some(images) = root.get_mut("images").and_then(Value::as_array_mut) else {
        return;
    };

    for image in images {
        let Some(uri) = image.get("uri").and_then(Value::as_str).map(ToOwned::to_owned) else {
            continue;
        };

        if uri.starts_with("data:") {
            continue;
        }

        let image_path = base_dir.join(&uri);
        match std::fs::read(&image_path) {
            Ok(bytes) => {
                let mime = guess_mime_type(&image_path);
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                image["uri"] = Value::String(format!("data:{mime};base64,{encoded}"));
            }
            Err(_) => warnings.push(ImportWarning::TextureMissing { uri }),
        }
    }
}

fn rewrite_asset_extras(root: &mut Value, source_path: &Path, converter_version: &str) {
    let Some(asset) = root.get_mut("asset").and_then(Value::as_object_mut) else {
        return;
    };

    let mut extras = asset
        .get("extras")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    extras.insert("source_format".to_owned(), Value::String("gltf".to_owned()));
    extras.insert(
        "source_path".to_owned(),
        Value::String(source_path.to_string_lossy().to_string()),
    );
    extras.insert(
        "converter_version".to_owned(),
        Value::String(converter_version.to_owned()),
    );

    asset.insert("extras".to_owned(), Value::Object(extras));
}

fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    if !uri.contains(";base64,") {
        return None;
    }

    let (_, encoded) = uri.split_once(',')?;
    base64::engine::general_purpose::STANDARD.decode(encoded).ok()
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn align4(bytes: &mut Vec<u8>, fill: u8) {
    while bytes.len() % 4 != 0 {
        bytes.push(fill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_data_uri_base64_ok() {
        let uri = "data:application/octet-stream;base64,AAECAw==";
        let bytes = decode_data_uri(uri).expect("decode");
        assert_eq!(bytes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn decode_data_uri_non_base64_is_none() {
        let uri = "data:text/plain,abc";
        assert!(decode_data_uri(uri).is_none());
    }
}
