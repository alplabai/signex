//! Shared config-root resolver for signex's on-disk preference files.
//!
//! Four files persist independently under the same OS-native config
//! directory: `prefs.json` ([`crate::fonts`]), `keyboard_shortcuts.toml`
//! (`crate::keymap::profile`), `distributors.toml`
//! (`crate::library::settings::persistence`), and `global_libraries.toml`
//! (`crate::panels::components_panel::global_prefs`). Each of those
//! modules used to compute `dirs::config_dir().join("signex")` itself;
//! this module hoists that one shared computation — and its test
//! redirect — so there is a single place that decides *where* signex's
//! config directory is. Each file keeps its own name, its own `None`
//! fallback, and its own error handling (issue #440).
//!
//! Resolves to:
//! - Windows: `%APPDATA%\signex\`
//! - macOS:   `~/Library/Application Support/signex/`
//! - Linux:   `$XDG_CONFIG_HOME/signex/` (or `~/.config/signex/`)
//!
//! `None` when the platform has no config directory to offer at all
//! (`dirs::config_dir()` returned `None` — rare: a stripped-down sandbox
//! or CI runner, a daemon with neither `$HOME` nor `$XDG_CONFIG_HOME`).
//! None of the four callers has a CWD fallback on `None` — each decides
//! its own behaviour; see their own doc comments.

use std::path::PathBuf;

/// True when the test/dev redirect gate is active for this build:
/// signex-app's own unit-test build (`cfg(test)`) or the
/// `test-prefs-redirect` feature (activated for every crate that links
/// signex-app as a dev-dependency — see `Cargo.toml`). Written once here
/// so no call site re-derives the predicate. [`config_root`] uses it
/// directly; [`crate::fonts`] also uses it to decide whether its
/// one-shot legacy-prefs migration should run at all — that migration
/// must not touch the developer's real config directory during a test
/// run (issue #437).
pub(crate) fn is_test_redirect_active() -> bool {
    cfg!(test) || cfg!(feature = "test-prefs-redirect")
}

/// Resolve the directory signex's per-user config files live under.
///
/// Under the test/dev redirect ([`is_test_redirect_active`]), returns one
/// shared per-process tempdir instead of the real OS config directory —
/// `<tmp>/signex-test-prefs-<pid>/` — so all four config files land under
/// the same throwaway root during a test run and none of them read or
/// write the developer's real config directory. Originally
/// `fonts::prefs_path`-only (#437), hoisted to cover all four resolvers
/// in #440.
pub fn config_root() -> Option<PathBuf> {
    if is_test_redirect_active() {
        return Some(
            std::env::temp_dir().join(format!("signex-test-prefs-{}", std::process::id())),
        );
    }
    dirs::config_dir().map(|dir| config_root_for_dir(&dir))
}

/// Join the `signex` subdirectory onto an arbitrary base directory.
///
/// [`config_root`] uses this for the real OS config dir, and it's the
/// one place the two `config_path_for_dir` test helpers
/// (`keymap::profile`, `library::settings::persistence`) get the same
/// join from — before #440 hoisted this, production went through
/// `config_root()` while those two test helpers still hardcoded
/// `base.join("signex")` themselves, so a rename of the folder here
/// would have silently diverged production from the tests that are
/// supposed to prove it (#440 review).
pub fn config_root_for_dir(base: &std::path::Path) -> PathBuf {
    base.join("signex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_is_active_under_the_crates_own_test_build() {
        assert!(is_test_redirect_active());
    }

    #[test]
    fn resolves_to_the_shared_per_process_tempdir_under_test() {
        let root = config_root().expect("test redirect always resolves");
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("signex-test-prefs-"))
        );
    }
}
