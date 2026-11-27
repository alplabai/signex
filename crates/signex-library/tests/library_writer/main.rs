// Standalone binary: declared as a `[[test]]` with `required-features` in Cargo.toml.
//! The bulk library writer (#3110) and the library checker (#3111).

#![cfg(feature = "local-git")]

mod check;
mod synthetic;
mod update;
mod write;

use std::collections::BTreeMap;
use std::path::Path;

/// Every file under `root` but the write-lock sidecars, relative path with
/// `/` -> bytes.
pub fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if !signex_library::is_write_lock_file(&path) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
