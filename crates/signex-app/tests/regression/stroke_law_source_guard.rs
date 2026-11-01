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

/// The schematic and Symbol Editor render paths and the schematic types.
/// PCB and footprint code keeps its own floors and is not scanned.
const GUARDED: [&str; 7] = [
    "signex-app/src/schematic_runtime",
    "signex-app/src/canvas",
    "signex-app/src/library/editor/symbol",
    "signex-app/src/scene_line_snap.rs",
    "signex-app/src/scene_shader.rs",
    "signex-app/src/renderer_scene_canvas.rs",
    "signex-types/src/schematic",
];

fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        out.push(path.to_path_buf());
        return;
    }
    for entry in std::fs::read_dir(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    {
        let path = entry.expect("dir entry").path();
        if path.is_dir() || path.extension().is_some_and(|ext| ext == "rs") {
            rust_files(&path, out);
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
    for guarded in GUARDED {
        let path = crates.join(guarded);
        assert!(
            path.exists(),
            "guarded path {guarded} is gone; update the guard"
        );
        rust_files(&path, &mut files);
    }
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read source");
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
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
