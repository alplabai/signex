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

use crate::adapter::{LibraryAdapter, LibraryError, RowEdit, RowEditOutcome};
use crate::component::ComponentRow;
use crate::identity::RowId;
use crate::manifest::WorkflowMode;
use crate::primitive::PrimitiveRef;
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

/// One saved primitive to cascade: its kind, identity and new version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimitiveSave {
    pub kind: CascadeKind,
    /// The library the primitive lives in; with `uuid` it is the identity
    /// rows bind to.
    pub library_id: Uuid,
    pub uuid: Uuid,
    pub version: String,
}

impl PrimitiveSave {
    fn target(&self) -> PrimitiveRef {
        PrimitiveRef::new(self.library_id, self.uuid)
    }
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
        library_id: adapter.manifest().library.library_id,
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
/// Team keeps its pin and is reported as stale. Every row to bump goes to
/// the adapter in one [`LibraryAdapter::update_rows`] call: the local
/// library writes its `.snxlib` once and commits once for the whole
/// cascade; an adapter without a batch write updates the rows in order and
/// stops at the first failure. A row that fails to update is recorded in
/// [`CascadeReport::failed`], never returned as an error.
///
/// Rows are independent: each re-pin is idempotent and a row already at the
/// saved version is skipped, so one stale or vanished row never holds back
/// the clean ones. The clean rows commit together; the others come back in
/// `stale` or `failed` for the caller to report, and a later pass retries
/// them. A write that fails as a whole moves no row, leaves the file and git
/// as they were, and reports every row as failed. The commit message names
/// the first rows and counts the rest.
pub fn cascade_after_saves(
    adapter: &dyn LibraryAdapter,
    saves: &[PrimitiveSave],
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    cascade_with(adapter, saves, mode, &mut |targets, edit, msg| {
        adapter.update_rows(targets, edit, msg)
    })
}

/// The batch write of a cascade: the rows to bump, the edit that bumps
/// one, the commit message; one outcome per row.
type WriteBatch<'a> = dyn FnMut(&[(String, RowId)], &mut RowEdit<'_>, &str) -> Result<Vec<RowEditOutcome>, LibraryError>
    + 'a;

fn cascade_with(
    adapter: &dyn LibraryAdapter,
    saves: &[PrimitiveSave],
    mode: WorkflowMode,
    write: &mut WriteBatch<'_>,
) -> Result<CascadeReport, LibraryError> {
    let mut report = CascadeReport::default();
    if saves.is_empty() {
        return Ok(report);
    }
    let mut targets: Vec<(String, RowId)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for (table, row) in adapter.iter_rows()? {
        let row_id = RowId::from_uuid(row.row_id);
        let mut bump = false;
        for save in saves {
            if !needs_bump(&row, save) {
                continue;
            }
            if !matches!(mode, WorkflowMode::Personal) && row.released {
                report.stale.push(row_id);
                continue;
            }
            bump = true;
        }
        if bump {
            names.push(row_label(&row));
            targets.push((table, row_id));
        }
    }
    if targets.is_empty() {
        return Ok(report);
    }
    // The rows are re-read at write time, so each is checked again against
    // the saves: a row another writer already moved is left alone.
    let mut edit = |row: &mut ComponentRow| {
        let mut changed = false;
        for save in saves {
            if needs_bump(row, save) {
                apply_cascade_bump(row, save);
                changed = true;
            }
        }
        changed
    };
    let msg = cascade_commit_message(saves, &names);
    match write(&targets, &mut edit, &msg) {
        Ok(outcomes) => {
            for (row_id, outcome) in outcomes {
                match outcome {
                    Ok(true) => report.auto_bumped.push(row_id),
                    Ok(false) => {}
                    Err(e) => report.failed.push((row_id, e.to_string())),
                }
            }
        }
        Err(e) => {
            let reason = e.to_string();
            report
                .failed
                .extend(targets.iter().map(|(_, id)| (*id, reason.clone())));
        }
    }
    Ok(report)
}

/// True when the row binds the saved primitive (as default or alternate
/// symbol, footprint or sim).
pub(crate) fn row_binds_to(row: &ComponentRow, save: &PrimitiveSave) -> bool {
    let target = save.target();
    match save.kind {
        CascadeKind::Symbol => {
            row.symbol_ref == target || row.alternate_symbols.iter().any(|a| a.symbol_ref == target)
        }
        CascadeKind::Footprint => row.footprint_ref == Some(target),
        CascadeKind::Sim => row.sim_ref == Some(target),
    }
}

/// True when every pin of the row that binds the saved primitive is
/// already at the saved version. Only meaningful for a bound row.
fn is_pinned_at(row: &ComponentRow, save: &PrimitiveSave) -> bool {
    let target = save.target();
    match save.kind {
        CascadeKind::Symbol => {
            let default_ok = row.symbol_ref != target || row.symbol_version == save.version;
            default_ok
                && row
                    .alternate_symbols
                    .iter()
                    .filter(|a| a.symbol_ref == target)
                    .all(|a| a.symbol_version == save.version)
        }
        CascadeKind::Footprint => row.footprint_version == save.version,
        CascadeKind::Sim => row.sim_version == save.version,
    }
}

/// True when the row binds the saved primitive and at least one of its
/// pins is not yet at the saved version.
pub(crate) fn needs_bump(row: &ComponentRow, save: &PrimitiveSave) -> bool {
    row_binds_to(row, save) && !is_pinned_at(row, save)
}

/// Re-pin the default symbol and every alternate that binds `target`.
fn bump_symbol_pins(row: &mut ComponentRow, target: PrimitiveRef, version: &str) {
    if row.symbol_ref == target {
        row.symbol_version = version.to_string();
    }
    for alt in row
        .alternate_symbols
        .iter_mut()
        .filter(|a| a.symbol_ref == target)
    {
        alt.symbol_version = version.to_string();
    }
}

pub(crate) fn apply_cascade_bump(row: &mut ComponentRow, save: &PrimitiveSave) {
    match save.kind {
        CascadeKind::Symbol => bump_symbol_pins(row, save.target(), &save.version),
        CascadeKind::Footprint => row.footprint_version = save.version.clone(),
        CascadeKind::Sim => row.sim_version = save.version.clone(),
    }
    row.version = patch_bump(&row.version);
    row.updated = chrono::Utc::now();
}

/// Rows named in a cascade commit message before "+N more".
const MESSAGE_ROWS: usize = 5;
/// Longest part number shown in a commit message, in characters.
const MESSAGE_NAME_CHARS: usize = 32;

/// What a commit message calls a row: its part number, or its short id when
/// the part number is blank.
fn row_label(row: &ComponentRow) -> String {
    let pn = row.internal_pn.to_string();
    let pn = pn.trim();
    if pn.is_empty() {
        return short_uuid(row.row_id);
    }
    let mut label: String = pn.chars().take(MESSAGE_NAME_CHARS).collect();
    if pn.chars().count() > MESSAGE_NAME_CHARS {
        label.push('~');
    }
    label.replace(['\n', '\r'], " ")
}

/// Synthesise the one git commit message of a cascade: it names each saved
/// primitive, how many rows the commit moves and the first few of them, so
/// the audit trail groups under the primitive UUID. The list is cut with
/// "+N more" so the subject stays one short line.
fn cascade_commit_message(saves: &[PrimitiveSave], rows: &[String]) -> String {
    let what: Vec<String> = saves
        .iter()
        .map(|s| {
            format!(
                "{} {} to v{}",
                s.kind.label(),
                short_uuid(s.uuid),
                s.version
            )
        })
        .collect();
    let mut message = format!("cascade: bump {} on {} row(s)", what.join(", "), rows.len());
    if rows.is_empty() {
        return message;
    }
    let shown = rows.len().min(MESSAGE_ROWS);
    message.push_str(": ");
    message.push_str(&rows[..shown].join(", "));
    if rows.len() > shown {
        message.push_str(&format!(", +{} more", rows.len() - shown));
    }
    message
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

    fn save() -> PrimitiveSave {
        PrimitiveSave {
            kind: CascadeKind::Footprint,
            library_id: Uuid::nil(),
            uuid: Uuid::nil(),
            version: "1.0.1".into(),
        }
    }

    #[test]
    fn the_commit_message_lists_few_rows_whole_and_cuts_many() {
        let names: Vec<String> = (0..3).map(|i| format!("R-{i}")).collect();
        assert_eq!(
            cascade_commit_message(&[save()], &names),
            "cascade: bump footprint 00000000 to v1.0.1 on 3 row(s): R-0, R-1, R-2"
        );
        let many: Vec<String> = (0..9).map(|i| format!("R-{i}")).collect();
        let msg = cascade_commit_message(&[save()], &many);
        assert!(msg.ends_with("R-0, R-1, R-2, R-3, R-4, +4 more"), "{msg}");
        assert!(msg.contains("on 9 row(s)"));
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
                alternate_symbols: Vec::new(),
                row_id: Uuid::now_v7(),
                internal_pn: InternalPn::new(pn),
                class: ComponentClass::new("resistor"),
                datasheet: DatasheetRef::default(),
                datasheet_label: String::new(),
                description: String::new(),
                more_datasheets: Vec::new(),
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
                server_revision: None,
            }
        }

        #[test]
        fn a_long_or_blank_part_number_stays_short_in_the_message() {
            let mut row = row(&"X".repeat(100), Uuid::nil(), Uuid::nil());
            assert_eq!(row_label(&row).chars().count(), MESSAGE_NAME_CHARS + 1);
            row.internal_pn = crate::identity::InternalPn::new("  ");
            assert_eq!(row_label(&row), short_uuid(row.row_id));
        }

        fn save(lib: Uuid, symbol: Uuid) -> PrimitiveSave {
            PrimitiveSave {
                kind: CascadeKind::Symbol,
                library_id: lib,
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
                &[save(lib, symbol)],
                WorkflowMode::Personal,
                &mut |targets, edit, msg| {
                    // The adapter writes every row but `bad`, which fails.
                    let (bad_target, rest): (Vec<_>, Vec<_>) =
                        targets.iter().cloned().partition(|(_, id)| *id == bad);
                    let mut outcomes = adapter.update_rows(&rest, edit, msg)?;
                    outcomes.extend(
                        bad_target
                            .iter()
                            .map(|(_, id)| (*id, Err(LibraryError::Backend("disk full".into())))),
                    );
                    Ok(outcomes)
                },
            )
            .unwrap();
            assert_eq!(first.auto_bumped.len(), 2);
            assert_eq!(first.failed, vec![(bad, "backend: disk full".to_string())]);
            assert_eq!(read(&adapter, bad).symbol_version, "0.0.1");

            let second =
                cascade_after_saves(&adapter, &[save(lib, symbol)], WorkflowMode::Personal)
                    .unwrap();
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
                cascade_after_saves(&adapter, &[save(lib, symbol)], WorkflowMode::Team).unwrap();
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
                &[save(lib, symbol)],
                WorkflowMode::Personal,
                &mut |targets, edit, msg| {
                    for (table, id) in targets {
                        let mut edited = other.read_row(table, *id).unwrap();
                        edited.primary_mpn = ManufacturerPart::draft("Acme", "EDITED-MPN");
                        other.update_row(table, edited, "edit").unwrap();
                    }
                    adapter.update_rows(targets, edit, msg)
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

        #[test]
        fn a_symbol_bump_re_pins_alternates_and_leaves_the_default_alone() {
            let dir = tempfile::tempdir().unwrap();
            let (file, adapter, lib) = library(dir.path());
            let (default_sym, alt_sym) = (Uuid::now_v7(), Uuid::now_v7());
            let mut r = row("R-1", lib, default_sym);
            r.alternate_symbols
                .push(crate::alternate_symbol::AlternateSymbol::new(
                    PrimitiveRef::new(lib, alt_sym),
                    "0.0.1",
                    "IEC",
                ));
            adapter.insert_row("R", r.clone(), "seed").unwrap();

            // The alternate survives the .snxlib round trip.
            let reopened = LocalGitAdapter::open(&file).unwrap();
            assert_eq!(
                read(&reopened, RowId::from_uuid(r.row_id))
                    .alternate_symbols
                    .len(),
                1
            );

            let report =
                cascade_after_symbol_save(&adapter, alt_sym, "0.0.2", WorkflowMode::Personal)
                    .unwrap();
            assert_eq!(report.auto_bumped.len(), 1);
            let after = read(&adapter, RowId::from_uuid(r.row_id));
            assert_eq!(after.alternate_symbols[0].symbol_version, "0.0.2");
            assert_eq!(after.symbol_version, "0.0.1");

            // Already pinned: a repeat pass changes nothing.
            let again =
                cascade_after_symbol_save(&adapter, alt_sym, "0.0.2", WorkflowMode::Personal)
                    .unwrap();
            assert!(again.is_empty());
        }
    }
}
