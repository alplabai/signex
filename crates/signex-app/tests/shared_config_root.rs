//! Guard (#440): the four independent on-disk prefs resolvers —
//! `fonts::prefs_path()`, `keymap::config_path()`,
//! `library::settings::persistence::config_path()`, and
//! `panels::components_panel::global_prefs::prefs_path()` — must all
//! resolve under the one shared `config_root::config_root()`, and that
//! root itself must resolve under the OS temp dir during a test run
//! (never the developer's real config directory).
//!
//! The #440 branch that hoisted `config_root()` proved this by hand
//! with scratch tests deleted before commit, so nothing in the
//! committed tree failed if a fifth resolver bypassed `config_root()`,
//! or if one of the four was repointed back at `dirs::config_dir()`
//! directly. This file is the permanent replacement. `fonts::
//! prefs_path()` is private and unreachable here — its half of the
//! guard is the unit test in `src/fonts/mod.rs`.
//!
//! Discriminates: reverting `keymap/profile.rs`'s `config_path()` to
//! `dirs::config_dir().map(|b| config_path_for_dir(&b))` makes
//! `keymap_config_path_lives_under_shared_root` fail (it resolves
//! outside the temp-dir root, or panics `Signex settings header`-free
//! against the developer's real config dir) — see the commit message
//! for the red/green run.

#[test]
fn config_root_resolves_under_the_os_temp_dir_during_tests() {
    let root = signex_app::config_root::config_root().expect("test redirect always resolves");
    assert!(
        root.starts_with(std::env::temp_dir()),
        "config_root() must resolve under the OS temp dir during a test run, got {}",
        root.display()
    );
}

#[test]
fn keymap_config_path_lives_under_shared_root() {
    let root = signex_app::config_root::config_root().expect("test redirect always resolves");
    let path = signex_app::keymap::config_path().expect("resolves under the test redirect");
    assert!(
        path.starts_with(&root),
        "keymap::config_path() must live under config_root(), got {} (root {})",
        path.display(),
        root.display()
    );
}

#[test]
fn distributors_config_path_lives_under_shared_root() {
    let root = signex_app::config_root::config_root().expect("test redirect always resolves");
    let path = signex_app::library::settings::persistence::config_path()
        .expect("resolves under the test redirect");
    assert!(
        path.starts_with(&root),
        "library::settings::persistence::config_path() must live under config_root(), got {} (root {})",
        path.display(),
        root.display()
    );
}

#[test]
fn global_libraries_prefs_path_lives_under_shared_root() {
    let root = signex_app::config_root::config_root().expect("test redirect always resolves");
    let path = signex_app::panels::components_panel::global_prefs::prefs_path()
        .expect("resolves under the test redirect");
    assert!(
        path.starts_with(&root),
        "global_prefs::prefs_path() must live under config_root(), got {} (root {})",
        path.display(),
        root.display()
    );
}
