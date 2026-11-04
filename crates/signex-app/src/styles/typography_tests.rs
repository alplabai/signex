//! Scale values, base tracking, and the no-literal guard for swept files.

use super::typography::{DEFAULT_BASE_SIZE, Spacing, TypeScale, chrome_px};

#[test]
fn default_base_is_twelve_px_and_body_text_is_twelve() {
    assert_eq!(DEFAULT_BASE_SIZE, 12.0);
    assert_eq!(TypeScale::default().body, 12.0);
}

#[test]
fn an_eleven_px_base_reproduces_the_pre_token_text_sizes_exactly() {
    let ts = TypeScale::from_base(11.0);
    assert_eq!(
        [ts.caption, ts.body, ts.label, ts.title, ts.heading],
        [10.0, 11.0, 12.0, 13.0, 14.0]
    );
}

#[test]
fn default_spacing_is_the_two_four_eight_twelve_sixteen_grid_at_an_eleven_px_base() {
    let sp = Spacing::from_base(11.0);
    assert_eq!(
        [sp.xs, sp.s, sp.m, sp.l, sp.xl],
        [2.0, 4.0, 8.0, 12.0, 16.0]
    );
}

#[test]
fn chrome_px_scales_a_fixed_length_with_the_base() {
    assert_eq!(chrome_px(11.0), DEFAULT_BASE_SIZE);
    assert!((chrome_px(40.0) - 40.0 * 12.0 / 11.0).abs() < 1e-4);
}

#[test]
fn scales_track_a_changed_base() {
    let ts = TypeScale::from_base(22.0);
    assert_eq!(ts.body, 22.0);
    assert_eq!(ts.heading, 28.0);
    let sp = Spacing::from_base(22.0);
    assert_eq!([sp.xs, sp.xl], [4.0, 32.0]);
}

#[test]
fn every_step_keeps_its_authored_ratio_to_body_at_any_base() {
    for base in [11.0_f32, 12.0, 22.0] {
        let ts = TypeScale::from_base(base);
        for (step, authored_px) in [
            (ts.caption, 10.0),
            (ts.body, 11.0),
            (ts.label, 12.0),
            (ts.title, 13.0),
            (ts.heading, 14.0),
            (ts.display, 20.0),
            (ts.glyph, 24.0),
            (ts.micro, 4.0),
            (ts.banner, 64.0),
        ] {
            assert!(
                (step / ts.body - authored_px / 11.0).abs() < 1e-4,
                "base {base}: step {step} is not {authored_px}/11 of body"
            );
        }
    }
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

/// Names of `const NAME: f32 = <number>;` items in `src`: a text size
/// routed through one of these is still a hardcoded literal.
fn literal_f32_consts(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| {
            let l = l
                .trim_start()
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub(super) ")
                .trim_start_matches("pub ");
            let rest = l.strip_prefix("const ")?;
            let (name, value) = rest.split_once(": f32 =")?;
            value
                .trim_start()
                .starts_with(|c: char| c.is_ascii_digit())
                .then(|| name.trim().to_string())
        })
        .collect()
}

/// Every `.size(<literal>)` under `src/` (the widget text size), or `.size(NAME)` where `NAME` is a
/// numeric `f32` const. A size is
/// a `TypeScale` step, so it follows the base size; padding and spacing
/// literals are still allowed outside `SWEPT`.
#[test]
fn no_text_size_literal_survives_anywhere_in_signex_app() {
    fn walk(dir: &std::path::Path, bad: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("src dir readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, bad);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "typography_tests.rs")
            {
                let src = std::fs::read_to_string(&path).expect("source readable");
                let consts = literal_f32_consts(&src);
                for (n, line) in src.lines().enumerate() {
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    let mut rest = line;
                    while let Some(at) = rest.find(".size(") {
                        let after = &rest[at + ".size(".len()..];
                        let ident: String = after
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect();
                        if after.starts_with(|c: char| c.is_ascii_digit())
                            || consts.contains(&ident)
                        {
                            bad.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                        }
                        rest = after;
                    }
                }
            }
        }
    }
    let mut bad = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut bad,
    );
    assert!(
        bad.is_empty(),
        "hardcoded text size; use a TypeScale step:\n{}",
        bad.join("\n")
    );
}
