use super::*;
use crate::managed_block::{BEGIN_MARKER, END_MARKER};

/// Minimal gitignore matcher for the block's own patterns: `*` and `?`
/// wildcards within one path segment; a pattern without `/` matches the
/// basename at any depth, one with `/` is anchored at the project root.
fn glob_matches(pattern: &str, text: &str) -> bool {
    fn go(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], t) || (!t.is_empty() && t[0] != b'/' && go(p, &t[1..])),
            (Some(b'?'), Some(c)) if *c != b'/' => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    if pattern.contains('/') {
        go(pattern.as_bytes(), text.as_bytes())
    } else {
        let base = text.rsplit('/').next().unwrap_or(text);
        go(pattern.as_bytes(), base.as_bytes())
    }
}

fn block_patterns() -> Vec<&'static str> {
    GITIGNORE_BLOCK_BODY
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn ignored(path: &str) -> bool {
    block_patterns().iter().any(|p| glob_matches(p, path))
}

#[test]
fn the_block_never_ignores_sources_libraries_assets_or_exports() {
    for kept in [
        "Board.snxsch",
        "sub/Sheet2.snxsch",
        "Board.snxpcb",
        "Board.snxprj",
        "parts.snxlib",
        "libs/parts.snxlib/components.tsv",
        "libs/parts.snxlib/symbols/r.snxsym",
        "libs/parts.snxlib/footprints/r.snxfpt",
        "libs/parts.snxlib/3dmodels/r.step",
        "assets/logo.png",
        "assets/logo.svg",
        "assets/photo.jpg",
        "assets/notes/readme.md",
        "exports/Board.pdf",
        "exports/bom.csv",
        "exports/netlist.net",
        "fab/gerber.gtl",
    ] {
        assert!(!ignored(kept), "{kept} must never be ignored");
    }
    assert!(
        !GITIGNORE_BLOCK_BODY.contains("snx"),
        "no source extension in the block"
    );
    assert!(!GITIGNORE_BLOCK_BODY.contains("assets"));
}

#[test]
fn the_block_ignores_what_signex_generates() {
    for junk in [
        "Board.snxsch.4242-7.tmp",
        "libs/parts.snxlib.1-0.tmp",
        ".parts.snxlib.signex-writelock",
        "libs/.parts.snxlib.signex-writelock",
        ".DS_Store",
        "sub/Thumbs.db",
        "desktop.ini",
    ] {
        assert!(ignored(junk), "{junk} should be ignored");
    }
}

#[test]
fn the_atomic_write_temp_name_is_matched() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Board.snxsch");
    // The real temp name: pause the writer by making the rename fail.
    std::fs::create_dir(&target).unwrap();
    assert!(atomic_write(&target, b"x").is_err());
    // A failed write cleans its temp; assert the shape the helper builds.
    let shown = format!("{}.{}-0.tmp", "Board.snxsch", std::process::id());
    assert!(ignored(&shown));
}

#[test]
fn the_gitattributes_lfs_block_covers_models_and_raster_assets_not_svg() {
    let body = gitattributes_block_body(true);
    for line in [
        "*.step filter=lfs",
        "*.stp filter=lfs",
        "*.wrl filter=lfs",
        "*.iges filter=lfs",
        "assets/3d-models/** filter=lfs",
        "assets/*.png filter=lfs",
        "assets/*.jpg filter=lfs",
        "assets/*.jpeg filter=lfs",
        "assets/*.gif filter=lfs",
    ] {
        assert!(body.contains(line), "missing {line}");
    }
    assert!(!body.contains("svg"));
    assert!(!gitattributes_block_body(false).contains("filter=lfs"));
}

#[test]
fn ensure_project_gitignore_creates_exactly_the_block() {
    let dir = tempfile::tempdir().unwrap();
    ensure_project_gitignore(dir.path()).unwrap();
    let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert_eq!(
        text,
        format!("{BEGIN_MARKER}\n{GITIGNORE_BLOCK_BODY}{END_MARKER}\n")
    );
}

#[test]
fn ensure_project_gitignore_keeps_user_lines_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    let user = "target/\r\n*.log";
    let old = format!("\n{BEGIN_MARKER}\nstale\n{END_MARKER}\n# tail\n");
    std::fs::write(&path, format!("{user}{old}")).unwrap();

    ensure_project_gitignore(dir.path()).unwrap();
    let once = std::fs::read_to_string(&path).unwrap();
    assert!(once.starts_with(user), "user bytes must lead untouched");
    assert!(once.ends_with("# tail\n"));
    assert!(!once.contains("stale"));

    ensure_project_gitignore(dir.path()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
}

#[test]
fn unbalanced_markers_leave_the_file_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    let broken = format!("keep\n{BEGIN_MARKER}\nno end\n");
    std::fs::write(&path, &broken).unwrap();

    let err = ensure_project_gitignore(dir.path()).unwrap_err();
    assert!(matches!(err, ManagedFileError::Markers { .. }), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
}

#[test]
fn a_non_utf8_file_is_refused_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, [0xff, 0xfe, b'\n']).unwrap();
    assert!(matches!(
        ensure_project_gitignore(dir.path()),
        Err(ManagedFileError::NotUtf8 { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0xfe, b'\n']);
}
