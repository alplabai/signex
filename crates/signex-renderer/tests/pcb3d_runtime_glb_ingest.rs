//! Integration tests for Milestone C runtime GLB ingest hooks.

use signex_gfx::scene::Scene;
use signex_renderer::pcb3d::{
    emit_opaque_pass_preview, ingest_runtime_glb, GlbSource, ModelTransform, OpaquePassLayout,
    RuntimeGlbIngestError, RuntimeGlbIngestRequest, RuntimeMaterialPolicy,
};
use signex_renderer::theme::ResolvedTheme;
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
    assert_eq!(model.metadata.mesh_primitive_count, 1);
    assert_eq!(model.metadata.opaque_instance_count, 1);
    assert_eq!(model.mesh_staging.opaque_primitives.len(), 1);
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
        assert_eq!(model.mesh_staging.opaque_primitives.len(), 1);

    fs::remove_file(path).expect("must remove temp test glb");
}

#[test]
fn pcb3d_runtime_glb_ingest_stages_mesh_primitives_for_scene_nodes() {
        let json = r#"{
            "asset": { "version": "2.0" },
            "scenes": [{ "nodes": [0] }],
            "nodes": [
                { "mesh": 0, "children": [1] },
                { "mesh": 1 }
            ],
            "meshes": [
                { "primitives": [
                    { "attributes": {} },
                    { "attributes": {}, "material": 3 }
                ] },
                { "primitives": [
                    { "attributes": {}, "material": 8 }
                ] }
            ]
        }"#;

        let source = GlbSource::Bytes(make_glb_with_json(json));
        let model = ingest_runtime_glb(request("U7", source)).expect("valid staged mesh payload");

        assert_eq!(model.metadata.mesh_count, 2);
        assert_eq!(model.metadata.mesh_primitive_count, 3);
        assert_eq!(model.metadata.opaque_instance_count, 3);
        assert_eq!(model.mesh_staging.opaque_primitives.len(), 3);

        let first = model.mesh_staging.opaque_primitives[0];
        assert_eq!(first.scene_index, 0);
        assert_eq!(first.node_index, 0);
        assert_eq!(first.mesh_index, 0);
        assert_eq!(first.primitive_index, 0);
        assert_eq!(first.material_index, None);

        let second = model.mesh_staging.opaque_primitives[1];
        assert_eq!(second.mesh_index, 0);
        assert_eq!(second.primitive_index, 1);
        assert_eq!(second.material_index, Some(3));

        let third = model.mesh_staging.opaque_primitives[2];
        assert_eq!(third.node_index, 1);
        assert_eq!(third.mesh_index, 1);
        assert_eq!(third.primitive_index, 0);
        assert_eq!(third.material_index, Some(8));
}

#[test]
fn pcb3d_runtime_glb_ingest_rejects_out_of_range_mesh_index() {
        let json = r#"{
            "asset": { "version": "2.0" },
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 4 }],
            "meshes": [{ "primitives": [{ "attributes": {} }] }]
        }"#;

        let source = GlbSource::Bytes(make_glb_with_json(json));
        let err = ingest_runtime_glb(request("U8", source)).expect_err("out-of-range mesh index must fail");

        match err {
                RuntimeGlbIngestError::InvalidGlb { model_id, reason } => {
                        assert_eq!(model_id, "U8");
                        assert!(reason.contains("mesh index") || reason.contains("out of range"));
                }
                other => panic!("unexpected error: {other:?}"),
        }
}

#[test]
fn pcb3d_runtime_glb_opaque_pass_preview_emits_one_polygon_per_staged_primitive() {
        let json = r#"{
            "asset": { "version": "2.0" },
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{ "primitives": [
                { "attributes": {} },
                { "attributes": {}, "material": 2 }
            ] }]
        }"#;

        let source = GlbSource::Bytes(make_glb_with_json(json));
        let model = ingest_runtime_glb(request("U9", source)).expect("valid staged payload for opaque pass");
        let mut scene = Scene::default();
        let theme = ResolvedTheme::builtin_default();

        emit_opaque_pass_preview(&model, &theme, &mut scene, OpaquePassLayout::default());

        assert_eq!(scene.polygons.len(), model.mesh_staging.opaque_primitives.len());
        assert_eq!(scene.polygons.len(), 2);
        assert_eq!(scene.polygons[0].vertices.len(), 4);
        assert_eq!(scene.polygons[1].vertices.len(), 4);
        assert!(scene.polygons[1].vertices[0][0] > scene.polygons[0].vertices[0][0]);
        assert!(scene.polygons[0].stroke_color.is_some());
}
