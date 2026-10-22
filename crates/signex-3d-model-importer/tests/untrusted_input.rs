//! Hostile / malformed input must be refused with a typed error, never read
//! files outside the model directory, overflow the stack, or panic (#112).

use std::path::Path;

use signex_3d_model_importer::{ModelImportError, ModelImportRequest, import_model};

fn import(
    dir: &Path,
    name: &str,
) -> Result<signex_3d_model_importer::ModelImportResult, ModelImportError> {
    import_model(ModelImportRequest {
        model_id: "t".to_owned(),
        source_path: dir.join(name),
        cache_dir: dir.join("cache"),
        converter_version: "0.0.0-test",
    })
}

fn gltf_with_buffer(uri: &str) -> String {
    format!(
        r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"byteLength":4,"uri":{}}}]}}"#,
        serde_json::to_string(uri).unwrap()
    )
}

fn gltf_with_image(uri: &str) -> String {
    format!(
        r#"{{"asset":{{"version":"2.0"}},"images":[{{"uri":{}}}]}}"#,
        serde_json::to_string(uri).unwrap()
    )
}

/// Layout: `<tmp>/secret.bin` (outside) and `<tmp>/models/` (the model dir).
fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let models = tmp.path().join("models");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(tmp.path().join("secret.bin"), b"TOPSECRET").unwrap();
    std::fs::write(tmp.path().join("secret.png"), b"TOPSECRET").unwrap();
    (tmp, models)
}

fn expect_gltf_rejection(models: &Path, json: &str) {
    std::fs::write(models.join("m.gltf"), json).unwrap();
    match import(models, "m.gltf") {
        Err(ModelImportError::GltfParseFailed { reason, .. }) => {
            assert!(reason.contains("rejected"), "unexpected reason: {reason}")
        }
        Ok(_) => panic!("hostile uri was accepted"),
        Err(other) => panic!("wrong error: {other}"),
    }
    assert!(
        !models.join("cache").exists()
            || std::fs::read_dir(models.join("cache"))
                .unwrap()
                .next()
                .is_none(),
        "nothing may be cached for a rejected model"
    );
}

#[test]
fn buffer_uri_parent_traversal_is_rejected() {
    let (_tmp, models) = fixture();
    expect_gltf_rejection(&models, &gltf_with_buffer("../secret.bin"));
}

#[test]
fn buffer_uri_nested_parent_traversal_is_rejected() {
    let (_tmp, models) = fixture();
    expect_gltf_rejection(&models, &gltf_with_buffer("sub/../../secret.bin"));
}

#[test]
fn buffer_uri_absolute_path_is_rejected() {
    let (tmp, models) = fixture();
    let abs = tmp.path().join("secret.bin");
    expect_gltf_rejection(&models, &gltf_with_buffer(&abs.to_string_lossy()));
    expect_gltf_rejection(&models, &gltf_with_buffer("/etc/passwd"));
}

#[test]
fn buffer_uri_scheme_or_drive_is_rejected() {
    let (_tmp, models) = fixture();
    expect_gltf_rejection(&models, &gltf_with_buffer("file:///etc/passwd"));
    expect_gltf_rejection(&models, &gltf_with_buffer("C:\\Windows\\win.ini"));
}

#[test]
fn image_uri_traversal_and_absolute_are_rejected() {
    let (tmp, models) = fixture();
    expect_gltf_rejection(&models, &gltf_with_image("../secret.png"));
    let abs = tmp.path().join("secret.png");
    expect_gltf_rejection(&models, &gltf_with_image(&abs.to_string_lossy()));
}

#[cfg(unix)]
#[test]
fn symlink_escaping_model_dir_is_rejected() {
    let (tmp, models) = fixture();
    std::os::unix::fs::symlink(tmp.path().join("secret.bin"), models.join("link.bin")).unwrap();
    expect_gltf_rejection(&models, &gltf_with_buffer("link.bin"));
}

#[test]
fn legitimate_relative_buffer_and_data_uris_still_work() {
    let (_tmp, models) = fixture();
    std::fs::create_dir_all(models.join("bin")).unwrap();
    std::fs::write(models.join("bin").join("geo.bin"), [0u8; 4]).unwrap();
    std::fs::write(models.join("m.gltf"), gltf_with_buffer("bin/geo.bin")).unwrap();
    import(&models, "m.gltf").expect("relative buffer imports");

    let data = gltf_with_buffer("data:application/octet-stream;base64,AAECAw==");
    std::fs::write(models.join("d.gltf"), data).unwrap();
    import(&models, "d.gltf").expect("data uri imports");
}

#[test]
fn missing_image_inside_model_dir_is_a_warning_not_an_error() {
    let (_tmp, models) = fixture();
    std::fs::write(models.join("m.gltf"), gltf_with_image("tex.png")).unwrap();
    let result = import(&models, "m.gltf").expect("imports");
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn deeply_nested_vrml_is_a_typed_error_not_a_stack_overflow() {
    let (_tmp, models) = fixture();
    let depth = 20_000;
    let mut src = String::from("#VRML V2.0 utf8\n");
    for _ in 0..depth {
        src.push_str("Transform { children [ ");
    }
    for _ in 0..depth {
        src.push_str("] }");
    }
    std::fs::write(models.join("deep.wrl"), src).unwrap();
    match import(&models, "deep.wrl") {
        Err(ModelImportError::VrmlParseFailed { reason, .. }) => {
            assert!(reason.contains("nesting"), "unexpected reason: {reason}")
        }
        Ok(_) => panic!("deep nesting accepted"),
        Err(other) => panic!("wrong error: {other}"),
    }
}

#[test]
fn deeply_nested_def_chain_is_bounded() {
    let (_tmp, models) = fixture();
    let src = "DEF a ".repeat(20_000) + "Shape { }";
    std::fs::write(models.join("def.wrl"), src).unwrap();
    assert!(matches!(
        import(&models, "def.wrl"),
        Err(ModelImportError::VrmlParseFailed { .. })
    ));
}

#[test]
fn moderately_nested_vrml_still_imports() {
    let (_tmp, models) = fixture();
    let depth = 100;
    let mut src = String::from("#VRML V2.0 utf8\n");
    for _ in 0..depth {
        src.push_str("Transform { children [ ");
    }
    src.push_str("Shape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }");
    for _ in 0..depth {
        src.push_str("] }");
    }
    std::fs::write(models.join("ok.wrl"), src).unwrap();
    import(&models, "ok.wrl").expect("depth 100 imports");
}

#[test]
fn truncated_and_garbage_inputs_error_without_panicking() {
    let (_tmp, models) = fixture();
    let cases: &[(&str, &[u8])] = &[
        (
            "trunc.wrl",
            b"Shape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0",
        ),
        ("garbage.wrl", &[0xff, 0xfe, 0x00, 0x01]),
        ("empty.stp", b""),
        (
            "trunc.stp",
            b"ISO-10303-21;\nDATA;\n#1 = CARTESIAN_POINT('',(0.,0",
        ),
        ("garbage.stp", b"DATA;\n#x = ;\nENDSEC;"),
        ("trunc.gltf", br#"{"asset":{"version":"2.0""#),
        ("empty.gltf", b""),
    ];
    for (name, bytes) in cases {
        std::fs::write(models.join(name), bytes).unwrap();
        assert!(import(&models, name).is_err(), "{name} should error");
    }
}
