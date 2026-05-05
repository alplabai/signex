//! Integration tests for Milestone C runtime GLB ingest hooks.

use signex_renderer::pcb3d::{
    ingest_runtime_glb, GlbSource, ModelTransform, RuntimeGlbIngestError, RuntimeGlbIngestRequest,
    RuntimeMaterialPolicy,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn request(model_id: &str, glb_source: GlbSource) -> RuntimeGlbIngestRequest {
    RuntimeGlbIngestRequest {
        model_id: model_id.to_string(),
        glb_source,
        transform: ModelTransform::default(),
        material_policy: RuntimeMaterialPolicy::PreserveEmbedded,
    }
}

fn make_glb_with_json(json: &str) -> Vec<u8> {
    let mut json_chunk = json.as_bytes().to_vec();
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }

    let total_len = 12 + 8 + json_chunk.len();
    let mut bytes = Vec::with_capacity(total_len);

    bytes.extend_from_slice(&0x46546C67_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(total_len as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x4E4F534A_u32.to_le_bytes());
    bytes.extend_from_slice(&json_chunk);

    bytes
}

fn valid_minimal_json() -> &'static str {
    r#"{
      "asset": { "version": "2.0" },
      "scenes": [{ "nodes": [0] }],
      "nodes": [{ "mesh": 0 }],
      "meshes": [{ "primitives": [{ "attributes": {} }] }]
    }"#
}

fn unique_temp_path(extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("signex_renderer_pcb3d_runtime_{nanos}.{extension}"))
}

#[test]
fn pcb3d_runtime_glb_ingest_rejects_non_glb_path_extension() {
    let source = GlbSource::FilePath(PathBuf::from("/tmp/model.step"));
    let err = ingest_runtime_glb(request("U1", source)).expect_err("STEP path must be rejected");

    match err {
        RuntimeGlbIngestError::UnsupportedSourceFormat {
            model_id,
            expected_extension,
            ..
        } => {
            assert_eq!(model_id, "U1");
            assert_eq!(expected_extension, "glb");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn pcb3d_runtime_glb_ingest_reports_missing_cache_entry() {
    let path = unique_temp_path("glb");
    let source = GlbSource::FilePath(path.clone());
    let err = ingest_runtime_glb(request("U2", source)).expect_err("missing cache should fail");

    match err {
        RuntimeGlbIngestError::MissingGlbCacheEntry {
            model_id,
            path: missing_path,
        } => {
            assert_eq!(model_id, "U2");
            assert_eq!(missing_path, path);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn pcb3d_runtime_glb_ingest_rejects_invalid_header_bytes() {
    let source = GlbSource::Bytes(vec![1, 2, 3, 4]);
    let err = ingest_runtime_glb(request("U3", source)).expect_err("invalid bytes must fail");

    match err {
        RuntimeGlbIngestError::InvalidGlb { model_id, reason } => {
            assert_eq!(model_id, "U3");
            assert!(reason.contains("header") || reason.contains("magic"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn pcb3d_runtime_glb_ingest_rejects_payload_without_meshes() {
    let json = r#"{
      "asset": { "version": "2.0" },
      "scenes": [{ "nodes": [0] }],
      "nodes": [{ "name": "empty" }]
    }"#;
    let source = GlbSource::Bytes(make_glb_with_json(json));
    let err = ingest_runtime_glb(request("U4", source)).expect_err("missing meshes must fail");

    match err {
        RuntimeGlbIngestError::InvalidGlb { model_id, reason } => {
            assert_eq!(model_id, "U4");
            assert!(reason.contains("mesh"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn pcb3d_runtime_glb_ingest_accepts_minimal_valid_payload_from_bytes() {
    let source = GlbSource::Bytes(make_glb_with_json(valid_minimal_json()));
    let model = ingest_runtime_glb(request("U5", source)).expect("valid GLB bytes should ingest");

    assert_eq!(model.model_id, "U5");
    assert_eq!(model.metadata.asset_version, "2.0");
    assert_eq!(model.metadata.scene_count, 1);
    assert_eq!(model.metadata.node_count, 1);
    assert_eq!(model.metadata.mesh_count, 1);
}

#[test]
fn pcb3d_runtime_glb_ingest_accepts_minimal_valid_payload_from_path() {
    let path = unique_temp_path("glb");
    let bytes = make_glb_with_json(valid_minimal_json());
    fs::write(&path, bytes).expect("must write test glb payload");

    let source = GlbSource::FilePath(path.clone());
    let model = ingest_runtime_glb(request("U6", source)).expect("valid GLB file should ingest");

    assert_eq!(model.model_id, "U6");
    assert_eq!(model.metadata.mesh_count, 1);

    fs::remove_file(path).expect("must remove temp test glb");
}
