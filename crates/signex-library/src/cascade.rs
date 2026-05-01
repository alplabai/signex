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

/// Outcome of one cascade pass — which rows were silently auto-bumped
/// and which were left alone with a stale `<kind>_version` pin.
///
/// The caller (typically `LocalGitAdapter::save_*`) inspects this
/// after a save to decide whether to surface a UI notice. Stage 15
/// callers ignore the stale list and let the Library Browser's
/// existing indicator pick it up; Stage 16 will pump the stale list
/// into the schematic-side Library Updates dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CascadeReport {
    /// Rows whose pinned version was advanced + own version
    /// patch-bumped. Ordered as encountered during `iter_rows`.
    pub auto_bumped: Vec<RowId>,
    /// Rows that bind to the saved primitive but stayed at their
    /// old `<kind>_version` pin. Surfaces in the Library Browser as
    /// a stale-binding indicator.
    pub stale: Vec<RowId>,
}

impl CascadeReport {
    pub fn is_empty(&self) -> bool {
        self.auto_bumped.is_empty() && self.stale.is_empty()
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
    if parts.len() == 3 {
        if let (Ok(major), Ok(minor), Ok(patch)) = (
            parts[0].parse::<u64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
        ) {
            return format!("{major}.{minor}.{}", patch + 1);
        }
    }
    format!("{version}.1")
}

/// Run the cascade after a `save_symbol(sym, _)` succeeded.
///
/// Walks every row across every table; rows whose `symbol_ref.uuid`
/// matches `sym_uuid` are bucketed and updated per [`WorkflowMode`].
/// Auto-bumped rows are written back via
/// [`LibraryAdapter::update_row`] with a synthesised commit message;
/// the message text is documented on
/// [`cascade_commit_message`].
pub fn cascade_after_symbol_save(
    adapter: &dyn LibraryAdapter,
    sym_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    cascade_after_save(
        adapter,
        sym_uuid,
        new_version,
        mode,
        PrimitiveKindTag::Symbol,
    )
}

/// Run the cascade after a `save_footprint(fp, _)` succeeded.
///
/// Mirrors [`cascade_after_symbol_save`] but matches against the row's
/// `footprint_ref` (an `Option<PrimitiveRef>` — rows without a
/// footprint binding are skipped silently).
pub fn cascade_after_footprint_save(
    adapter: &dyn LibraryAdapter,
    fp_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    cascade_after_save(
        adapter,
        fp_uuid,
        new_version,
        mode,
        PrimitiveKindTag::Footprint,
    )
}

/// Run the cascade after a `save_sim(sm, _)` succeeded.
///
/// Mirrors [`cascade_after_symbol_save`] but matches against the row's
/// `sim_ref` (an `Option<PrimitiveRef>` — rows without a sim binding
/// are skipped silently).
pub fn cascade_after_sim_save(
    adapter: &dyn LibraryAdapter,
    sim_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
) -> Result<CascadeReport, LibraryError> {
    cascade_after_save(adapter, sim_uuid, new_version, mode, PrimitiveKindTag::Sim)
}

/// Internal dispatcher — picks the right binding column and shared
/// the row scan + bucket logic across the three primitive kinds.
#[derive(Clone, Copy)]
enum PrimitiveKindTag {
    Symbol,
    Footprint,
    Sim,
}

impl PrimitiveKindTag {
    fn label(self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Footprint => "footprint",
            Self::Sim => "sim",
        }
    }
}

fn cascade_after_save(
    adapter: &dyn LibraryAdapter,
    primitive_uuid: Uuid,
    new_version: &str,
    mode: WorkflowMode,
    kind: PrimitiveKindTag,
) -> Result<CascadeReport, LibraryError> {
    let mut report = CascadeReport::default();
    for (table, row) in adapter.iter_rows()? {
        if !row_binds_to(&row, primitive_uuid, kind) {
            continue;
        }
        // Bucket: under Personal mode (or Team-mode-with-unreleased-row)
        // we silently auto-cascade. Released rows under Team mode keep
        // their old pin and surface as stale.
        if matches!(mode, WorkflowMode::Personal) || !row.released {
            let mut updated = row.clone();
            apply_cascade_bump(&mut updated, new_version, kind);
            let row_id = RowId::from_uuid(updated.row_id);
            let msg = cascade_commit_message(kind, primitive_uuid, new_version, &row_id);
            adapter.update_row(&table, updated, &msg)?;
            report.auto_bumped.push(row_id);
        } else {
            report.stale.push(RowId::from_uuid(row.row_id));
        }
    }
    Ok(report)
}

fn row_binds_to(row: &ComponentRow, primitive_uuid: Uuid, kind: PrimitiveKindTag) -> bool {
    match kind {
        PrimitiveKindTag::Symbol => row.symbol_ref.uuid == primitive_uuid,
        PrimitiveKindTag::Footprint => row
            .footprint_ref
            .as_ref()
            .is_some_and(|r| r.uuid == primitive_uuid),
        PrimitiveKindTag::Sim => row
            .sim_ref
            .as_ref()
            .is_some_and(|r| r.uuid == primitive_uuid),
    }
}

fn apply_cascade_bump(row: &mut ComponentRow, new_version: &str, kind: PrimitiveKindTag) {
    match kind {
        PrimitiveKindTag::Symbol => row.symbol_version = new_version.to_string(),
        PrimitiveKindTag::Footprint => row.footprint_version = new_version.to_string(),
        PrimitiveKindTag::Sim => row.sim_version = new_version.to_string(),
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
    kind: PrimitiveKindTag,
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
}
