//! Integration tests for runtime bridge: source model -> importer -> GLB ingest.

use signex_renderer::pcb3d::{
    ingest_runtime_model_with_bridge, ModelTransform, RuntimeMaterialPolicy,
    RuntimeModelBridgeError, RuntimeModelBridgeRequest, RuntimeModelSource,
};
use std::fs;
use std::path::{Path, PathBuf};

fn bridge_request(
    model_id: &str,
    source: RuntimeModelSource,
    cache_dir: PathBuf,
) -> RuntimeModelBridgeRequest {
    RuntimeModelBridgeRequest {
        model_id: model_id.to_owned(),
        source,
        cache_dir,
        converter_version: "0.1.0",
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

fn write_tier0_wrl(path: &Path) {
    fs::write(
        path,
        b"#VRML V2.0 utf8\n\
Shape {\n\
  geometry IndexedFaceSet {\n\
    coord Coordinate {\n\
      point [ 0 0 0, 1 0 0, 0 1 0 ]\n\
    }\n\
    coordIndex [ 0 1 2 -1 ]\n\
  }\n\
}\n",
    )
    .expect("write vrml fixture");
}

fn write_tier0_step(path: &Path) {
    let step = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('simple triangle'),'2;1');
ENDSEC;
DATA;
#1 = CARTESIAN_POINT('',(0.,0.,0.));
#2 = CARTESIAN_POINT('',(1.,0.,0.));
#3 = CARTESIAN_POINT('',(0.,1.,0.));
#10 = VERTEX_POINT('',#1);
#11 = VERTEX_POINT('',#2);
#12 = VERTEX_POINT('',#3);
#20 = POLY_LOOP('',(#10,#11,#12));
#21 = FACE_OUTER_BOUND('',#20,.T.);
#30 = ADVANCED_FACE('',(#21),#999,.T.);
ENDSEC;
END-ISO-10303-21;
"#;
    fs::write(path, step).expect("write step fixture");
}

#[test]
fn pcb3d_runtime_bridge_passes_through_glb_path_without_conversion() {
    let dir = tempfile::tempdir().expect("tempdir");
    let glb_path = dir.path().join("tier0.glb");
    let cache_dir = dir.path().join("cache");

    fs::write(&glb_path, make_glb_with_json(valid_minimal_json())).expect("write glb fixture");

    let result = ingest_runtime_model_with_bridge(bridge_request(
        "B1",
        RuntimeModelSource::FilePath(glb_path.clone()),
        cache_dir,
    ))
    .expect("bridge ingest should succeed");

    assert!(!result.conversion_performed);
    assert!(!result.cache_hit);
    assert_eq!(result.resolved_glb_path, Some(glb_path));
    assert_eq!(result.model.metadata.mesh_count, 1);
}

#[test]
fn pcb3d_runtime_bridge_accepts_glb_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache_dir = dir.path().join("cache");

    let result = ingest_runtime_model_with_bridge(bridge_request(
        "B2",
        RuntimeModelSource::GlbBytes(make_glb_with_json(valid_minimal_json())),
        cache_dir,
    ))
    .expect("bridge ingest from bytes should succeed");

    assert!(!result.conversion_performed);
    assert_eq!(result.resolved_glb_path, None);
    assert_eq!(result.model.metadata.mesh_count, 1);
}

#[test]
fn pcb3d_runtime_bridge_converts_vrml_to_glb_and_ingests() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vrml_path = dir.path().join("tier0.wrl");
    let cache_dir = dir.path().join("cache");
    write_tier0_wrl(&vrml_path);

    let result = ingest_runtime_model_with_bridge(bridge_request(
        "B3",
        RuntimeModelSource::FilePath(vrml_path),
        cache_dir,
    ))
    .expect("bridge ingest for vrml should succeed");

    assert!(result.conversion_performed);
    assert_eq!(result.model.metadata.mesh_count, 1);
    let resolved = result
        .resolved_glb_path
        .expect("resolved glb path should be returned");
    assert_eq!(resolved.extension().and_then(|e| e.to_str()), Some("glb"));
    assert!(resolved.exists());
}

#[test]
fn pcb3d_runtime_bridge_converts_step_to_glb_and_ingests() {
    let dir = tempfile::tempdir().expect("tempdir");
    let step_path = dir.path().join("tier0.step");
    let cache_dir = dir.path().join("cache");
    write_tier0_step(&step_path);

    let result = ingest_runtime_model_with_bridge(bridge_request(
        "B4",
        RuntimeModelSource::FilePath(step_path),
        cache_dir,
    ))
    .expect("bridge ingest for step should succeed");

    assert!(result.conversion_performed);
    assert_eq!(result.model.metadata.mesh_count, 1);
    let resolved = result
        .resolved_glb_path
        .expect("resolved glb path should be returned");
    assert_eq!(resolved.extension().and_then(|e| e.to_str()), Some("glb"));
    assert!(resolved.exists());
}

#[test]
fn pcb3d_runtime_bridge_reports_import_failure_for_missing_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing_path = dir.path().join("missing.step");
    let cache_dir = dir.path().join("cache");

    let err = ingest_runtime_model_with_bridge(bridge_request(
        "B5",
        RuntimeModelSource::FilePath(missing_path.clone()),
        cache_dir,
    ))
    .expect_err("missing source should fail");

    match err {
        RuntimeModelBridgeError::ImportFailed { model_id, path, reason } => {
            assert_eq!(model_id, "B5");
            assert_eq!(path, missing_path);
            assert!(reason.contains("source file not found"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
