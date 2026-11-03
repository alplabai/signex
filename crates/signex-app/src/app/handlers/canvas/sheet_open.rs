//! Opening a sheet without reading it inside `update` (#859, #1756).
//!
//! Every route that shows a sheet by path (the Navigator, net navigation, Find,
//! search reveal, Up Hierarchy, the canvas descent) goes through
//! [`Signex::open_sheet`]. A sheet that is already open is focused at once;
//! anything else is read on a worker and the tab is installed when the result
//! lands, only while it is still the read that was asked for. What the route
//! wanted to happen once the sheet is showing travels with the request
//! ([`SheetOpen`]) instead of running straight after the call.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::schematic::{InstancePath, SelectedItem};
use uuid::Uuid;

use super::super::super::*;

/// The placement a sheet's tab shows once opened.
#[derive(Debug, Clone)]
pub enum PlacementOnOpen {
    /// Leave whatever placement the tab already records.
    Keep,
    /// Record this one; `None` clears a stale one rather than leave the tab on
    /// a different copy than the user entered.
    Record(Option<InstancePath>),
}

/// What to do once the opened sheet is the focused one.
#[derive(Debug, Clone)]
pub enum AfterOpen {
    Nothing,
    /// Centre on a world point and select `item` there.
    FocusAt {
        x: f64,
        y: f64,
        item: Option<SelectedItem>,
    },
    /// Search reveal: select `item` at the position it has now.
    Reveal {
        item: Option<SelectedItem>,
    },
    /// Offer Create Sheet for `symbol`, a sheet symbol on the sheet just opened.
    OfferCreate {
        symbol: Uuid,
    },
    /// A back / forward step: the sheet shown is the step's destination.
    ArriveHistory {
        forward: bool,
    },
}

/// One request to show a sheet.
#[derive(Debug, Clone)]
pub struct SheetOpen {
    pub placement: PlacementOnOpen,
    pub then: AfterOpen,
    /// The sheet symbol the file belongs to: a file found missing offers to
    /// create it for this symbol (#1275).
    pub create_for: Option<Uuid>,
}

impl SheetOpen {
    pub fn plain() -> Self {
        Self {
            placement: PlacementOnOpen::Keep,
            then: AfterOpen::Nothing,
            create_for: None,
        }
    }

    pub fn at(placement: Option<InstancePath>) -> Self {
        Self {
            placement: PlacementOnOpen::Record(placement),
            ..Self::plain()
        }
    }

    pub fn then(mut self, then: AfterOpen) -> Self {
        self.then = then;
        self
    }
}

/// A sheet open whose file is being read on a worker: the request, the sheet it
/// came from, and the read it waits for.
#[derive(Debug)]
pub struct PendingSheetOpen {
    open: SheetOpen,
    /// The sheet the request started on: a missing child is offered for
    /// creation only while that sheet is still the focused one.
    parent: Option<PathBuf>,
    /// The read this open waits for; any other completion for the path (a
    /// stale one) must leave it alone. `None` when the path was claimed by an
    /// untracked read (File > Open): [`Signex::settle_untracked_open`]
    /// answers it when that read ends.
    generation: Option<u64>,
    /// `UiState::sheet_open_seq` when requested.
    seq: u64,
    /// The tab that showed this file under another path (a link): where the
    /// request's jump runs.
    aliased_to: Option<PathBuf>,
}

impl PendingSheetOpen {
    /// What the request runs once its sheet is showing.
    pub(crate) fn requested_then(&self) -> &AfterOpen {
        &self.open.then
    }

    /// Record that the file turned out to be the open tab `held`.
    pub(crate) fn aliased(&mut self, held: PathBuf) {
        self.aliased_to = Some(held);
    }

    /// Parked on an untracked read (File > Open) rather than a tracked one.
    pub(crate) fn is_untracked(&self) -> bool {
        self.generation.is_none()
    }
}

impl Signex {
    /// Show the sheet at `path`. Returns the task that reads it when it is not
    /// open yet; the request in `open` runs when the sheet is showing.
    pub(crate) fn open_sheet(&mut self, path: PathBuf, open: SheetOpen) -> Task<Message> {
        self.open_sheet_outcome(path, open).0
    }

    /// [`Self::open_sheet`], also saying whether the open was refused (a
    /// rename or delete still moving the file), so a caller need not infer it
    /// from state.
    pub(crate) fn open_sheet_outcome(
        &mut self,
        path: PathBuf,
        open: SheetOpen,
    ) -> (Task<Message>, bool) {
        // Record under the spelling the tab is held at (#1149), not the
        // spelling the hierarchy resolved.
        let path = self.resolve_held_spelling_now(path);
        self.ui_state.sheet_open_seq += 1;
        let seq = self.ui_state.sheet_open_seq;
        let stepping = matches!(open.then, AfterOpen::ArriveHistory { .. });
        // A back / forward step's own focus change is not a visit.
        self.document_state.net_nav.history.hold(stepping);
        let focused = self.focus_open_sheet(&path);
        self.document_state.net_nav.history.hold(false);
        if focused {
            return (self.arrive_at_sheet(&path, open), false);
        }
        // A rename or delete still moving the file would leave the new tab on
        // a path that is about to vanish.
        if let Err(refusal) = self.refuse_start_while_path_op(&path, "opened") {
            let detail = refusal.to_string();
            crate::diagnostics::log_warning(detail.clone());
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::open(detail));
            return (Task::none(), true);
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Schematic".to_string());
        let parent = self.active_tab_path();
        let task = self.begin_document_read(
            path.clone(),
            crate::app::document_read::ReadJob::ChildSchematic { title },
        );
        // The read this open rides on: the one just started, or the one
        // already in flight for the file.
        let generation = self.document_state.pending_opens.generation_of(&path);
        if generation.is_none() {
            crate::diagnostics::log_info(format!(
                "{} is already being opened by another route; the request waits for it",
                path.display()
            ));
        }
        // One request per path: a later open of a sheet still being read
        // replaces the earlier one (the later request wins).
        if let Some(replaced) = self.ui_state.pending_sheet_opens.insert(
            path.clone(),
            PendingSheetOpen {
                open,
                parent,
                generation,
                seq,
                aliased_to: None,
            },
        ) {
            crate::diagnostics::log_warning(format!(
                "{} was asked to open again while it was being read; the earlier request                  ({}) was replaced",
                path.display(),
                if matches!(replaced.open.then, AfterOpen::ArriveHistory { .. }) {
                    "a history step"
                } else {
                    "a jump"
                }
            ));
        }
        (task, false)
    }

    /// [`Self::open_sheet`] recording `placement`: `None` clears a stale one.
    pub(crate) fn open_sheet_with_placement(
        &mut self,
        path: PathBuf,
        placement: Option<InstancePath>,
    ) -> Task<Message> {
        self.open_sheet(path, SheetOpen::at(placement))
    }

    /// The sheet is showing: record its placement, then run what was asked.
    fn arrive_at_sheet(&mut self, path: &Path, open: SheetOpen) -> Task<Message> {
        if let PlacementOnOpen::Record(placement) = open.placement {
            self.record_descent_placement(path.to_path_buf(), placement);
        }
        if matches!(open.then, AfterOpen::Nothing) {
            return Task::none();
        }
        // The user may have moved on while the sheet was read.
        let held = self.resolve_held_spelling(path.to_path_buf());
        if self.document_state.active_path.as_deref() != Some(held.as_path()) {
            crate::diagnostics::log_warning(format!(
                "{} opened but is not the focused sheet; the jump was not made",
                path.display()
            ));
            return Task::none();
        }
        match open.then {
            AfterOpen::Nothing => Task::none(),
            AfterOpen::FocusAt { x, y, item } => self.handle_focus_at(x, y, item),
            AfterOpen::Reveal { item } => self.reveal_item_on_active_sheet(path, item),
            AfterOpen::OfferCreate { symbol } => {
                self.open_create_child_sheet_dialog(symbol, true);
                Task::none()
            }
            AfterOpen::ArriveHistory { forward } => {
                let landed = self.active_tab_path().unwrap_or_else(|| path.to_path_buf());
                self.document_state.net_nav.history.arrive(forward, &landed);
                Task::none()
            }
        }
    }

    /// The pending open for `path`, only when it waits for read `generation`.
    fn take_sheet_open(&mut self, path: &Path, generation: u64) -> Option<PendingSheetOpen> {
        let waits = self
            .ui_state
            .pending_sheet_opens
            .get(path)
            .is_some_and(|d| d.generation == Some(generation));
        if waits {
            self.ui_state.pending_sheet_opens.remove(path)
        } else {
            None
        }
    }

    /// An untracked read of the schematic at `path` (File > Open) ended. An
    /// open that was deduplicated against its claim has no read of its own to
    /// wait on: run it now when the tab is up, otherwise read the file again
    /// under a tracked claim so a missing child still offers Create Sheet.
    pub(crate) fn settle_untracked_open(&mut self, path: &Path) -> Task<Message> {
        let waits = self
            .ui_state
            .pending_sheet_opens
            .get(path)
            .is_some_and(|d| d.is_untracked());
        if !waits {
            return Task::none();
        }
        let Some(descent) = self.ui_state.pending_sheet_opens.remove(path) else {
            return Task::none();
        };
        if self.is_overtaken(&descent, path) {
            crate::diagnostics::log_warning(format!(
                "{} was opened by another route; the request for it was dropped: a newer request or a history move overtook it",
                path.display()
            ));
            return Task::none();
        }
        if self.has_tab_for(path) {
            return self.arrive_at_sheet(path, descent.open);
        }
        let parent = descent.parent;
        let (task, refused) = self.open_sheet_outcome(path.to_path_buf(), descent.open);
        if !refused && let Some(pending) = self.ui_state.pending_sheet_opens.get_mut(path) {
            // The Create offer is still tied to the sheet it was entered from.
            pending.parent = parent;
        }
        task
    }

    /// Whether the open waiting on read `generation` of `path` is a history
    /// step, whose tab install must not count as a visit.
    pub(crate) fn is_history_step_read(&self, path: &Path, generation: u64) -> bool {
        self.ui_state
            .pending_sheet_opens
            .get(path)
            .is_some_and(|d| {
                d.generation == Some(generation)
                    && matches!(d.open.then, AfterOpen::ArriveHistory { .. })
            })
    }

    /// Whether the open waiting on read `generation` of `path` was overtaken
    /// while it ran: a newer sheet-open request exists, or a history step's
    /// entry is no longer the one the stack would step to. Its tab still
    /// installs, but it neither takes the view nor runs its follow-up.
    pub(crate) fn is_superseded_sheet_open(&self, path: &Path, generation: u64) -> bool {
        let Some(open) = self
            .ui_state
            .pending_sheet_opens
            .get(path)
            .filter(|d| d.generation == Some(generation))
        else {
            return false;
        };
        self.is_overtaken(open, path)
    }

    fn is_overtaken(&self, open: &PendingSheetOpen, path: &Path) -> bool {
        if open.seq != self.ui_state.sheet_open_seq {
            return true;
        }
        match open.open.then {
            AfterOpen::ArriveHistory { forward } => {
                self.document_state.net_nav.history.peek(forward) != Some(path)
            }
            _ => false,
        }
    }

    /// The read of `path` ended: when its tab is up, record the placement and
    /// run what the open asked. Anything else (a failed or stale read) leaves
    /// nothing to do.
    pub(crate) fn finish_sheet_open(&mut self, path: &Path, generation: u64) -> Task<Message> {
        let superseded = self.is_superseded_sheet_open(path, generation);
        let Some(descent) = self.take_sheet_open(path, generation) else {
            return Task::none();
        };
        let shown = descent.aliased_to.unwrap_or_else(|| path.to_path_buf());
        if !self.has_tab_for(&shown) {
            return Task::none();
        }
        if superseded {
            if let PlacementOnOpen::Record(placement) = descent.open.placement {
                self.record_descent_placement(shown, placement);
            }
            crate::diagnostics::log_warning(format!(
                "{} opened after a newer request for a sheet; the view was left where it was",
                path.display()
            ));
            return Task::none();
        }
        self.arrive_at_sheet(&shown, descent.open)
    }

    /// The read of `path` failed (`missing` when the file is not there): a
    /// descent through a sheet symbol offers to create the sheet, a history
    /// step forgets the entry it could not reach.
    pub(crate) fn sheet_read_failed(
        &mut self,
        path: &Path,
        generation: u64,
        missing: bool,
    ) -> Task<Message> {
        let Some(descent) = self.take_sheet_open(path, generation) else {
            return Task::none();
        };
        match descent.open.then {
            // The step goes on to the next sheet it can reach, as it did
            // when the file was checked before the step.
            AfterOpen::ArriveHistory { forward } => {
                if !self.drop_unreadable_history_entry(forward, path) {
                    crate::diagnostics::log_warning(format!(
                        "History step to {} failed, but the history moved on while it was                          read; the step was not retried",
                        path.display()
                    ));
                    return Task::none();
                }
                return self.step_sheet_history(forward);
            }
            AfterOpen::Reveal { .. } => crate::diagnostics::log_warning(format!(
                "Search: could not open {}; the list may be out of date.",
                path.display()
            )),
            _ => {}
        }
        let Some(symbol) = descent.open.create_for.filter(|_| missing) else {
            return Task::none();
        };
        if descent.parent != self.active_tab_path() {
            crate::diagnostics::log_warning(format!(
                "Create Sheet was not offered for {}: the sheet it was entered from is no \
                 longer the focused one",
                path.display()
            ));
            return Task::none();
        }
        self.open_create_child_sheet_dialog(symbol, true);
        Task::none()
    }
}
