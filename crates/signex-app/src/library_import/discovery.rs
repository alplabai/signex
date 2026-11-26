//! Which library converters are installed (contract §1). Nothing is listed
//! by name: a converter is any program that answers `describe --json` with a
//! supported contract, found
//!
//! 1. in a single-converter environment variable ([`SINGLE_CONVERTER_ENVS`]),
//! 2. in [`CONVERTER_LIST_ENV`], a path list of converter programs,
//! 3. on `PATH` (absolute entries only), named `signex-<format>-import`.
//!
//! The same program found twice (by canonical path) is listed once.
//! Blocking: it runs every candidate's `describe`; call from a worker.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::describe::ConverterDescription;
use super::error::ImportError;
use super::run::{ConverterCommand, run_describe};

/// A path list (`PATH` syntax) of converter programs to offer.
pub const CONVERTER_LIST_ENV: &str = "SIGNEX_LIBRARY_CONVERTERS";

/// Variables that each name one converter program.
pub const SINGLE_CONVERTER_ENVS: &[&str] = &[crate::altium_import::ENV_OVERRIDE];

/// How long one candidate's `describe` may take during discovery.
pub const DISCOVERY_DESCRIBE_TIMEOUT: Duration = Duration::from_secs(5);

const NAME_PREFIX: &str = "signex-";
const NAME_SUFFIX: &str = "-import";

/// The environment discovery reads, so tests can supply their own.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryEnv {
    pub path: Option<OsString>,
    pub list: Option<OsString>,
    /// `(variable, value)` for each set [`SINGLE_CONVERTER_ENVS`] entry.
    pub singles: Vec<(String, OsString)>,
}

impl DiscoveryEnv {
    pub fn from_process() -> Self {
        Self {
            path: std::env::var_os("PATH"),
            list: std::env::var_os(CONVERTER_LIST_ENV),
            singles: SINGLE_CONVERTER_ENVS
                .iter()
                .filter_map(|var| Some((var.to_string(), std::env::var_os(var)?)))
                .collect(),
        }
    }
}

/// A converter that answered `describe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredConverter {
    pub path: PathBuf,
    pub command: ConverterCommand,
    pub description: ConverterDescription,
}

/// What discovery found, and the candidates it set aside with why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    pub converters: Vec<DiscoveredConverter>,
    pub skipped: Vec<(PathBuf, ImportError)>,
}

impl Discovery {
    /// Every converter that reads `path`; several when more than one
    /// claims its extension, and the dialog chooses.
    pub fn converters_for(&self, path: &Path) -> Vec<&DiscoveredConverter> {
        self.converters
            .iter()
            .filter(|c| c.description.input_for(path).is_some())
            .collect()
    }

    /// The refusal to show when nothing was found.
    pub fn none_found_error() -> ImportError {
        ImportError::NoConverters {
            env: CONVERTER_LIST_ENV.to_string(),
        }
    }
}

/// `signex-<format>-import`, plus the platform's executable suffix.
pub fn is_converter_name(file_name: &str) -> bool {
    let stem = file_name
        .strip_suffix(std::env::consts::EXE_SUFFIX)
        .filter(|_| !std::env::consts::EXE_SUFFIX.is_empty())
        .unwrap_or(file_name);
    stem.strip_prefix(NAME_PREFIX)
        .and_then(|rest| rest.strip_suffix(NAME_SUFFIX))
        .is_some_and(|format| !format.is_empty())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// `binary` (plus the platform's executable suffix) in an absolute `PATH`
/// entry. An empty or relative entry would resolve against the working
/// directory, so it is skipped.
pub fn find_on_path(binary: &str, path: Option<OsString>) -> Option<PathBuf> {
    let name = format!("{binary}{}", std::env::consts::EXE_SUFFIX);
    absolute_entries(path.as_ref())
        .map(|dir| dir.join(&name))
        .find(|p| p.is_file())
}

fn absolute_entries(path: Option<&OsString>) -> impl Iterator<Item = PathBuf> + '_ {
    path.into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
}

/// Every candidate program, in discovery order, de-duplicated by
/// canonical path; a single-converter variable that names no file is
/// reported in the second list.
pub fn candidates(env: &DiscoveryEnv) -> (Vec<PathBuf>, Vec<(PathBuf, ImportError)>) {
    let mut found = Vec::new();
    let mut bad = Vec::new();
    for (var, value) in &env.singles {
        let path = PathBuf::from(value);
        if value.is_empty() {
            continue;
        }
        if path.is_file() {
            found.push(path);
        } else {
            bad.push((
                path.clone(),
                ImportError::OverrideNotAFile {
                    env: var.clone(),
                    path,
                },
            ));
        }
    }
    found.extend(absolute_entries(env.list.as_ref()).filter(|p| p.is_file()));
    for dir in absolute_entries(env.path.as_ref()) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut named: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_str().is_some_and(is_converter_name))
            .map(|e| e.path())
            .filter(|p| is_executable(p))
            .collect();
        named.sort();
        found.extend(named);
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|p| seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone())));
    (found, bad)
}

/// How many `describe` calls run at once.
pub const PARALLEL_DESCRIBES: usize = 4;

/// What a discovery depends on: the environment it read and, for every
/// candidate, its size and modification time. Equal fingerprints give the
/// same answer, so a session asks each converter once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryFingerprint {
    path: Option<OsString>,
    list: Option<OsString>,
    singles: Vec<(String, OsString)>,
    candidates: Vec<(PathBuf, Option<(u64, std::time::SystemTime)>)>,
}

impl DiscoveryFingerprint {
    fn of(env: &DiscoveryEnv, candidates: &[PathBuf]) -> Self {
        Self {
            path: env.path.clone(),
            list: env.list.clone(),
            singles: env.singles.clone(),
            candidates: candidates
                .iter()
                .map(|p| {
                    let stamp = std::fs::metadata(p)
                        .ok()
                        .and_then(|m| Some((m.len(), m.modified().ok()?)));
                    (p.clone(), stamp)
                })
                .collect(),
        }
    }
}

/// A discovery and what it was computed from; kept in the app's model for
/// the session and handed back to the next discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryCache {
    pub fingerprint: DiscoveryFingerprint,
    pub discovery: Discovery,
}

/// Run `describe` on every candidate, [`PARALLEL_DESCRIBES`] at a time, and
/// keep those that answer with a supported contract, in candidate order.
/// `command_for` builds how a candidate is run.
pub fn discover_with(
    env: &DiscoveryEnv,
    timeout: Duration,
    command_for: impl Fn(&Path) -> ConverterCommand + Sync,
) -> Discovery {
    discover_cached(None, env, timeout, command_for).discovery
}

/// [`discover_with`], answered from `cache` when nothing it depends on
/// changed.
pub fn discover_cached(
    cache: Option<&DiscoveryCache>,
    env: &DiscoveryEnv,
    timeout: Duration,
    command_for: impl Fn(&Path) -> ConverterCommand + Sync,
) -> DiscoveryCache {
    let (paths, mut skipped) = candidates(env);
    let fingerprint = DiscoveryFingerprint::of(env, &paths);
    if let Some(cache) = cache.filter(|c| c.fingerprint == fingerprint) {
        return cache.clone();
    }
    let answers = describe_all(&paths, timeout, &command_for);
    let mut converters = Vec::new();
    for (path, command, answer) in answers {
        match answer {
            Ok(description) => converters.push(DiscoveredConverter {
                path,
                command,
                description,
            }),
            Err(error) => {
                log::info!(
                    "library import: {} is not a library converter: {error}",
                    path.display()
                );
                skipped.push((path, error));
            }
        }
    }
    DiscoveryCache {
        fingerprint,
        discovery: Discovery {
            converters,
            skipped,
        },
    }
}

type Answer = (
    PathBuf,
    ConverterCommand,
    Result<ConverterDescription, ImportError>,
);

/// `describe` every path on [`PARALLEL_DESCRIBES`] scoped threads; the
/// answers come back in `paths` order.
fn describe_all(
    paths: &[PathBuf],
    timeout: Duration,
    command_for: &(impl Fn(&Path) -> ConverterCommand + Sync),
) -> Vec<Answer> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, PoisonError};
    let next = AtomicUsize::new(0);
    let answers: Mutex<Vec<Option<Answer>>> = Mutex::new(vec![None; paths.len()]);
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL_DESCRIBES.min(paths.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(i) else { break };
                    let command = command_for(path);
                    let label = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let answer = run_describe(&command, &label, timeout);
                    answers.lock().unwrap_or_else(PoisonError::into_inner)[i] =
                        Some((path.clone(), command, answer));
                }
            });
        }
    });
    answers
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
        .into_iter()
        .flatten()
        .collect()
}

/// [`discover_cached`] over this process's environment.
pub fn discover(cache: Option<&DiscoveryCache>) -> DiscoveryCache {
    discover_cached(
        cache,
        &DiscoveryEnv::from_process(),
        DISCOVERY_DESCRIBE_TIMEOUT,
        |path| ConverterCommand::new(path.to_path_buf()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_signex_format_import_names_count() {
        assert!(is_converter_name("signex-a-import"));
        assert!(is_converter_name("signex-altium-import"));
        assert!(!is_converter_name("signex--import"));
        assert!(!is_converter_name("signex-cli"));
        assert!(!is_converter_name("other-a-import"));
        assert!(!is_converter_name("signex-a-import.sh"));
    }

    #[test]
    fn a_bad_single_variable_is_named() {
        let env = DiscoveryEnv {
            singles: vec![("SNX_X".into(), "/no/such/converter".into())],
            ..Default::default()
        };
        let (found, bad) = candidates(&env);
        assert!(found.is_empty());
        assert!(matches!(&bad[0].1, ImportError::OverrideNotAFile { env, .. } if env == "SNX_X"));
    }

    #[test]
    fn find_on_path_uses_absolute_entries_only() {
        let dir = tempfile::tempdir().unwrap();
        let name = format!("conv{}", std::env::consts::EXE_SUFFIX);
        std::fs::write(dir.path().join(&name), "").unwrap();
        assert_eq!(
            find_on_path("conv", Some(dir.path().as_os_str().to_owned())),
            Some(dir.path().join(&name))
        );
        assert_eq!(find_on_path("conv", Some("relative/dir".into())), None);
    }
}
