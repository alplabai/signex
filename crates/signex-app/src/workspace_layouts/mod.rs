//! Workspace layouts (mockup row 166): a named arrangement of the docked panels, their
//! docks and sizes, and the split of the document area. Four built-ins
//! (Schematic, Layout, Review, Notebook) plus the user's own, stored per user
//! in `prefs.json`. View ▸ Window ▸ Layouts applies one; Save Layout and
//! Manage Layouts edit the list. Design: docs/WORKSPACE_LAYOUTS.md.
//!
//! This module holds the data, the rules, the store and the dialog state;
//! the update half is `app/handlers/workspace_layouts.rs`, the dialogs are
//! `app/view/dialogs/workspace_layouts/`.

pub mod builtins;
pub mod model;
pub mod problems;
pub mod store;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub use model::{
    BuiltinLayout, Captured, DeletedLayout, DocumentSplit, LayoutId, Layouts, NameClash, Naming,
    RenameRefusal, SaveOutcome, SaveRefusal, WorkspaceLayout,
};
pub use store::{ImportRefusal, LayoutProblem, PrefsWorkspaceLayoutsStore, WorkspaceLayoutsStore};

/// How long the "Deleted layout" toast offers Undo.
pub const UNDO_TOAST_DURATION: Duration = Duration::from_secs(8);

/// The layouts and their two dialogs, in `UiState`.
pub struct WorkspaceLayoutsState {
    pub layouts: Layouts,
    /// What the stored list holds that this build could not read; every
    /// write puts it back.
    pub kept: store::KeptEntries,
    /// Where the list is written; a test swaps in a recorder.
    pub store: Arc<dyn WorkspaceLayoutsStore>,
    pub save: Option<SaveLayoutDialog>,
    pub manage: Option<ManageLayoutsDialog>,
    /// Hands out the undo-toast generations.
    pub toast_generation: u64,
}

impl WorkspaceLayoutsState {
    /// The state a launch starts with: the list read from `prefs` (its whole
    /// JSON), written back through `store`. The problems name skipped
    /// entries, for the Messages panel.
    pub fn boot(
        prefs: Option<&serde_json::Value>,
        store: Arc<dyn WorkspaceLayoutsStore>,
    ) -> (Self, Vec<LayoutProblem>) {
        let (layouts, problems, kept) = store::read_prefs(prefs);
        (
            Self {
                layouts,
                kept,
                store,
                save: None,
                manage: None,
                toast_generation: 0,
            },
            problems,
        )
    }
}

/// Save Layout, while open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveLayoutDialog {
    pub name: String,
    /// "Open panels, their docks and sizes".
    pub store_panels: bool,
    /// "Split of the document area"; only stored while there is a split.
    pub store_split: bool,
}

/// Manage Layouts, while open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManageLayoutsDialog {
    pub selected: LayoutId,
    /// The inline Rename field's text, while it is open.
    pub rename: Option<String>,
    /// A user row dragged by its grip.
    pub drag: Option<RowDrag>,
    /// The "Deleted layout" toast with its Undo, while it shows.
    pub toast: Option<UndoToast>,
}

impl ManageLayoutsDialog {
    pub fn new(selected: LayoutId) -> Self {
        Self {
            selected,
            rename: None,
            drag: None,
            toast: None,
        }
    }
}

/// A user row on its way from `from` to `over` (indices of the user list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowDrag {
    pub from: usize,
    pub over: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoToast {
    pub deleted: DeletedLayout,
    pub generation: u64,
}

/// What View ▸ Window ▸ Layouts shows, read off the Model for the menu
/// (plain data, so the native menu can tell when it changed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutsMenu {
    /// The built-in on screen, which carries the check mark.
    pub active_builtin: Option<BuiltinLayout>,
    pub user: Vec<LayoutsMenuRow>,
    /// The active built-in's name while it carries an override: the Reset row.
    pub resettable: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutsMenuRow {
    pub id: String,
    pub name: String,
    pub active: bool,
}

impl LayoutsMenu {
    pub fn of(layouts: &Layouts, naming: &Naming) -> Self {
        let active = layouts.active();
        Self {
            active_builtin: active.and_then(LayoutId::builtin),
            user: layouts
                .user()
                .iter()
                .map(|l| LayoutsMenuRow {
                    id: l.id.key().to_string(),
                    name: l.name.clone(),
                    active: active == Some(&l.id),
                })
                .collect(),
            resettable: layouts.resettable().map(|b| naming.builtin(b).to_string()),
        }
    }
}

/// The names of `i18n`'s language. Plain text (no isolation marks): the
/// names are stored.
pub fn naming(i18n: &Arc<signex_i18n::I18n>) -> Naming {
    use signex_i18n::ids;
    let name = |id| i18n.message_plain(id, &[]);
    let numbered = Arc::clone(i18n);
    let copy_of = Arc::clone(i18n);
    let nth = Arc::clone(i18n);
    Naming {
        builtins: [
            name(ids::WORKSPACE_LAYOUTS_SCHEMATIC),
            name(ids::WORKSPACE_LAYOUTS_LAYOUT),
            name(ids::WORKSPACE_LAYOUTS_REVIEW),
            name(ids::WORKSPACE_LAYOUTS_NOTEBOOK),
        ],
        numbered: Box::new(move |n| {
            numbered.message_plain(
                ids::WORKSPACE_LAYOUTS_NUMBERED,
                &[("n", signex_i18n::Arg::from(n))],
            )
        }),
        copy_of: Box::new(move |source| {
            copy_of.message_plain(
                ids::WORKSPACE_LAYOUTS_COPY_OF,
                &[("name", signex_i18n::Arg::from(source))],
            )
        }),
        nth: Box::new(move |base, n| {
            nth.message_plain(
                ids::WORKSPACE_LAYOUTS_NTH,
                &[
                    ("name", signex_i18n::Arg::from(base)),
                    ("n", signex_i18n::Arg::from(n)),
                ],
            )
        }),
    }
}

/// Messages of the workspace layouts, under
/// `Message::DockLayout(DockLayoutMsg::Workspace(..))`.
#[derive(Debug, Clone)]
pub enum WorkspaceLayoutMsg {
    /// A Layouts row, its command or Manage's Apply: arrange the panels.
    ApplyRequested(LayoutId),
    /// View ▸ Window ▸ Layouts ▸ Reset "<name>": the active built-in's
    /// override is dropped and the built-in applied as shipped.
    ResetActiveRequested,
    Save(SaveLayoutMsg),
    Manage(ManageLayoutsMsg),
    /// The list's `prefs.json` write finished; the error is what Messages shows.
    Saved(Result<(), String>),
}

#[derive(Debug, Clone)]
pub enum SaveLayoutMsg {
    Requested,
    NameEdited(String),
    PanelsToggled,
    SplitToggled,
    Confirmed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum ManageLayoutsMsg {
    Requested,
    Closed,
    RowSelected(LayoutId),
    RenameStarted,
    RenameEdited(String),
    RenameCommitted,
    RenameCancelled,
    DuplicateRequested,
    DeleteRequested,
    UndoDeleteRequested,
    /// The toast of generation `.0` timed out; a newer toast stays.
    ToastExpired(u64),
    UpdateFromCurrentRequested,
    ApplyRequested,
    /// A user row's grip was pressed (its index in the user list).
    GripPressed(usize),
    /// The pointer entered user row `.0` (a drag moves over it).
    RowEntered(usize),
    /// The button went up over the list: a drag in progress drops.
    GripReleased,
    ImportRequested,
    ImportPicked(Option<PathBuf>),
    ImportRead {
        path: PathBuf,
        result: Result<store::ImportRead, ImportRefusal>,
    },
    ExportRequested,
    ExportPicked(Option<PathBuf>),
    /// The written path, or the system's reason the write failed.
    ExportWritten(Result<PathBuf, String>),
}

impl ManageLayoutsMsg {
    /// Whether this changes the stored list, so an unreadable list refuses
    /// it. A drag is refused where it drops.
    pub fn edits_the_list(&self) -> bool {
        matches!(
            self,
            Self::RenameCommitted
                | Self::DuplicateRequested
                | Self::DeleteRequested
                | Self::UndoDeleteRequested
                | Self::UpdateFromCurrentRequested
                | Self::ImportRequested
                | Self::ImportRead { .. }
        )
    }
}

impl From<WorkspaceLayoutMsg> for crate::app::Message {
    fn from(msg: WorkspaceLayoutMsg) -> Self {
        Self::DockLayout(crate::dock::DockLayoutMsg::Workspace(msg))
    }
}

impl From<SaveLayoutMsg> for crate::app::Message {
    fn from(msg: SaveLayoutMsg) -> Self {
        WorkspaceLayoutMsg::Save(msg).into()
    }
}

impl From<ManageLayoutsMsg> for crate::app::Message {
    fn from(msg: ManageLayoutsMsg) -> Self {
        WorkspaceLayoutMsg::Manage(msg).into()
    }
}
