//! Scale values, base tracking, and the no-literal guard for swept files.

use super::typography::{DEFAULT_BASE_SIZE, Spacing, TypeScale};

#[test]
fn default_scale_reproduces_the_pre_token_text_sizes_exactly() {
    let ts = TypeScale::default();
    assert_eq!(
        [ts.caption, ts.body, ts.label, ts.title, ts.heading],
        [10.0, 11.0, 12.0, 13.0, 14.0]
    );
}

#[test]
fn default_spacing_is_the_two_four_eight_twelve_sixteen_grid() {
    let sp = Spacing::default();
    assert_eq!(
        [sp.xs, sp.s, sp.m, sp.l, sp.xl],
        [2.0, 4.0, 8.0, 12.0, 16.0]
    );
}

#[test]
fn scales_track_a_changed_base() {
    let ts = TypeScale::from_base(DEFAULT_BASE_SIZE * 2.0);
    assert_eq!(ts.body, 22.0);
    assert_eq!(ts.heading, 28.0);
    let sp = Spacing::from_base(DEFAULT_BASE_SIZE * 2.0);
    assert_eq!([sp.xs, sp.xl], [4.0, 32.0]);
}

#[test]
fn scale_steps_are_strictly_increasing() {
    let ts = TypeScale::from_base(13.0);
    assert!(
        ts.caption < ts.body && ts.body < ts.label && ts.label < ts.title && ts.title < ts.heading
    );
    let sp = Spacing::from_base(13.0);
    assert!(sp.xs < sp.s && sp.s < sp.m && sp.m < sp.l && sp.l < sp.xl);
}

/// Files fully migrated onto the scale tokens (paths under `src/`). Add a
/// file here in the same commit that sweeps it.
const SWEPT: &[&str] = &[
    "library/close_prompt.rs",
    "library/document_options.rs",
    "library/settings/distributor_apis.rs",
    "library/editor/footprint/step_attach.rs",
    "library/editor/footprint/pads_active_bar.rs",
    "library/editor/footprint/move_by_modal.rs",
    "library/editor/footprint/align_modal.rs",
];

/// Calls whose argument must come from the scale, not a literal.
const GUARDED: &[&str] = &[
    ".size(",
    ".text_size(",
    ".padding(",
    ".spacing(",
    "Space::new().width(",
    "Space::new().height(",
];

/// Non-zero numeric literals in `arg` (zero is "no padding", always fine).
fn has_nonzero_literal(arg: &str) -> bool {
    let b = arg.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let prev_ident = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if c.is_ascii_digit() && !prev_ident {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if arg[start..i]
                .chars()
                .any(|d| d.is_ascii_digit() && d != '0')
            {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// `(line, snippet)` of every guarded call in `src` with a literal argument.
fn literal_violations(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for pat in GUARDED {
            let mut rest = line;
            while let Some(at) = rest.find(pat) {
                let after = &rest[at + pat.len()..];
                let mut depth = 1;
                let end = after
                    .char_indices()
                    .find(|&(_, c)| {
                        match c {
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            _ => {}
                        }
                        depth == 0
                    })
                    .map_or(after.len(), |(i, _)| i);
                if has_nonzero_literal(&after[..end]) {
                    out.push((n + 1, line.trim().to_string()));
                }
                rest = after;
            }
        }
    }
    out
}

#[test]
fn guard_flags_literals_and_accepts_tokens() {
    assert!(!literal_violations("text(x).size(11)").is_empty());
    assert!(!literal_violations("pick_list(a, b, c).text_size(11)").is_empty());
    assert!(!literal_violations("c.padding([4, sp.m])").is_empty());
    assert!(!literal_violations("Space::new().height(4.5)").is_empty());
    assert!(!literal_violations("row![].spacing(ts.label * 2.0)").is_empty());
    assert!(literal_violations("text(x).size(ts.body)").is_empty());
    assert!(literal_violations("c.padding([0, sp.l]).spacing(0)").is_empty());
    assert!(literal_violations("// .size(11) in a comment").is_empty());
}

#[test]
fn swept_files_use_scale_tokens_not_literals() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut bad = Vec::new();
    for rel in SWEPT {
        let src = std::fs::read_to_string(root.join(rel)).expect("swept file readable");
        for (line, snippet) in literal_violations(&src) {
            bad.push(format!("{rel}:{line}: {snippet}"));
        }
    }
    assert!(
        bad.is_empty(),
        "hardcoded size/padding/spacing in swept files; use TypeScale/Spacing:\n{}",
        bad.join("\n")
    );
}
