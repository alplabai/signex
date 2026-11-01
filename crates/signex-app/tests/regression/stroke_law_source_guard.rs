//! #1858 — source guard: no per-kind zoom-to-stroke curve comes back.
//!
//! Every schematic and symbol stroke goes through the one law in
//! `signex_types::schematic::stroke`. These identifiers belonged to the
//! per-kind px curves it replaced; any of them reappearing means a stroke kind
//! has its own zoom rule again. Screen-constant chrome (selection outlines,
//! handles, axes) is named `*_OUTLINE_PX` / `*_LINE_PX`, never `*_STROKE_PX`.

use std::path::{Path, PathBuf};

const BANNED: [&str; 5] = [
    "stroke_px_at_zoom",
    "PIN_STROKE_SELECTED_PX",
    "stroke_world_mm",
    "SCHEMATIC_RENDER_STROKE_MAX_SCALE_MULTIPLIER",
    "_STROKE_PX_AT_100",
];

/// The surface and PCB floors are the only `*_STROKE_PX` names allowed.
const ALLOWED_STROKE_PX: [&str; 1] = ["MIN_STROKE_PX"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// `*_STROKE_PX` identifiers on `line` other than the allowed floors.
fn stroke_px_names(line: &str) -> Vec<String> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.ends_with("_STROKE_PX") && !ALLOWED_STROKE_PX.contains(word))
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_per_kind_stroke_curve_returns_1858() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for dir in ["signex-app/src", "signex-types/src"] {
        rust_files(&crates.join(dir), &mut files);
    }
    assert!(files.len() > 100, "the guard must scan the real sources");
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read source");
        for (n, line) in text.lines().enumerate() {
            let mut names: Vec<String> = BANNED
                .iter()
                .filter(|banned| line.contains(*banned))
                .map(|banned| (*banned).to_owned())
                .collect();
            names.extend(stroke_px_names(line));
            for name in names {
                hits.push(format!("{}:{}: {name}", file.display(), n + 1));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "stroke kinds no longer share one zoom law (#1858): a per-kind stroke curve is back; \
         route the width through signex_types::schematic::stroke instead:\n{}",
        hits.join("\n")
    );
}
