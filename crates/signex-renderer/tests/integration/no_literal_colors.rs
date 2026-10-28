//! Lint-style regression test for renderer runtime source.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                stack.push(child);
                continue;
            }

            if child.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(child);
            }
        }
    }

    files.sort();
    files
}

fn runtime_source_section(source: &str) -> &str {
    source.split("\n#[cfg(test)]").next().unwrap_or(source)
}

#[test]
fn renderer_runtime_source_rejects_literal_color_blocks() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let rgba_literal_re = Regex::new(
        r"\[\s*(?:\d+\.\d+|\d+)\s*,\s*(?:\d+\.\d+|\d+)\s*,\s*(?:\d+\.\d+|\d+)\s*,\s*(?:\d+\.\d+|\d+)\s*\]",
    )
    .expect("valid rgba regex");

    let mut violations = Vec::new();

    for file in collect_rust_files(&src_root) {
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        let runtime = runtime_source_section(&source);

        if runtime.contains("from_rgb(") || runtime.contains("from_rgba(") {
            violations.push(format!(
                "{}: disallowed literal color constructor",
                file.display()
            ));
        }

        if let Some(m) = rgba_literal_re.find(runtime) {
            let line = runtime[..m.start()].bytes().filter(|b| *b == b'\n').count() + 1;
            violations.push(format!(
                "{}:{}: disallowed RGBA literal block {}",
                file.display(),
                line,
                m.as_str()
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "renderer runtime source contains forbidden color literals:\n{}",
        violations.join("\n")
    );
}
