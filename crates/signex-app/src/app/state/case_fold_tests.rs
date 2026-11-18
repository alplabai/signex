//! `CaseFold` (`app/core/case_fold.rs`) through the key function it feeds,
//! `scope::path_key_with`; kept outside the core, which cannot name it.

use std::path::Path;

use crate::app::core::case_fold::CaseFold;
use crate::app::state::scope::path_key_with;

#[test]
fn folding_case_does_not_make_a_backslash_a_separator() {
    // APFS: case-insensitive, but `\` is an ordinary filename character.
    let apfs = CaseFold::case_only();

    assert_ne!(
        path_key_with(Path::new("/w/a\\b.snxsch"), apfs),
        path_key_with(Path::new("/w/a/b.snxsch"), apfs),
        "`a\\b.snxsch` and `a/b.snxsch` are two different files"
    );
    assert_eq!(
        path_key_with(Path::new("/w/Top.snxsch"), apfs),
        path_key_with(Path::new("/w/top.snxsch"), apfs),
        "the case rule still folds"
    );
}

#[test]
fn nfc_and_nfd_spellings_are_one_key_only_where_the_volume_normalises() {
    let nfc = Path::new("/w/caf\u{e9}.snxsch");
    let nfd = Path::new("/w/cafe\u{301}.snxsch");
    assert_eq!(
        path_key_with(nfc, CaseFold::apfs()),
        path_key_with(nfd, CaseFold::apfs())
    );
    assert_ne!(
        path_key_with(nfc, CaseFold::FOLD),
        path_key_with(nfd, CaseFold::FOLD),
        "NTFS keeps the two forms as two names"
    );
}

#[test]
fn dot_and_dotdot_segments_resolve_without_touching_the_disk() {
    let fold = CaseFold::PRESERVE;
    let want = path_key_with(Path::new("/w/p/Top.snxsch"), fold);
    for spelling in [
        "/w/p/./Top.snxsch",
        "/w/p/sub/../Top.snxsch",
        "/w//p/Top.snxsch",
        "/w/p/sub/./../Top.snxsch",
    ] {
        assert_eq!(path_key_with(Path::new(spelling), fold), want, "{spelling}");
    }
    assert_eq!(path_key_with(Path::new("/.."), fold), "/");
    assert_eq!(path_key_with(Path::new("/"), fold), "/");
    assert_eq!(path_key_with(Path::new("../a"), fold), "../a");
    assert_eq!(path_key_with(Path::new("a/../../b"), fold), "../b");
    assert_eq!(path_key_with(Path::new("C:/../x"), CaseFold::FOLD), "c:/x");
}

#[test]
fn the_separator_rule_stays_host_derived_whatever_the_probe_says() {
    for ignores_case in [true, false] {
        assert_eq!(
            CaseFold::from_probe(ignores_case).backslash_is_separator,
            cfg!(windows)
        );
        assert_eq!(CaseFold::from_probe(ignores_case).fold_case, ignores_case);
    }
}

#[test]
fn a_case_sensitive_directory_on_a_windows_host_keeps_the_backslash_rule() {
    // NTFS per-directory case sensitivity: case preserved, `\` still a
    // separator on the Windows host. Built by hand so it runs anywhere.
    let ntfs_sensitive = CaseFold {
        fold_case: false,
        backslash_is_separator: true,
        normalize_unicode: false,
    };

    assert_eq!(
        path_key_with(Path::new("W\\Sub\\A.snxsch"), ntfs_sensitive),
        "W/Sub/A.snxsch"
    );
    assert_ne!(
        path_key_with(Path::new("W/A.snxsch"), ntfs_sensitive),
        path_key_with(Path::new("W/a.snxsch"), ntfs_sensitive)
    );
}
