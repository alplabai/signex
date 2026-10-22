//! The import cache must never serve a stale or partial GLB (#147).

use std::path::Path;
use std::time::{Duration, SystemTime};

use signex_3d_model_importer::{ModelImportRequest, import_model};

const WRL_A: &str = "#VRML V2.0 utf8\nShape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] } }\n";
// Same byte length as WRL_A, different geometry.
const WRL_B: &str = "#VRML V2.0 utf8\nShape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0, 2 0 0, 0 2 0 ] } coordIndex [ 0 1 2 -1 ] } }\n";

fn import(dir: &Path) -> signex_3d_model_importer::ModelImportResult {
    import_model(ModelImportRequest {
        model_id: "t".to_owned(),
        source_path: dir.join("part.wrl"),
        cache_dir: dir.join("cache"),
        converter_version: "0.0.0-test",
    })
    .expect("import")
}

fn set_mtime(path: &Path, t: SystemTime) {
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(t).unwrap();
}

#[test]
fn same_size_edit_within_one_second_gets_a_fresh_glb() {
    assert_eq!(WRL_A.len(), WRL_B.len());
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("part.wrl");
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);

    std::fs::write(&src, WRL_A).unwrap();
    set_mtime(&src, t0);
    let first = import(tmp.path());
    assert!(!first.cache_hit);
    let first_bytes = std::fs::read(&first.glb_path).unwrap();

    std::fs::write(&src, WRL_B).unwrap();
    set_mtime(&src, t0 + Duration::from_millis(400));
    let second = import(tmp.path());
    assert!(
        !second.cache_hit,
        "sub-second edit must not hit the stale entry"
    );
    assert_ne!(first.glb_path, second.glb_path);
    assert_ne!(first_bytes, std::fs::read(&second.glb_path).unwrap());
}

#[test]
fn truncated_cache_entry_is_regenerated_not_served() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("part.wrl"), WRL_A).unwrap();
    let first = import(tmp.path());
    let good = std::fs::read(&first.glb_path).unwrap();

    std::fs::write(&first.glb_path, &good[..good.len() / 2]).unwrap();
    let second = import(tmp.path());
    assert!(!second.cache_hit, "partial GLB must be a miss");
    assert_eq!(std::fs::read(&second.glb_path).unwrap(), good);

    // Non-empty garbage is also a miss.
    std::fs::write(&second.glb_path, b"garbage").unwrap();
    let third = import(tmp.path());
    assert!(!third.cache_hit);
    assert_eq!(std::fs::read(&third.glb_path).unwrap(), good);

    // And an intact entry is still a hit.
    assert!(import(tmp.path()).cache_hit);
}

#[test]
fn cache_write_leaves_no_temp_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("part.wrl"), WRL_A).unwrap();
    import(tmp.path());
    let names: Vec<String> = std::fs::read_dir(tmp.path().join("cache"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 1, "unexpected cache contents: {names:?}");
    assert!(names[0].ends_with(".glb"));
}
