use signex_3d_model_importer::{import_model, ModelImportRequest, SourceFormat};

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
    assert_eq!(u32::from_le_bytes(glb[4..8].try_into().unwrap()), 2, "GLB version mismatch");
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
