//! Feeds arbitrary bytes to the 3D model importer. The first byte picks the
//! format by file extension (`.step` / `.wrl` / `.gltf` / `.glb`), the rest is
//! the file body. Any `Err` is fine; a panic, overflow, or hang is the bug.
#![no_main]

use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;
use signex_3d_model_importer::{ModelImportRequest, import_model};

const EXTENSIONS: [&str; 4] = ["step", "wrl", "gltf", "glb"];

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    let ext = EXTENSIONS[usize::from(selector) % EXTENSIONS.len()];

    // One scratch dir per process; the cache is wiped each run so a cache hit
    // never short-circuits the parser.
    let root: PathBuf = std::env::temp_dir().join(format!("signex-fuzz-{}", std::process::id()));
    let cache_dir = root.join("cache");
    let _ = std::fs::remove_dir_all(&cache_dir);
    if std::fs::create_dir_all(&cache_dir).is_err() {
        return;
    }
    let source_path = root.join(format!("model.{ext}"));
    if std::fs::write(&source_path, body).is_err() {
        return;
    }

    let _ = import_model(ModelImportRequest {
        model_id: "fuzz".to_string(),
        source_path,
        cache_dir,
        converter_version: "fuzz",
    });
});
