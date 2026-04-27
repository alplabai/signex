//! User-config persistence for the Distributor APIs panel.
//!
//! UI-WS7: stores the `[distributor_apis] preferred_order = [...]`
//! list at `<config_dir>/signex/distributors.toml`.
//!
//! Why a dedicated file vs. piggy-backing on `prefs.json`:
//! - `prefs.json` is JSON; the rest of the v0.9 library config is TOML
//!   (matching `Manifest`).
//! - The file gets cross-tool-readable lifecycle defaults in v0.9.1
//!   (AVL, template defaults, etc.); keeping it TOML now lets the
//!   schema grow without a migration.
//!
//! Errors are intentionally swallowed at the boundary because:
//! - On startup, a missing/corrupt config must not block the app —
//!   we fall back to [`DistributorSettings::default()`].
//! - On save, an I/O failure is non-critical — the next save will
//!   just overwrite. A `tracing::warn!` surfaces the why.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use signex_library::DistributorSource;

/// File name for the distributors config — kept as a constant so the
/// install path tests can pin it.
const FILE_NAME: &str = "distributors.toml";

/// On-disk shape. `serde(default)` lets older / partial files load
/// cleanly when v0.9.1 lands new sub-tables.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DistributorsConfig {
    #[serde(default)]
    distributor_apis: DistributorApisSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DistributorApisSection {
    /// Persisted as wire strings so renaming the in-code enum doesn't
    /// silently strand existing configs. See [`source_to_str`] for the
    /// canonical mapping.
    #[serde(default = "default_order_strs")]
    preferred_order: Vec<String>,
}

impl Default for DistributorApisSection {
    fn default() -> Self {
        Self {
            preferred_order: default_order_strs(),
        }
    }
}

fn default_order_strs() -> Vec<String> {
    default_order()
        .into_iter()
        .map(source_to_str)
        .map(str::to_string)
        .collect()
}

/// Default order matches `DistributorSettings::default()`.
fn default_order() -> Vec<DistributorSource> {
    vec![
        DistributorSource::DigiKey,
        DistributorSource::Mouser,
        DistributorSource::Lcsc,
        DistributorSource::Jlcpcb,
    ]
}

/// Wire-name for a distributor source — kept in this module so the
/// schema is local to the file that owns it.
fn source_to_str(s: DistributorSource) -> &'static str {
    match s {
        DistributorSource::DigiKey => "digikey",
        DistributorSource::Mouser => "mouser",
        DistributorSource::Lcsc => "lcsc",
        DistributorSource::Jlcpcb => "jlcpcb",
        DistributorSource::Octopart => "octopart",
        DistributorSource::Oemsecrets => "oemsecrets",
        DistributorSource::Other => "other",
    }
}

fn str_to_source(s: &str) -> Option<DistributorSource> {
    match s.to_ascii_lowercase().as_str() {
        "digikey" => Some(DistributorSource::DigiKey),
        "mouser" => Some(DistributorSource::Mouser),
        "lcsc" => Some(DistributorSource::Lcsc),
        "jlcpcb" => Some(DistributorSource::Jlcpcb),
        "octopart" => Some(DistributorSource::Octopart),
        "oemsecrets" => Some(DistributorSource::Oemsecrets),
        "other" => Some(DistributorSource::Other),
        _ => None,
    }
}

/// Resolve `<config_dir>/signex/distributors.toml`. Returns `None`
/// when the platform refuses to hand us a config dir (rare; e.g. some
/// sandboxed CI runners). Tests override via [`config_path_for_dir`].
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("signex").join(FILE_NAME))
}

/// Test-friendly variant — same layout, but rooted under the supplied
/// directory. Lets unit tests round-trip without touching the real
/// per-user config dir.
#[allow(dead_code)]
pub fn config_path_for_dir(base: &std::path::Path) -> PathBuf {
    base.join("signex").join(FILE_NAME)
}

/// Load the persisted preferred-order list. Returns the LIBRARY_PLAN
/// default when the file is missing/empty/corrupt — startup is
/// best-effort.
pub fn load_preferred_order() -> Vec<DistributorSource> {
    config_path()
        .map(|p| load_preferred_order_at(&p))
        .unwrap_or_else(default_order)
}

/// Load preferred order from a specific path — extracted so tests can
/// hit the actual parse path without the `dirs::config_dir` dance.
pub fn load_preferred_order_at(path: &std::path::Path) -> Vec<DistributorSource> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return default_order();
    };
    let Ok(parsed) = toml::from_str::<DistributorsConfig>(&text) else {
        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "distributors.toml: parse failed; falling back to defaults"
        );
        return default_order();
    };
    let mut out: Vec<DistributorSource> = parsed
        .distributor_apis
        .preferred_order
        .iter()
        .filter_map(|s| str_to_source(s))
        .collect();
    // Defensive: if the file ends up empty (every name unknown) fall
    // back so the picker isn't useless. We also de-duplicate so a
    // hand-edit listing the same source twice doesn't wedge the
    // up/down buttons.
    if out.is_empty() {
        return default_order();
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert(*s));
    out
}

/// Persist the preferred-order list. Silently swallows I/O errors
/// after a `tracing::warn!` — losing this is non-fatal.
pub fn save_preferred_order(order: &[DistributorSource]) {
    let Some(path) = config_path() else {
        tracing::warn!(
            target: "signex::library",
            "distributors.toml: no config dir on this platform; skipping save"
        );
        return;
    };
    save_preferred_order_at(&path, order);
}

/// Variant for tests / explicit paths.
pub fn save_preferred_order_at(path: &std::path::Path, order: &[DistributorSource]) {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(
            target: "signex::library",
            path = %parent.display(),
            error = %e,
            "distributors.toml: create_dir_all failed"
        );
        return;
    }
    let cfg = DistributorsConfig {
        distributor_apis: DistributorApisSection {
            preferred_order: order
                .iter()
                .copied()
                .map(source_to_str)
                .map(str::to_string)
                .collect(),
        },
    };
    match toml::to_string_pretty(&cfg) {
        Ok(s) => {
            if let Err(e) = std::fs::write(path, s) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "distributors.toml: write failed"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "distributors.toml: serialize failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_writes_and_reads_back() {
        let tmp = tempfile::tempdir().unwrap();
        let path = config_path_for_dir(tmp.path());
        let original = vec![
            DistributorSource::Lcsc,
            DistributorSource::DigiKey,
            DistributorSource::Mouser,
        ];
        save_preferred_order_at(&path, &original);
        let read = load_preferred_order_at(&path);
        assert_eq!(read, original);
    }

    #[test]
    fn missing_file_returns_default_order() {
        let tmp = tempfile::tempdir().unwrap();
        let path = config_path_for_dir(tmp.path()).join("does-not-exist.toml");
        let read = load_preferred_order_at(&path);
        assert_eq!(read, default_order());
    }

    #[test]
    fn corrupt_file_returns_default_order() {
        let tmp = tempfile::tempdir().unwrap();
        let path = config_path_for_dir(tmp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"this is not toml [\n").unwrap();
        let read = load_preferred_order_at(&path);
        assert_eq!(read, default_order());
    }

    #[test]
    fn unknown_distributor_strings_filtered_out() {
        let tmp = tempfile::tempdir().unwrap();
        let path = config_path_for_dir(tmp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            br#"[distributor_apis]
preferred_order = ["mouser", "made_up_one", "lcsc"]
"#,
        )
        .unwrap();
        let read = load_preferred_order_at(&path);
        assert_eq!(
            read,
            vec![DistributorSource::Mouser, DistributorSource::Lcsc]
        );
    }

    #[test]
    fn empty_after_filter_falls_back_to_default() {
        let tmp = tempfile::tempdir().unwrap();
        let path = config_path_for_dir(tmp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            br#"[distributor_apis]
preferred_order = ["nothing", "matches"]
"#,
        )
        .unwrap();
        let read = load_preferred_order_at(&path);
        assert_eq!(read, default_order());
    }

    #[test]
    fn duplicates_are_collapsed() {
        let tmp = tempfile::tempdir().unwrap();
        let path = config_path_for_dir(tmp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            br#"[distributor_apis]
preferred_order = ["mouser", "digikey", "mouser"]
"#,
        )
        .unwrap();
        let read = load_preferred_order_at(&path);
        assert_eq!(
            read,
            vec![DistributorSource::Mouser, DistributorSource::DigiKey]
        );
    }

    #[test]
    fn source_str_round_trip_covers_every_variant() {
        let all = [
            DistributorSource::DigiKey,
            DistributorSource::Mouser,
            DistributorSource::Lcsc,
            DistributorSource::Jlcpcb,
            DistributorSource::Octopart,
            DistributorSource::Oemsecrets,
            DistributorSource::Other,
        ];
        for s in all {
            let wire = source_to_str(s);
            assert_eq!(str_to_source(wire), Some(s));
        }
    }
}
