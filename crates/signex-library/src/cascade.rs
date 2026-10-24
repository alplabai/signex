//! Primitive-save cascade engine — Stage 15 of `v0.9-snxlib-as-file-plan.md`.
//!
//! When a user saves a Symbol / Footprint / Sim primitive, every
//! [`ComponentRow`](crate::component::ComponentRow) whose
//! `<kind>_ref.uuid` matches the primitive UUID has its pinned
//! `<kind>_version` cell drift away from the new primitive version.
//! The cascade engine scans those rows and, depending on the library's
//! [`WorkflowMode`], either auto-bumps them in place or leaves them
//! flagged as stale bindings.
//!
//! ## Buckets (per plan §3.5.cascade)
//!
//! - **Auto-cascade-eligible**: a row whose `released == false`. The
//!   row's pinned `<kind>_version` advances to the new primitive
//!   version *and* the row's own `version` patch-bumps so the
//!   schematic-side Library Updates dialog (Stage 16) picks the change
//!   up. Personal mode treats *every* row as eligible — released flags
//!   are hidden in that mode.
//! - **Needs-review (stale)**: a row whose `released == true` under
//!   `Team` mode. The row's `<kind>_version` cell stays at the old
//!   value; the Library Browser already renders a "stale" indicator
//!   when the binding's pinned version drifts from the bound
//!   primitive's current version.
//!
//! Stage 15 ships the *data-side* logic only — the Team-mode cascade
//! modal lives in plan §3.5.cascade and arrives in a v0.9.x polish
//! pass. The engine is synchronous because the
//! [`LibraryAdapter`](crate::adapter::LibraryAdapter) trait is
//! synchronous; making it async would force the whole trait surface
//! to gain `async fn`. Async cascade is a v1.x concern.

use crate::adapter::{LibraryAdapter, LibraryError};
use crate::component::ComponentRow;
use crate::identity::RowId;
use crate::manifest::WorkflowMode;
use uuid::Uuid;

/// Outcome of one cascade pass: which rows were auto-bumped, which were
/// left alone with a stale `<kind>_version` pin, and which could not be
/// written.
///
/// The caller inspects this after a save to decide what to tell the
/// user. A row already at the saved version is in none of the lists, so a
/// second pass over the same save reports nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CascadeReport {
    /// Rows whose pinned version was advanced + own version
    /// patch-bumped. Ordered as encountered during `iter_rows`.
    pub auto_bumped: Vec<RowId>,
    /// Rows that bind to the saved primitive but stayed at their
    /// old `<kind>_version` pin. Surfaces in the Library Browser as
    /// a stale-binding indicator.
    pub stale: Vec<RowId>,
    /// Rows whose update failed, with the reason. The pass goes on past a
    /// failed row, so one bad row does not strand the rest; a later pass
    /// over the same save retries only these.
    pub failed: Vec<(RowId, String)>,
}

impl CascadeReport {
    pub fn is_empty(&self) -> bool {
        self.auto_bumped.is_empty() && self.stale.is_empty() && self.failed.is_empty()
    }
}

/// Which row binding a primitive save moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeKind {
    Symbol,
    Footprint,
    Sim,
}

impl CascadeKind {
    fn label(self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Footprint => "footprint",
            Self::Sim => "sim",
        }
    }
}

/// One saved primitive to cascade: its kind, uuid and new version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimitiveSave {
    pub kind: CascadeKind,
    pub uuid: Uuid,
    pub version: String,
}

/// Patch-bump a semver-style `X.Y.Z` string. Returns `<old>.1`
/// when the input doesn't parse (e.g. legacy rows with
/// `version = "draft"` or empty strings) so the cascade always
/// produces *some* observable change on the row — the Library
/// Updates dialog needs that to detect drift.
///
/// Examples:
///
/// - `"1.2.3"` → `"1.2.4"`
/// - `"0.0.1"` → `"0.0.2"`
/// - `""` → `".1"`
/// - `"draft"` → `"draft.1"`
pub fn patch_bump(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3
        && let (Ok(major), Ok(minor), Ok(patch)) = (
            parts[0].parse::<u64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
        )
    {
        return format!("{major}.{minor}.{}", patch + 1);
    }
    format!("{version}.1")
}

/// Run the cascade after a `save_symbol(sym, _)` succeeded.
///
/// Rows whose `symbol_ref.uuid` matches `sym_uuid` are bucketed and
/// updated per [`WorkflowMode`]; see [`cascade_after_saves`].
pub fn cascade_after_symbol_save(
    adapter: &dyn LibraryAdapter,
    sym_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    single_save(adapter, CascadeKind::Symbol, sym_uuid, new_version, mode)
}

/// Run the cascade after a `save_footprint(fp, _)` succeeded. Rows without
/// a footprint binding are skipped silently.
pub fn cascade_after_footprint_save(
    adapter: &dyn LibraryAdapter,
    fp_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    single_save(adapter, CascadeKind::Footprint, fp_uuid, new_version, mode)
}

/// Run the cascade after a `save_sim(sm, _)` succeeded. Rows without a sim
/// binding are skipped silently.
pub fn cascade_after_sim_save(
    adapter: &dyn LibraryAdapter,
    sim_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    single_save(adapter, CascadeKind::Sim, sim_uuid, new_version, mode)
}

fn single_save(
    adapter: &dyn LibraryAdapter,
    kind: CascadeKind,
    uuid: Uuid,
    version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    let save = PrimitiveSave {
        kind,
        uuid,
        version: version.to_string(),
    };
    cascade_after_saves(adapter, std::slice::from_ref(&save), mode)
}

/// Cascade a batch of primitive saves with one scan of the library.
///
/// A row bound to a saved primitive that is already pinned at the saved
/// version is skipped, so repeating a cascade never double-bumps a row.
/// Under [`WorkflowMode::Personal`], or for an unreleased row, the pin
/// advances and the row's own version patch-bumps; a released row under
/// Team keeps its pin and is reported as stale. A row that fails to update
/// is recorded in [`CascadeReport::failed`] and the pass goes on. Each
/// row is its own locked write and git commit
/// ([`LibraryAdapter::bind_row_primitive_version`]).
pub fn cascade_after_saves(
    adapter: &dyn LibraryAdapter,
    saves: &[PrimitiveSave],
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    cascade_with(adapter, saves, mode, &mut |table, row_id, save, msg| {
        adapter.bind_row_primitive_version(table, row_id, save.kind, &save.version, msg)
    })
}

type BumpRow<'a> = dyn FnMut(&str, RowId, &PrimitiveSave, &str) -> Result<bool, LibraryError> + 'a;

fn cascade_with(
    adapter: &dyn LibraryAdapter,
    saves: &[PrimitiveSave],
    mode: WorkflowMode,
    bump: &mut BumpRow<'_>,
) -> Result<CascadeReport, LibraryError> {
    let mut report = CascadeReport::default();
    if saves.is_empty() {
        return Ok(report);
    }
    for (table, row) in adapter.iter_rows()? {
        let row_id = RowId::from_uuid(row.row_id);
        for save in saves {
            if !row_binds_to(&row, save.uuid, save.kind)
                || pinned_version(&row, save.kind) == save.version
            {
                continue;
            }
            if !matches!(mode, WorkflowMode::Personal) && row.released {
                report.stale.push(row_id);
                continue;
            }
            let msg = cascade_commit_message(save.kind, save.uuid, &save.version, &row_id);
            match bump(&table, row_id, save, &msg) {
                Ok(_) => report.auto_bumped.push(row_id),
                Err(e) => report.failed.push((row_id, e.to_string())),
            }
        }
    }
    Ok(report)
}

pub(crate) fn pinned_version(row: &ComponentRow, kind: CascadeKind) -> &str {
    match kind {
        CascadeKind::Symbol => &row.symbol_version,
        CascadeKind::Footprint => &row.footprint_version,
        CascadeKind::Sim => &row.sim_version,
    }
}

fn row_binds_to(row: &ComponentRow, primitive_uuid: Uuid, kind: CascadeKind) -> bool {
    match kind {
        CascadeKind::Symbol => row.symbol_ref.uuid == primitive_uuid,
        CascadeKind::Footprint => row
            .footprint_ref
            .as_ref()
            .is_some_and(|r| r.uuid == primitive_uuid),
        CascadeKind::Sim => row
            .sim_ref
            .as_ref()
            .is_some_and(|r| r.uuid == primitive_uuid),
    }
}

pub(crate) fn apply_cascade_bump(row: &mut ComponentRow, new_version: &str, kind: CascadeKind) {
    match kind {
        CascadeKind::Symbol => row.symbol_version = new_version.to_string(),
        CascadeKind::Footprint => row.footprint_version = new_version.to_string(),
        CascadeKind::Sim => row.sim_version = new_version.to_string(),
    }
    row.version = patch_bump(&row.version);
    row.updated = chrono::Utc::now();
}

/// Synthesise the git commit message for a cascade-driven row update.
///
/// The plan §3.5.cascade calls for a single combined commit covering
/// the primitive save + every cascaded row update; for v0.9 simplicity
/// we emit one commit per row with a clearly-marked subject so the
/// audit trail still groups visually under the same primitive UUID.
/// A future polish pass can fold these into the parent save's commit.
fn cascade_commit_message(
    kind: CascadeKind,
    primitive_uuid: Uuid,
    new_version: &str,
    row_id: &RowId,
) -> String {
    format!(
        "cascade: bump {kind_label} {short_uuid} to v{new_version} on row {row_id}",
        kind_label = kind.label(),
        short_uuid = short_uuid(primitive_uuid),
    )
}

fn short_uuid(uuid: Uuid) -> String {
    let s = uuid.to_string();
    s.split('-').next().unwrap_or(&s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_bump_increments_clean_semver() {
        assert_eq!(patch_bump("1.2.3"), "1.2.4");
        assert_eq!(patch_bump("0.0.1"), "0.0.2");
        assert_eq!(patch_bump("10.20.99"), "10.20.100");
    }

    #[test]
    fn patch_bump_falls_back_on_garbage() {
        // Empty + non-semver inputs both fall through to the
        // append-`.1` branch so cascade always produces an
        // observable version delta.
        assert_eq!(patch_bump(""), ".1");
        assert_eq!(patch_bump("draft"), "draft.1");
        assert_eq!(patch_bump("1.2"), "1.2.1");
        assert_eq!(patch_bump("1.2.3.4"), "1.2.3.4.1");
    }

    #[test]
    fn cascade_report_default_is_empty() {
        let r = CascadeReport::default();
        assert!(r.is_empty());
        assert!(r.auto_bumped.is_empty());
        assert!(r.stale.is_empty());
    }

    #[cfg(feature = "local-git")]
    mod with_library {
        use super::*;
        use crate::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
        use crate::component::{DatasheetRef, PlmReserved};
        use crate::identity::{ComponentClass, InternalPn};
        use crate::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
        use crate::lifecycle::LifecycleState;
        use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
        use crate::manufacturer::ManufacturerPart;
        use crate::param::ParamMap;
        use crate::primitive::PrimitiveRef;

        fn library(dir: &std::path::Path) -> (std::path::PathBuf, LocalGitAdapter, Uuid) {
            crate::git_identity::test_identity::install();
            let file = dir.join("Lib").join("Lib.snxlib");
            let library_id = Uuid::now_v7();
            let manifest = SnxlibManifest {
                format: FORMAT_TOKEN.into(),
                library_id,
                library: LibrarySection {
                    name: "Lib".into(),
                    description: None,
                },
                mode: LibraryMode::default(),
                workflow: WorkflowConfig::default(),
                users: UsersConfig::default(),
                classes: Vec::new(),
            };
            let adapter =
                LocalGitAdapter::init(&file, manifest, LibraryInitOptions::default()).unwrap();
            (file, adapter, library_id)
        }

        fn row(pn: &str, library_id: Uuid, symbol: Uuid) -> ComponentRow {
            let now = chrono::Utc::now();
            ComponentRow {
                row_id: Uuid::now_v7(),
                internal_pn: InternalPn::new(pn),
                class: ComponentClass::new("resistor"),
                datasheet: DatasheetRef::default(),
                state: LifecycleState::Draft,
                symbol_ref: PrimitiveRef::new(library_id, symbol),
                footprint_ref: None,
                sim_ref: None,
                pin_map_overrides: Vec::new(),
                primary_mpn: ManufacturerPart::draft("Acme", pn),
                alternates: Vec::new(),
                supply: Vec::new(),
                parameters: ParamMap::new(),
                plm: PlmReserved::default(),
                version: "0.0.1".into(),
                released: false,
                symbol_version: "0.0.1".into(),
                footprint_version: String::new(),
                sim_version: String::new(),
                created: now,
                updated: now,
                content_hash: [0u8; 32],
            }
        }

        fn save(symbol: Uuid) -> PrimitiveSave {
            PrimitiveSave {
                kind: CascadeKind::Symbol,
                uuid: symbol,
                version: "0.0.2".into(),
            }
        }

        fn read(adapter: &LocalGitAdapter, id: RowId) -> ComponentRow {
            adapter.read_row("R", id).unwrap()
        }

        #[test]
        fn a_failed_row_does_not_stop_the_others_and_a_second_pass_retries_only_it() {
            let dir = tempfile::tempdir().unwrap();
            let (_file, adapter, lib) = library(dir.path());
            let symbol = Uuid::now_v7();
            let rows: Vec<ComponentRow> = (0..3)
                .map(|i| row(&format!("R-{i}"), lib, symbol))
                .collect();
            for r in &rows {
                adapter.insert_row("R", r.clone(), "seed").unwrap();
            }
            let bad = RowId::from_uuid(rows[1].row_id);

            let first = cascade_with(
                &adapter,
                &[save(symbol)],
                WorkflowMode::Personal,
                &mut |table, id, save, msg| {
                    if id == bad {
                        return Err(LibraryError::Backend("disk full".into()));
                    }
                    adapter.bind_row_primitive_version(table, id, save.kind, &save.version, msg)
                },
            )
            .unwrap();
            assert_eq!(first.auto_bumped.len(), 2);
            assert_eq!(first.failed, vec![(bad, "backend: disk full".to_string())]);
            assert_eq!(read(&adapter, bad).symbol_version, "0.0.1");

            let second =
                cascade_after_saves(&adapter, &[save(symbol)], WorkflowMode::Personal).unwrap();
            assert_eq!(
                second.auto_bumped,
                vec![bad],
                "only the failed row is retried"
            );
            assert!(second.failed.is_empty());
            for r in &rows {
                let on_disk = read(&adapter, RowId::from_uuid(r.row_id));
                assert_eq!(on_disk.symbol_version, "0.0.2");
                assert_eq!(on_disk.version, "0.0.2", "no row is bumped twice");
            }
        }

        #[test]
        fn a_released_row_under_team_is_reported_stale_and_left_alone() {
            let dir = tempfile::tempdir().unwrap();
            let (_file, adapter, lib) = library(dir.path());
            let symbol = Uuid::now_v7();
            let mut released = row("R-REL", lib, symbol);
            released.released = true;
            let id = RowId::from_uuid(released.row_id);
            adapter.insert_row("R", released, "seed").unwrap();

            let report =
                cascade_after_saves(&adapter, &[save(symbol)], WorkflowMode::Team).unwrap();
            assert_eq!(report.stale, vec![id]);
            assert!(report.auto_bumped.is_empty());
            assert_eq!(read(&adapter, id).symbol_version, "0.0.1");
        }

        #[test]
        fn the_bump_keeps_a_field_another_writer_changed_since_the_scan() {
            let dir = tempfile::tempdir().unwrap();
            let (file, adapter, lib) = library(dir.path());
            let symbol = Uuid::now_v7();
            adapter
                .insert_row("R", row("R-1", lib, symbol), "seed")
                .unwrap();
            let other = LocalGitAdapter::open(&file).unwrap();

            // Another process edits the row after the cascade's scan and
            // before its write.
            let report = cascade_with(
                &adapter,
                &[save(symbol)],
                WorkflowMode::Personal,
                &mut |table, id, save, msg| {
                    let mut edited = other.read_row(table, id).unwrap();
                    edited.primary_mpn = ManufacturerPart::draft("Acme", "EDITED-MPN");
                    other.update_row(table, edited, "edit").unwrap();
                    adapter.bind_row_primitive_version(table, id, save.kind, &save.version, msg)
                },
            )
            .unwrap();

            assert_eq!(report.auto_bumped.len(), 1);
            let after = read(&adapter, report.auto_bumped[0]);
            assert_eq!(after.symbol_version, "0.0.2");
            assert_eq!(
                after.primary_mpn.mpn, "EDITED-MPN",
                "the concurrent edit survives"
            );
        }
    }
}
