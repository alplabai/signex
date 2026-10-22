//! Commit-author identity resolution shared by every git entry point
//! (library adapter, project adapter, "Enable Version Control").
//!
//! Durable rule: a commit is authored by the active user, never by a
//! generic Signex identity. When no user name and email resolve, callers get an
//! error they surface to the user and the commit is not made.

use crate::adapter::LibraryError;

/// Resolve the commit identity for `repo`. Name and email are each
/// resolved independently, first hit wins, in git's own order:
///
/// 1. `GIT_AUTHOR_NAME` / `GIT_AUTHOR_EMAIL`, then
///    `GIT_COMMITTER_NAME` / `GIT_COMMITTER_EMAIL` (env overrides config).
/// 2. git config `user.name` / `user.email` (repo, then global).
///
/// There is no OS-user or hostname fallback: a missing name or email is
/// an error the caller shows the user, and no commit is made.
pub fn identity_for_repo(repo: &git2::Repository) -> Result<(String, String), LibraryError> {
    resolve_identity(repo.config().ok().as_ref(), |k| std::env::var(k).ok())
}

/// Same rule as [`identity_for_repo`] for callers that have no repo yet
/// (checks the default git config chain).
pub fn identity_for_default_config() -> Result<(String, String), LibraryError> {
    resolve_identity(git2::Config::open_default().ok().as_ref(), |k| {
        std::env::var(k).ok()
    })
}

/// Pure resolver behind the helpers above; `env` is injected so the
/// "nothing configured" path is testable without touching the process
/// environment.
pub fn resolve_identity(
    cfg: Option<&git2::Config>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<(String, String), LibraryError> {
    let non_empty = |s: String| (!s.trim().is_empty()).then_some(s);
    let cfg_get = |key: &str| cfg.and_then(|c| c.get_string(key).ok()).and_then(non_empty);
    let env_get = |key: &str| env(key).and_then(non_empty);

    let name = env_get("GIT_AUTHOR_NAME")
        .or_else(|| env_get("GIT_COMMITTER_NAME"))
        .or_else(|| cfg_get("user.name"));
    let email = env_get("GIT_AUTHOR_EMAIL")
        .or_else(|| env_get("GIT_COMMITTER_EMAIL"))
        .or_else(|| cfg_get("user.email"));
    let (Some(name), Some(email)) = (name, email) else {
        return Err(LibraryError::Backend(
            "git identity not configured - set both your name and email with ".to_string()
                + "`git config --global user.name \"...\"` and "
                + "`git config --global user.email \"...\"` (or GIT_AUTHOR_NAME and "
                + "GIT_AUTHOR_EMAIL). Signex won't author commits with a generic or guessed identity.",
        ));
    };
    Ok((name, email))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn no_identity_anywhere_is_an_error_not_a_generic_author() {
        let err = resolve_identity(None, no_env).unwrap_err();
        assert!(err.to_string().contains("identity not configured"));
    }

    #[test]
    fn blank_values_do_not_count_as_an_identity() {
        let env = |k: &str| (k == "GIT_AUTHOR_NAME").then(|| "  ".to_string());
        assert!(resolve_identity(None, env).is_err());
    }

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn a_name_without_an_email_is_refused_not_synthesised() {
        let env = env_of(&[("GIT_AUTHOR_NAME", "Ada"), ("COMPUTERNAME", "box")]);
        assert!(resolve_identity(None, env).is_err());
    }

    #[test]
    fn os_user_and_email_hints_are_not_an_identity() {
        let env = env_of(&[
            ("USER", "ada"),
            ("USERNAME", "ada"),
            ("EMAIL", "ada@example.org"),
            ("HOSTNAME", "box"),
        ]);
        assert!(resolve_identity(None, env).is_err());
    }

    #[test]
    fn env_identity_alone_resolves() {
        let env = env_of(&[
            ("GIT_AUTHOR_NAME", "Ada"),
            ("GIT_AUTHOR_EMAIL", "ada@example.org"),
        ]);
        assert_eq!(
            resolve_identity(None, env).unwrap(),
            ("Ada".to_string(), "ada@example.org".to_string())
        );
    }

    #[test]
    fn env_wins_over_config_like_git() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = git2::Config::open(&dir.path().join("cfg")).unwrap();
        cfg.set_str("user.name", "Config Name").unwrap();
        cfg.set_str("user.email", "config@example.org").unwrap();
        let env = env_of(&[
            ("GIT_AUTHOR_NAME", "Env Name"),
            ("GIT_AUTHOR_EMAIL", "env@example.org"),
        ]);
        assert_eq!(
            resolve_identity(Some(&cfg), env).unwrap(),
            ("Env Name".to_string(), "env@example.org".to_string())
        );
        // Config still fills what the env leaves unset.
        let only_name = env_of(&[("GIT_AUTHOR_NAME", "Env Name")]);
        assert_eq!(
            resolve_identity(Some(&cfg), only_name).unwrap(),
            ("Env Name".to_string(), "config@example.org".to_string())
        );
    }
}

/// Hermetic commit identity for tests that make real git commits.
///
/// Since #141 no commit is authored without an explicit identity, so a test
/// machine or CI runner with no `~/.gitconfig` would refuse them all. This
/// points libgit2's *global* config at a throwaway file holding a fixed test
/// identity. Do not use it from a test that asserts the no-identity path.
#[cfg(any(test, feature = "test-identity"))]
pub mod test_identity {
    use std::sync::Once;

    /// Install the test identity once per process; later calls are no-ops.
    pub fn install() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let dir = std::env::temp_dir().join(format!("signex-test-git-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("create test git config dir");
            std::fs::write(
                dir.join(".gitconfig"),
                "[user]
	name = Signex Test
	email = test@example.org
",
            )
            .expect("write test .gitconfig");
            // SAFETY: runs once, before any test of this process commits;
            // it only sets libgit2's config search path.
            unsafe {
                git2::opts::set_search_path(git2::ConfigLevel::Global, &dir)
                    .expect("set libgit2 global config path");
            }
        });
    }
}
