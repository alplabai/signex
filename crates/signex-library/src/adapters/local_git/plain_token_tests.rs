//! A saved library whose database `auth` is a hand-written token warns that
//! the token is stored in plain text: once per adapter, never for an empty
//! auth, a keychain reference or an `${env:NAME}` reference (#1363).

use std::sync::atomic::Ordering;

use super::write_oracle_tests::{Rng, World, random_row};
use super::*;
use crate::identity::InternalPn;
use crate::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

/// Saves two rows into a library whose database mode carries `auth`, and
/// returns how many plain-text warnings the adapter logged.
fn warnings_after_two_saves(auth: &str) -> usize {
    crate::git_identity::test_identity::install();
    let mut rng = Rng(11);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..2).map(|_| rng.uuid()).collect(),
        footprints: (0..2).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("L").join("L.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: w.lib,
        library: LibrarySection {
            name: "L".into(),
            description: None,
        },
        mode: LibraryMode::Database {
            url: "https://lib.example/api".into(),
            auth: auth.into(),
        },
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let opts = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(&file, manifest, opts).unwrap();
    for pn in ["PN-1", "PN-2"] {
        let id = rng.uuid();
        let mut row = random_row(&mut rng, &w, id);
        row.internal_pn = InternalPn::new(pn);
        adapter.insert_row("t", row, "save").unwrap();
    }
    adapter.plain_token_warnings.load(Ordering::Relaxed)
}

#[test]
fn a_raw_token_warns_exactly_once_across_saves() {
    assert_eq!(warnings_after_two_saves("s3cr3t-token"), 1);
}

#[test]
fn no_auth_a_keychain_reference_or_an_env_reference_never_warns() {
    assert_eq!(warnings_after_two_saves(""), 0);
    assert_eq!(warnings_after_two_saves("@signex-keychain:lab"), 0);
    assert_eq!(warnings_after_two_saves("${env:SIGNEX_DB_TOKEN}"), 0);
}
