use signex_3d_model_importer::{ModelImportRequest, SourceFormat, import_model};

fn write_tier0_wrl(path: &std::path::Path) {
    std::fs::write(
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
    .expect("write fixture");
}

fn write_tier1_wrl(path: &std::path::Path) {
    // Two-body component: IC body + single pad (two Shape nodes)
    std::fs::write(
        path,
        b"#VRML V2.0 utf8\n\
Shape {\n\
  appearance Appearance {\n\
    material Material { diffuseColor 0.1 0.1 0.1 }\n\
  }\n\
  geometry IndexedFaceSet {\n\
    coord Coordinate {\n\
      point [ 0 0 0, 4 0 0, 4 1 0, 0 1 0 ]\n\
    }\n\
    coordIndex [ 0 1 2 -1, 0 2 3 -1 ]\n\
  }\n\
}\n\
Shape {\n\
  appearance Appearance {\n\
    material Material { diffuseColor 0.8 0.6 0.0 }\n\
  }\n\
  geometry IndexedFaceSet {\n\
    coord Coordinate {\n\
      point [ 0 0 0, 0.5 0 0, 0.5 0.5 0 ]\n\
    }\n\
    coordIndex [ 0 1 2 -1 ]\n\
  }\n\
}\n",
    )
    .expect("write tier1 fixture");
}

fn write_tier0_gltf(path: &std::path::Path, bin_path: &std::path::Path) {
    // POSITION (3 vertices) + INDEX (1 triangle)
    let mut bin = Vec::new();

    // positions: (0,0,0), (1,0,0), (0,1,0)
    for f in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bin.extend_from_slice(&f.to_le_bytes());
    }
    // indices: 0,1,2 as u32
    for i in [0u32, 1, 2] {
        bin.extend_from_slice(&i.to_le_bytes());
    }

    std::fs::write(bin_path, &bin).expect("write gltf bin fixture");

    let gltf = format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "scene": 0,
  "scenes": [{{ "nodes": [0] }}],
  "nodes": [{{ "mesh": 0 }}],
  "meshes": [{{ "primitives": [{{ "attributes": {{ "POSITION": 0 }}, "indices": 1, "mode": 4 }}] }}],
  "accessors": [
  {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0] }},
  {{ "bufferView": 1, "componentType": 5125, "count": 3, "type": "SCALAR" }}
  ],
  "bufferViews": [
  {{ "buffer": 0, "byteOffset": 0, "byteLength": 36, "target": 34962 }},
  {{ "buffer": 0, "byteOffset": 36, "byteLength": 12, "target": 34963 }}
  ],
  "buffers": [{{ "uri": "{}", "byteLength": {} }}]
}}"#,
        bin_path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("bin filename"),
        bin.len()
    );

    std::fs::write(path, gltf).expect("write gltf fixture");
}

fn write_tier0_step(path: &std::path::Path) {
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
    std::fs::write(path, step).expect("write step fixture");
}

#[test]
fn import_tier0_vrml_produces_glb() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier0.wrl");
    let cache = dir.path().join("cache");
    write_tier0_wrl(&src);

    let result = import_model(ModelImportRequest {
        model_id: "tier0".into(),
        source_path: src.clone(),
        cache_dir: cache.clone(),
        converter_version: "0.1.0",
    })
    .expect("import failed");

    assert!(!result.cache_hit);
    assert!(result.glb_path.exists());
    assert_eq!(result.metadata.source_format, SourceFormat::Vrml);
    assert_eq!(result.metadata.mesh_count, 1);
    assert!(result.metadata.byte_len > 0);
}

#[test]
fn import_tier0_glb_has_valid_magic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier0.wrl");
    let cache = dir.path().join("cache");
    write_tier0_wrl(&src);

    let result = import_model(ModelImportRequest {
        model_id: "tier0_magic".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    })
    .expect("import failed");

    let glb = std::fs::read(&result.glb_path).expect("read glb");
    assert_eq!(&glb[0..4], b"glTF", "GLB magic mismatch");
    assert_eq!(
        u32::from_le_bytes(glb[4..8].try_into().unwrap()),
        2,
        "GLB version mismatch"
    );
}

#[test]
fn import_cache_hit_on_second_call() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier0.wrl");
    let cache = dir.path().join("cache");
    write_tier0_wrl(&src);

    let r1 = import_model(ModelImportRequest {
        model_id: "cache_test".into(),
        source_path: src.clone(),
        cache_dir: cache.clone(),
        converter_version: "0.1.0",
    })
    .expect("first import failed");
    assert!(!r1.cache_hit);

    let r2 = import_model(ModelImportRequest {
        model_id: "cache_test".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    })
    .expect("second import failed");
    assert!(r2.cache_hit, "expected cache hit on second call");
    assert_eq!(r1.glb_path, r2.glb_path);
}

#[test]
fn import_cache_miss_on_version_bump() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier0.wrl");
    let cache = dir.path().join("cache");
    write_tier0_wrl(&src);

    let r1 = import_model(ModelImportRequest {
        model_id: "version_bump".into(),
        source_path: src.clone(),
        cache_dir: cache.clone(),
        converter_version: "0.1.0",
    })
    .expect("import v1 failed");

    let r2 = import_model(ModelImportRequest {
        model_id: "version_bump".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.2.0",
    })
    .expect("import v2 failed");

    assert!(!r2.cache_hit, "expected cache miss after version bump");
    assert_ne!(r1.glb_path, r2.glb_path);
}

#[test]
fn import_tier1_two_body_produces_two_meshes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier1.wrl");
    let cache = dir.path().join("cache");
    write_tier1_wrl(&src);

    let result = import_model(ModelImportRequest {
        model_id: "tier1".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    })
    .expect("import failed");

    assert_eq!(result.metadata.mesh_count, 2, "expected 2 mesh bodies");
}

#[test]
fn import_tier0_gltf_produces_glb() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier0.gltf");
    let bin = dir.path().join("tier0.bin");
    let cache = dir.path().join("cache");
    write_tier0_gltf(&src, &bin);

    let result = import_model(ModelImportRequest {
        model_id: "gltf_tier0".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    })
    .expect("gltf import failed");

    assert_eq!(result.metadata.source_format, SourceFormat::Gltf);
    assert_eq!(result.metadata.mesh_count, 1);
    assert_eq!(result.metadata.primitive_count, 1);
    let glb = std::fs::read(&result.glb_path).expect("read glb");
    assert_eq!(&glb[0..4], b"glTF");
}

#[test]
fn import_tier0_step_produces_glb() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("tier0.step");
    let cache = dir.path().join("cache");
    write_tier0_step(&src);

    let result = import_model(ModelImportRequest {
        model_id: "step_tier0".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    })
    .expect("step import failed");

    assert_eq!(result.metadata.source_format, SourceFormat::Step);
    assert_eq!(result.metadata.mesh_count, 1);
    let glb = std::fs::read(&result.glb_path).expect("read glb");
    assert_eq!(&glb[0..4], b"glTF");
}

#[test]
fn import_unsupported_format_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("model.obj");
    std::fs::write(&src, b"# obj file\n").expect("write");
    let cache = dir.path().join("cache");

    let err = import_model(ModelImportRequest {
        model_id: "unsupported".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    });
    assert!(matches!(
        err,
        Err(signex_3d_model_importer::ModelImportError::UnsupportedFormat { .. })
    ));
}

#[test]
fn import_missing_source_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("does_not_exist.wrl");
    let cache = dir.path().join("cache");

    let err = import_model(ModelImportRequest {
        model_id: "missing".into(),
        source_path: src,
        cache_dir: cache,
        converter_version: "0.1.0",
    });
    assert!(matches!(
        err,
        Err(signex_3d_model_importer::ModelImportError::SourceNotFound { .. })
    ));
}
