use super::*;
use crate::managed_block::{BEGIN_MARKER, END_MARKER};
use std::path::Path;

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

/// Git ignores a path when a pattern matches it or any directory above it.
fn ignored(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    (1..=parts.len()).any(|n| {
        let prefix = parts[..n].join("/");
        block_patterns().iter().any(|p| glob_matches(p, &prefix))
    })
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
fn the_real_atomic_write_temp_name_is_matched() {
    let tmp = crate::atomic_io::tmp_path_for(Path::new("sub/Board.snxsch")).unwrap();
    let name = tmp.to_str().unwrap().replace('\\', "/");
    assert!(name.ends_with(".tmp"), "{name}");
    assert!(ignored(&name), "{name} should be ignored");
}

#[test]
fn a_directory_named_like_a_temp_file_hides_what_is_inside() {
    // Git semantics: an ignored directory ignores its contents.
    assert!(ignored("x.tmp/Board.snxsch"));
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

fn legacy_gitattributes(use_lfs: bool) -> String {
    let mut t = String::from(
        "# Generated by Signex when version control was enabled for this project.\n\
         # Re-running \"Enable Version Control\" rewrites this file from scratch \u{2014}\n\
         # manual edits will be lost. Hand-edit only if you're not planning to\n\
         # re-run the modal.\n\n\
         # Native Signex formats are line-based UTF-8 \u{2014} keep LF endings everywhere\n\
         # so cross-platform collaborators don't churn the diff with CRLF flips.\n",
    );
    for ext in [
        "snxsch", "snxpcb", "snxprj", "snxmat", "snxnet", "snxbom", "snxout", "snxsym", "snxfpt",
        "snxlib", "snxmod",
    ] {
        t.push_str(&format!("*.{ext}\ttext eol=lf\n"));
    }
    t.push_str("\n# Binary attachments \u{2014} git shouldn't diff or merge them.\n");
    for ext in ["step", "stp", "wrl", "iges", "png", "jpg", "jpeg", "pdf"] {
        t.push_str(&format!("*.{ext}\tbinary\n"));
    }
    if use_lfs {
        t.push_str("\n# 3D models opt-in via Git LFS so the working tree doesn't bloat.\n");
        t.push_str("assets/3d-models/**\tfilter=lfs diff=lfs merge=lfs -text\n");
    }
    t
}

#[test]
fn a_legacy_generated_gitattributes_is_replaced_by_the_block_not_appended_to() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, legacy_gitattributes(true)).unwrap();

    ensure_gitattributes(dir.path(), true).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(BEGIN_MARKER), "{text}");
    assert!(!text.contains("rewrites this file from scratch"));
    assert_eq!(text.matches("*.snxsch").count(), 1, "{text}");
    assert!(text.ends_with(&format!("{END_MARKER}\n")));
    ensure_gitattributes(dir.path(), true).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn legacy_cleanup_keeps_every_user_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    let mut legacy = legacy_gitattributes(false);
    legacy.push_str("*.mine text\n");
    std::fs::write(&path, &legacy).unwrap();

    ensure_gitattributes(dir.path(), false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# >>> signex"), "{text}");
    assert!(text.ends_with("\n\n*.mine text\n"), "{text}");
}

#[test]
fn a_user_file_that_merely_mentions_a_spec_line_is_not_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, "*.snxsch\ttext eol=lf\n*.x text\n").unwrap();
    ensure_gitattributes(dir.path(), false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# >>> signex"), "{text}");
    assert!(
        text.ends_with("\n\n*.snxsch\ttext eol=lf\n*.x text\n"),
        "{text}"
    );
}

#[test]
fn a_read_only_file_is_refused_and_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, "mine\n").unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();

    let err = ensure_project_gitignore(dir.path()).unwrap_err();
    assert!(matches!(err, ManagedFileError::ReadOnly { .. }), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine\n");
    assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_refused_and_the_target_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret");
    std::fs::write(&secret, "TOP SECRET\n").unwrap();
    for name in [".gitignore", ".gitattributes"] {
        std::os::unix::fs::symlink(&secret, dir.path().join(name)).unwrap();
    }

    for result in [
        ensure_project_gitignore(dir.path()),
        ensure_gitattributes(dir.path(), true),
    ] {
        assert!(matches!(
            result,
            Err(ManagedFileError::NotRegularFile { .. })
        ));
    }
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "TOP SECRET\n");
    for name in [".gitignore", ".gitattributes"] {
        let meta = std::fs::symlink_metadata(dir.path().join(name)).unwrap();
        assert!(meta.file_type().is_symlink(), "{name} must stay a link");
    }
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_and_a_fifo_are_refused_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(dir.path().join("nowhere"), dir.path().join(".gitignore")).unwrap();
    assert!(matches!(
        ensure_project_gitignore(dir.path()),
        Err(ManagedFileError::NotRegularFile { .. })
    ));

    let fifo = dir.path().join(".gitattributes");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(matches!(
        ensure_gitattributes(dir.path(), false),
        Err(ManagedFileError::NotRegularFile { .. })
    ));
}

#[cfg(unix)]
#[test]
fn a_hard_linked_file_is_refused_and_both_names_keep_their_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    let twin = dir.path().join("twin");
    std::fs::write(&path, "mine\n").unwrap();
    std::fs::hard_link(&path, &twin).unwrap();

    let err = ensure_project_gitignore(dir.path()).unwrap_err();
    assert!(matches!(err, ManagedFileError::HardLinked { .. }), "{err}");
    assert_eq!(std::fs::read_to_string(&twin).unwrap(), "mine\n");
}

/// What `git check-attr <attr> -- <path>` reports, or `None` without a git CLI.
fn git_attr(dir: &Path, attr: &str, path: &str) -> Option<String> {
    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .output()
        .ok()?;
    assert!(init.status.success());
    let out = std::process::Command::new("git")
        .args(["check-attr", attr, "--", path])
        .current_dir(dir)
        .output()
        .ok()?;
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    Some(text.rsplit(": ").next().unwrap().trim().to_owned())
}

#[test]
fn a_users_own_attribute_still_wins_after_the_block_is_merged() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".gitattributes"), "*.pdf diff=pdf\n").unwrap();
    ensure_gitattributes(dir.path(), false).unwrap();
    let text = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert!(text.starts_with(BEGIN_MARKER), "block goes first:\n{text}");
    assert!(
        text.ends_with("*.pdf diff=pdf\n"),
        "user line kept:\n{text}"
    );
    let Some(diff) = git_attr(dir.path(), "diff", "x.pdf") else {
        eprintln!("git CLI unavailable; attribute lookup skipped");
        return;
    };
    assert_eq!(
        diff, "pdf",
        "the block must not override the user's diff=pdf"
    );
}

#[test]
fn a_new_gitattributes_block_goes_after_the_bom_with_one_separator_in_the_files_eol() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, "\u{feff}*.pdf diff=pdf\r\n").unwrap();
    ensure_gitattributes(dir.path(), false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with(&format!("\u{feff}{BEGIN_MARKER}\r\n")),
        "{text:?}"
    );
    assert!(
        text.ends_with(&format!("{END_MARKER}\r\n\r\n*.pdf diff=pdf\r\n")),
        "{text:?}"
    );
    ensure_gitattributes(dir.path(), false).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "idempotent");
}

#[test]
fn an_existing_gitattributes_block_is_replaced_where_it_sits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    let old = format!("*.a x\n{BEGIN_MARKER}\nold\n{END_MARKER}\n*.b y\n");
    std::fs::write(&path, &old).unwrap();
    ensure_gitattributes(dir.path(), false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with("*.a x\n") && text.ends_with("*.b y\n"),
        "{text}"
    );
    assert!(!text.contains("\nold\n"));
}

#[test]
fn a_legacy_lfs_models_line_is_dropped_when_lfs_is_turned_off() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, legacy_gitattributes(true)).unwrap();
    ensure_gitattributes(dir.path(), false).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("filter=lfs"), "{text}");
}

#[test]
fn a_users_gitignore_negation_stays_below_the_block() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, "!keep/*.tmp\n").unwrap();
    ensure_project_gitignore(dir.path()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(BEGIN_MARKER), "{text}");
    assert!(
        text.ends_with(&format!("{END_MARKER}\n\n!keep/*.tmp\n")),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn merging_keeps_the_files_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, "mine\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    ensure_project_gitignore(dir.path()).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn a_write_that_cannot_create_its_temp_file_is_a_write_error_and_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, "mine\n").unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores directory permissions; there the write cannot be made to fail.
    let can_still_write = std::fs::File::create(dir.path().join("probe")).is_ok();

    let result = ensure_project_gitignore(dir.path());

    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    if can_still_write {
        return;
    }
    assert!(
        matches!(result, Err(ManagedFileError::Write { .. })),
        "{result:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine\n");
}
