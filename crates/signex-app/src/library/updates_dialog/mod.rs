//! "Library Updates Available" modal: the placed parts whose library row
//! has a newer version, for the user to accept or skip in Team workflow
//! mode (Personal mode applies them silently). The card is in [`view`].
//! See docs/architecture/library-dialogs.md, "Library Updates dialog".

use std::path::PathBuf;

use signex_library::RowId;
use signex_types::schematic::StampField;
use uuid::Uuid;

use super::part_sync::{Choice, Choices, ConflictView};

mod view;
pub use view::view;

#[cfg(test)]
mod tests;

/// Severity classification for a drift entry — drives the default
/// checkbox state and the Change pill of its row. Pure-data
/// (no semver crate dependency); see [`classify_bump`] for the
/// inference rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    /// Patch bump (`1.0.0 → 1.0.1`) — same major + minor; treated as
    /// an opt-in default-on auto-update.
    Patch,
    /// Minor bump (`1.0.0 → 1.1.0`) — same major; treated as
    /// review-before-accepting (default off).
    Minor,
    /// Major bump (`1.0.0 → 2.0.0`) or any version pair where parsing
    /// the components fails — treated as likely-breaking. Default off
    /// + warning glyph.
    Major,
}

impl BumpKind {
    /// Every kind, most severe first: the order of the summary pills.
    pub const ALL: [Self; 3] = [Self::Major, Self::Minor, Self::Patch];

    /// Default checkbox state for the "Update Selected Components"
    /// flow — patch-only is the safe default.
    pub fn default_checked(self) -> bool {
        matches!(self, BumpKind::Patch)
    }
}

/// Classify a `(current, latest)` semver-style version pair into a
/// [`BumpKind`]. Both inputs are opaque-string-treated semver: the
/// rule splits on `.` and compares the leading two numeric segments.
/// Any parse failure or unequal-major pair upgrades to [`BumpKind::Major`].
pub fn classify_bump(current: &str, latest: &str) -> BumpKind {
    fn parts(s: &str) -> (Option<u32>, Option<u32>) {
        let mut iter = s.split('.');
        let major = iter.next().and_then(|s| s.parse::<u32>().ok());
        let minor = iter.next().and_then(|s| s.parse::<u32>().ok());
        (major, minor)
    }
    let (cur_maj, cur_min) = parts(current);
    let (lat_maj, lat_min) = parts(latest);
    match (cur_maj, lat_maj, cur_min, lat_min) {
        (Some(a), Some(b), _, _) if a != b => BumpKind::Major,
        (Some(_), Some(_), Some(a), Some(b)) if a != b => BumpKind::Minor,
        (Some(_), Some(_), Some(_), Some(_)) => BumpKind::Patch,
        _ => BumpKind::Major,
    }
}

/// One row in the modal — drift between a placed Symbol's pinned
/// version and its source row's current version.
#[derive(Debug, Clone)]
pub struct LibraryUpdateEntry {
    /// Schematic-side UUID for the placed `Symbol` — used to look up
    /// the instance back on apply (the engine indexes by Symbol UUID).
    pub symbol_uuid: Uuid,
    /// Reference designator (e.g. `R12`, `U3`) — sort key + display
    /// label.
    pub ref_des: String,
    /// The row's Internal PN, read with the row at scan time; shown muted
    /// beside the designator. Empty when there is none to show.
    pub internal_pn: String,
    /// Embedded-symbol key (`<library>:<symbol>`). Every instance sharing it
    /// shares one embedded copy, so the modal selects them together.
    pub lib_id: String,
    /// Source library that owns the row.
    pub library_id: Uuid,
    /// Library's display name — pulled from `OpenLibrary.display_name`
    /// at scan-time so the modal can show the user a friendly source
    /// when the bump dialog opens.
    pub library_name: String,
    /// Row identity inside the library.
    pub row_id: RowId,
    /// Library path (absolute `.snxlib` file path) — kept on the entry
    /// so the apply step can re-resolve through `LibraryState::set`
    /// even if the row's adapter changed mounts between scan and apply.
    pub library_path: PathBuf,
    /// Pinned version on the placed Symbol.
    pub current_version: String,
    /// Row's current version inside the library.
    pub latest_version: String,
    /// Severity hint — drives the checkbox default + Change pill.
    pub bump_kind: BumpKind,
    /// Live checkbox state — pre-set from `bump_kind.default_checked()`
    /// on modal open; the dispatcher toggles it via
    /// `LibraryMessage::LibraryUpdatesToggleSelection`.
    pub selected: bool,
    /// Fields where both the part and the row moved, each with the user's
    /// answer (default: keep the part's value). Applied with the update, in
    /// the same undo step (#2004).
    pub conflicts: Vec<ConflictChoice>,
}

impl LibraryUpdateEntry {
    /// The answers to this part's conflicts.
    pub fn choices(&self) -> Choices {
        self.conflicts.iter().map(|c| (c.field, c.choice)).collect()
    }
}

/// One field of a part that differs from its row after both moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictChoice {
    pub field: StampField,
    /// What the part holds.
    pub mine: String,
    /// What the row offers now.
    pub library: String,
    pub choice: Choice,
}

impl ConflictChoice {
    /// An open conflict, answered "keep mine" until the user says otherwise.
    pub fn new(view: ConflictView) -> Self {
        Self {
            field: view.field,
            mine: view.mine,
            library: view.library,
            choice: Choice::KeepMine,
        }
    }
}

/// Modal state — owned by [`crate::library::LibraryState::library_updates`].
/// `None` while closed; populated by the schematic-open scan. Sorted
/// by `ref_des` in natural order (`R2` before `R12`).
#[derive(Debug, Clone)]
pub struct LibraryUpdatesState {
    /// Schematic the scan ran against — drift applies into this
    /// schematic's engine when the user clicks Update Selected. Kept
    /// so the dispatcher can re-resolve `engines.get_mut(&path)` at
    /// apply time without hauling an extra arg through the message.
    pub schematic_path: PathBuf,
    /// Drift rows, sorted by `ref_des`.
    pub entries: Vec<LibraryUpdateEntry>,
    /// The open-time scan could not check or update some symbol: a later
    /// Apply must not clear the sheet's pending-update flag.
    pub scan_failed: bool,
    /// [`signex_engine::Engine::instance_id`] of the sheet when scanned.
    pub engine_instance: u64,
}

impl LibraryUpdatesState {
    /// Build a state from a freshly-collected drift list. Auto-sorts
    /// by `ref_des` and applies the `bump_kind.default_checked()`
    /// rule to each entry's checkbox.
    pub fn new(schematic_path: PathBuf, mut entries: Vec<LibraryUpdateEntry>) -> Self {
        entries
            .sort_by(|a, b| signex_types::designator::compare_references(&a.ref_des, &b.ref_des));
        // Instances of one symbol share a copy, so they share a checkbox:
        // default on only when every one of them is a safe (patch) bump.
        let safe: Vec<bool> = entries
            .iter()
            .map(|e| {
                entries
                    .iter()
                    .filter(|o| o.lib_id == e.lib_id)
                    .all(|o| o.bump_kind.default_checked())
            })
            .collect();
        for (entry, safe) in entries.iter_mut().zip(safe) {
            entry.selected = safe;
        }
        Self {
            schematic_path,
            entries,
            scan_failed: false,
            engine_instance: 0,
        }
    }

    /// Toggle the checkbox of the entry addressed by `symbol_uuid` and of
    /// every entry sharing its `lib_id` (one embedded copy, one decision).
    /// No-op when the uuid isn't present (e.g. the user dismissed and
    /// re-scanned).
    pub fn toggle(&mut self, symbol_uuid: Uuid) {
        let Some(e) = self.entries.iter().find(|e| e.symbol_uuid == symbol_uuid) else {
            return;
        };
        let (lib_id, selected) = (e.lib_id.clone(), !e.selected);
        for e in self.entries.iter_mut().filter(|e| e.lib_id == lib_id) {
            e.selected = selected;
        }
    }

    /// Tick (`true`) or clear (`false`) every entry. Entries sharing a
    /// `lib_id` end up alike, as [`Self::toggle`] keeps them.
    pub fn set_all(&mut self, selected: bool) {
        for e in &mut self.entries {
            e.selected = selected;
        }
    }

    /// Every entry is ticked (and there is at least one): the header
    /// checkbox shows checked, and a click on it clears them all.
    pub fn all_selected(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|e| e.selected)
    }

    /// The header checkbox was clicked: clear every entry when all are
    /// ticked, else tick every entry.
    pub fn toggle_all(&mut self) {
        let selected = !self.all_selected();
        self.set_all(selected);
    }

    /// Record the answer to one conflict of the part `symbol_uuid`. Answering
    /// ticks the entry (and, as with a click, the entries sharing its
    /// `lib_id`): an answer on an unticked entry would be ignored.
    pub fn answer_conflict(&mut self, symbol_uuid: Uuid, field: StampField, choice: Choice) {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.symbol_uuid == symbol_uuid)
        else {
            return;
        };
        let Some(conflict) = entry.conflicts.iter_mut().find(|c| c.field == field) else {
            return;
        };
        conflict.choice = choice;
        let lib_id = entry.lib_id.clone();
        for e in self.entries.iter_mut().filter(|e| e.lib_id == lib_id) {
            e.selected = true;
        }
    }

    /// Number of entries the user has currently checked.
    pub fn selected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.selected).count()
    }

    /// Number of entries of bump `kind`: the count on its summary pill.
    pub fn count_of(&self, kind: BumpKind) -> usize {
        self.entries.iter().filter(|e| e.bump_kind == kind).count()
    }
}
