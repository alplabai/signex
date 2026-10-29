pub mod wrap;

use std::path::Path;

use crate::error::ModelImportError;

pub use wrap::GltfWrapResult;

/// Load a `.gltf` JSON source and wrap it into a GLB payload.
pub fn load(path: &Path, converter_version: &str) -> Result<GltfWrapResult, ModelImportError> {
    let source =
        crate::read::read_text_capped(path, crate::read::MAX_GLTF_JSON_BYTES).map_err(|e| {
            ModelImportError::IoFailed {
                path: path.to_path_buf(),
                message: e.to_string(),
            }
        })?;

    wrap::wrap_gltf(&source, path, converter_version)
}

/// Fingerprint of the external buffer and image files a `.gltf` references
/// (name, mtime, length each), for the cache key: editing a `.bin` or texture
/// beside an unchanged `.gltf` must not serve the stale GLB. Empty when the
/// JSON is unreadable (the conversion then reports the real error).
pub fn dependency_stamp(path: &Path) -> String {
    let Ok(source) = crate::read::read_text_capped(path, crate::read::MAX_GLTF_JSON_BYTES) else {
        return String::new();
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&source) else {
        return String::new();
    };
    let base_dir = wrap::model_base_dir(path);
    let mut stamp = String::new();
    for key in ["buffers", "images"] {
        let uris = root[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["uri"].as_str())
            .filter(|uri| !uri.starts_with("data:"));
        for uri in uris {
            let Ok(file) = wrap::resolve_external_uri(&base_dir, uri) else {
                continue;
            };
            let (nanos, len) = std::fs::metadata(&file)
                .ok()
                .map(|m| {
                    let nanos = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_nanos());
                    (nanos, m.len())
                })
                .unwrap_or_default();
            stamp.push_str(&format!("{uri}:{nanos}:{len};"));
        }
    }
    stamp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_stamp_changes_when_an_external_buffer_changes() {
        let dir = tempfile::tempdir().unwrap();
        let gltf = dir.path().join("m.gltf");
        std::fs::write(
            &gltf,
            r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"m.bin","byteLength":4},{"uri":"data:;base64,AAAA","byteLength":3}]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("m.bin"), [0u8; 4]).unwrap();
        let before = dependency_stamp(&gltf);
        assert!(before.contains("m.bin") && !before.contains("data:"));
        std::fs::write(dir.path().join("m.bin"), [0u8; 8]).unwrap();
        assert_ne!(before, dependency_stamp(&gltf));
    }
}
