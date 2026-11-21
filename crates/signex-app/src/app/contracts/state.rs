//! Context actions + modal/dialog state structs.

/// R / G / B channel selector for the custom net-colour picker inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
}

#[derive(Debug, Clone)]
pub enum ContextAction {
    /// Run a catalog command exactly as the palette and the keymap do:
    /// the menu closes, then the id is dispatched with the point the menu
    /// was opened at, and `project` when the row stands on a project (#1727).
    Command {
        id: crate::keymap::AppCommandId,
        project: Option<String>,
    },
    OpenChildSheet,
    /// Write the missing child file of the selected sheet symbol, with one
    /// port per entry, then open it.
    CreateChildSheet,
    /// Reconcile the selected sheet symbol's pins with the child schematic's
    /// hierarchical / global labels (#359). Gated like `OpenChildSheet`.
    SyncSheetPins,
    /// Run an Active Bar action from a context-menu submenu row that has no
    /// command id (the power ports). Closes both menus and dispatches the
    /// action through the Active Bar handler.
    ActiveBar(crate::active_bar::ActiveBarAction),
    /// A board menu row of a project's reuse block (#2410): the menu closes,
    /// then the event runs on the board of the window the menu was in.
    ReuseBlock(crate::app::board_reuse::ReuseBlockMsg),
}

/// Which click-to-open submenu is currently expanded inside the right-
/// click context menu, if any. Owned by `InteractionState` and cleared
/// alongside `context_menu` whenever the menu closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSubmenu {
    Place,
    Align,
    /// "Arrange ›": order the selected drawings (#1583). Shown only when the
    /// selection holds a drawing.
    Arrange,
    /// "Part Actions ›": the selected placed part's library actions (#2419).
    PartActions,
    /// "Sheet Actions ›": the selected sheet symbol's child-sheet actions and
    /// the project's sheet numbering (#2419).
    SheetActions,
    /// Project-tree → "Add New to Project ›" launcher. Items are
    /// version-tagged placeholders today; they do not create documents yet.
    AddNewToProject,
    /// "Unions ›": create a union from the selection, add or remove the
    /// selection, break the union (mockup row 158). Shown with a selection.
    Unions,
    /// The greyed "Planned (N) ›" launcher of the sheet menu: the features
    /// not built yet, each row with what it will do (Graphite mockup #62).
    Planned,
    /// Project-tree root → "Planned ›": the project features that are not
    /// built yet, as greyed rows (mockup row 20).
    ProjectPlanned,
    /// Project-tree library node → "Add New ›": a component, symbol library
    /// or footprint library in that library (mockup row 20).
    AddNewToLibrary,
}

/// Widget id of the in-place editor, so opening it can focus it.
pub static TEXT_EDIT_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.text_edit"));

/// Widget id of the status bar's board angle field, so opening it can focus
/// it (#2316).
pub static VIEW_ANGLE_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.view_angle"));

/// Widget id of the Rename dialog's name field, focused with its text
/// selected when the dialog opens.
pub static RENAME_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.rename.name"));

/// The in-place editor. `kind` says which text of `uuid` it edits: a label
/// or text note, a part's designator (`SymbolRefField`) or value
/// (`SymbolValField`), or, as `Symbol`, the part's footprint, which has no
/// text on the canvas.
#[derive(Debug, Clone)]
pub struct TextEditState {
    pub uuid: uuid::Uuid,
    pub kind: signex_types::schematic::SelectedKind,
    pub text: String,
    pub original_text: String,
    /// World-space position of the object being edited (mm). Converted to
    /// screen coords at render time so the inline editor tracks pan/zoom.
    pub world_x: f64,
    pub world_y: f64,
    /// The sheet the editor opened on and, for a designator, the placement
    /// it showed. A commit that finds either changed is refused: it would
    /// write to a different copy than the one the user typed into.
    pub sheet: Option<std::path::PathBuf>,
    pub placement: Option<signex_types::schematic::InstancePath>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
    /// Where on the sheet the menu was opened, in world mm; `None` when the
    /// cursor position was not known to belong to this document.
    pub world: Option<(f64, f64)>,
    /// The undocked window the menu was opened in; `None` for the main
    /// window. Only that window paints it and its actions act on that
    /// window's sheet (#1910).
    pub window: Option<iced::window::Id>,
}

/// v0.18.10 — Altium-style grid picker popup state. Anchors the
/// floating menu at the cursor position when `G` is pressed.
#[derive(Debug, Clone)]
pub struct GridPickerState {
    pub x: f32,
    pub y: f32,
    /// The footprint its rows and Grid Properties act on.
    pub target: std::path::PathBuf,
    /// The undocked window it opened in; `None` for the main window.
    pub owner: Option<iced::window::Id>,
}

/// Grid Properties dialog draft (Ctrl+G in a footprint editor): the step
/// text, the fine and coarse display and the multiplier. OK
/// (`GridPropertiesMsg::Apply`) writes them; Cancel discards.
#[derive(Debug, Clone, Default)]
pub struct GridPropertiesState {
    /// Grid step in mm; one value drives both axes.
    pub step_mm: String,
    pub fine_display: crate::library::editor::footprint::state::GridDisplay,
    pub coarse_display: crate::library::editor::footprint::state::GridDisplay,
    pub multiplier: u32,
    /// The live preview is drawn taller.
    pub preview_expanded: bool,
    /// The step and coarse labels on the preview: which one is open.
    pub preview_labels: signex_widgets::dimension_label::DimensionLabels,
    /// The document Apply writes to: the one it was opened on.
    pub target: std::path::PathBuf,
}

/// v0.18.14.1 — Custom Selection Filter modal draft state. Mirrors
/// `SelectionFilter` from `library::editor::footprint::state` so
/// the user can flip flags without touching the live editor until
/// they hit Apply.
#[derive(Debug, Clone)]
pub struct SelectionFilterCustomState {
    pub pads: bool,
    pub tracks: bool,
    pub arcs: bool,
    pub pours: bool,
    pub bodies_3d: bool,
    pub keepouts: bool,
    pub cutouts: bool,
    pub texts: bool,
    pub vias: bool,
    pub regions: bool,
    pub fills: bool,
    pub other: bool,
    /// The name Apply saves the filter under as a preset; blank = "Filter N".
    pub preset_name: String,
    /// The document Apply writes to: the one it was opened on.
    pub target: std::path::PathBuf,
}

/// State for the Projects-panel tree-view right-click menu. The menu's
/// action set is computed from `key` (leaf vs branch vs empty) at render
/// time, so we only need to store the anchor coordinates + the clicked
/// row's stable key (or `None` for the background menu). A key, not an
/// index path: the tree can be rebuilt while the menu is open, and a stale
/// index would aim the picked action at a different node.
#[derive(Debug, Clone)]
pub struct ProjectTreeContextMenuState {
    pub x: f32,
    pub y: f32,
    /// `Some(key)` = right-click on a specific node; `None` = right-click
    /// in empty tree area, offering only the generic actions.
    pub key: Option<signex_widgets::tree_view::TreeKey>,
    /// The undocked window the menu was opened in (`None`: the main window).
    /// Only that window paints it, and the tree it names is that window's.
    pub window: Option<iced::window::Id>,
}

/// State for the "Close Project — Unsaved Edits" confirmation modal.
/// Opens only when the user closes a project that has at least one
/// dirty document rooted in the project's directory; the modal lists every dirty file by filename so the
/// user can see what they're about to lose.
#[derive(Debug, Clone)]
pub struct ProjectCloseConfirmState {
    /// The project the close was requested for. Held by id, not list
    /// index: the list may shift while the modal is up, so the confirm step
    /// looks it up again and aborts when it is gone.
    pub project_id: crate::app::ProjectId,
    /// Project display name shown in the modal header.
    pub project_name: String,
    /// Absolute paths of dirty files inside the project's directory.
    /// The view layer renders the file basenames; the handler uses
    /// the absolute paths to locate the engines for save / discard.
    pub dirty_paths: Vec<std::path::PathBuf>,
    /// Component Preview rows with unsaved edits whose library lives
    /// inside the project's directory. They are keyed by row address,
    /// not by file, so `dirty_paths` cannot carry them.
    pub dirty_rows: Vec<crate::library::state::EditorAddress>,
    /// Dirty documents of this project that another open project claims.
    /// They are not saved or discarded: closing leaves them open, and the
    /// modal names them so nothing is dropped unannounced.
    pub kept_paths: Vec<std::path::PathBuf>,
    /// The child-sheet references the dirty list was attributed with. The
    /// Save All / Discard All paths reuse them for the engine sweep instead
    /// of reading the disk again.
    pub sheet_refs: crate::app::state::sheet_refs::SheetRefs,
}

/// Why a child sheet could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildSheetWriteError {
    /// The file was already there; it was left alone.
    AlreadyExists,
    Failed(String),
}

/// The "Create Sheet" dialog: the file a sheet symbol points at does not
/// exist, and the user picks where to write it. Held by the parent sheet's
/// path and the symbol's id, so the answer acts on THAT document whatever tab
/// is active by then.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateChildSheetConfirmState {
    pub symbol: uuid::Uuid,
    /// The symbol's name, for the dialog text.
    pub name: String,
    /// The sheet that carries the symbol.
    pub parent: std::path::PathBuf,
    /// The project folder the file must stay inside (the parent's folder for
    /// a loose sheet).
    pub project_dir: std::path::PathBuf,
    /// The editable file path: relative to the parent's folder, or absolute.
    pub path_text: String,
    /// Why Create refused, shown under the field.
    pub error: Option<signex_i18n::Translatable>,
    /// The typed path names a file that already exists: offer to use it.
    pub exists: bool,
    /// What using that existing file would make of this symbol, when the
    /// Model shows the file is already a page of the project or placed by a
    /// sheet symbol of another open sheet (mockup row 27); `None` otherwise.
    pub reuse_warning: Option<signex_i18n::Translatable>,
}

/// State for the "Exit Signex — Unsaved Edits" confirmation modal.
/// Opens when the user requests app exit (chrome ✕, File ▸ Exit,
/// Alt+F4) while any document is dirty. Lists
/// every dirty file across the whole workspace so the user sees what
/// they are about to lose before choosing Save All / Discard All /
/// Cancel. Reuses `ProjectCloseChoice` for the three outcomes.
#[derive(Debug, Clone)]
pub struct AppQuitConfirmState {
    /// Absolute paths of every dirty file in the workspace, sorted
    /// for a stable display order. The view renders the basenames.
    pub dirty_paths: Vec<std::path::PathBuf>,
    /// Every Component Preview row with unsaved edits, sorted. Rows are
    /// keyed by `(library, table, row)` rather than a file path, so they
    /// are listed and saved alongside `dirty_paths`, not inside it.
    pub dirty_rows: Vec<crate::library::state::EditorAddress>,
}

/// What a Save All does once every write it started has completed.
#[derive(Debug, Clone)]
pub enum SaveAllNext {
    /// Close the project; the tail of `ProjectCloseChoice::SaveAll`.
    CloseProject {
        project_id: crate::app::ProjectId,
        sheet_refs: Box<crate::app::state::sheet_refs::SheetRefs>,
        kept_paths: Vec<std::path::PathBuf>,
    },
    /// Close the main window; the tail of the app-quit Save All.
    Quit,
    /// Close a library; the tail of the close-library Save All.
    CloseLibrary { library_path: std::path::PathBuf },
    /// File ▸ Save All: report the outcome and do nothing else.
    Nothing,
}

/// A Save All whose loose documents' volumes have been probed off-thread;
/// it is started, with `probes`, when this message arrives (#1216).
#[derive(Debug, Clone)]
pub struct SaveAllPlan {
    pub context: String,
    pub paths: Vec<std::path::PathBuf>,
    pub rows: Vec<crate::library::state::EditorAddress>,
    pub next: SaveAllNext,
    /// Directory -> whether its volume ignores case, for every directory
    /// the probe could decide. An undecided one is absent.
    pub probes: std::collections::HashMap<std::path::PathBuf, bool>,
    /// The directories the probe could not decide, with its answer; the
    /// Save All reports each from `update` and falls back to the host rule.
    pub undecided: Vec<(
        std::path::PathBuf,
        crate::app::state::project_case::CaseProbeOutcome,
    )>,
}

/// The tail of a Save All, sent when its writes have completed.
#[derive(Debug, Clone)]
pub struct SaveAllSettle {
    pub next: SaveAllNext,
    /// Files and rows the Save All started, a file held under several
    /// spellings counted once.
    pub items: usize,
    /// The dirty files the Save All wrote; any still dirty did not save.
    pub paths: Vec<std::path::PathBuf>,
    /// `(label, reason)` of what already failed before a write started
    /// (Component Preview rows).
    pub failed: Vec<(String, String)>,
    /// The Component Preview rows the Save All started: those whose write
    /// ran off-thread report a failure into `LibraryState::batch_row_failures`.
    pub rows: Vec<crate::library::state::EditorAddress>,
    /// This Save All's id, the key of its rows' entries in
    /// `LibraryState::batch_row_failures`.
    pub batch: u64,
    /// Files held under several case-variant spellings: the spelling that
    /// was written, and the others it stands for. The others' dirty markers
    /// go once the written one is clean.
    pub case_variants: Vec<CaseVariants>,
    /// The writes whose task panicked instead of reporting (#2269). Their
    /// completions never arrive, so the settle fails each one itself.
    pub stopped: Vec<SaveAllWrite>,
}

/// What one Save All write saves, so a write that stops without a result
/// can be failed the way its own completion would fail it.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveAllWrite {
    /// A document file. `generation` is the save stamp of the symbol or
    /// footprint editor the write snapshotted; `None` for other documents.
    File {
        path: std::path::PathBuf,
        generation: Option<u64>,
    },
    /// A Component Preview row.
    Row(crate::library::state::EditorAddress),
}

/// A file held under several case-variant spellings: the one written and
/// the others it stands for, each pinned as it was when the write began.
#[derive(Debug, Clone)]
pub struct CaseVariants {
    pub written: std::path::PathBuf,
    pub others: Vec<PinnedSpelling>,
}

/// A spelling's state when a write began. One edited since (its engine
/// revision moved) is not on disk, so its dirty marker stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedSpelling {
    pub path: std::path::PathBuf,
    pub revision: u64,
}

/// User choice from the project-close confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectCloseChoice {
    /// Save every dirty file in the project, then close.
    SaveAll,
    /// Drop the engines for every dirty file in the project without
    /// writing to disk, then close.
    DiscardAll,
    /// Dismiss the modal; the project stays open.
    Cancel,
}

/// State for the document-tab right-click menu. The menu's items are
/// derived from `tab_idx` (the clicked tab) so the same menu builder
/// works for any tab; mutually exclusive with the canvas and project-
/// tree context menus.
#[derive(Debug, Clone)]
pub struct TabContextMenuState {
    pub x: f32,
    pub y: f32,
    pub tab_idx: usize,
}

/// Concrete actions dispatched when the user picks a menu item in the
/// document-tab right-click menu.
#[derive(Debug, Clone)]
pub enum TabContextAction {
    /// Close just this tab.
    Close(usize),
    /// Close every tab except the one at this index.
    CloseAllOthers(usize),
    /// Close every open tab.
    CloseAll,
    /// Pop the tab at this index into its own OS window.
    Undock(usize),
    /// Close every tab to the right of the one at this index.
    CloseToTheRight(usize),
    /// Put the file path of the tab at this index on the clipboard.
    CopyPath(usize),
    /// Show the Projects panel with the file of the tab at this index
    /// selected in its tree.
    RevealInProjects(usize),
}

/// Concrete actions dispatched when the user picks a menu item in the
/// Projects-panel tree-view context menu.
#[derive(Debug, Clone)]
pub enum ProjectTreeAction {
    /// Open the file backed by this leaf in the current document slot.
    OpenNode(signex_widgets::tree_view::TreeKey),
    /// Expand (or collapse) a specific branch node.
    ToggleNode(signex_widgets::tree_view::TreeKey),
    /// Recursively expand every node in the tree.
    ExpandAll,
    /// Recursively collapse every node in the tree.
    CollapseAll,
    /// Re-scan the project and rebuild the tree from current state.
    Refresh,
    /// Close every open document tab without closing the project
    /// itself. Fired from the project-root "Close Project Documents"
    /// menu item.
    CloseAllDocuments,
    /// Reveal a file (leaf click) or the project directory (root
    /// click) in the OS file manager. The tree path's first index
    /// picks which project's directory the operation resolves
    /// against — leaves nested under project B reveal in B's dir
    /// even when project A is active. A single-element path means
    /// the project root row was clicked.
    RevealInExplorer(signex_widgets::tree_view::TreeKey),
    /// Fire the print preview flow — only surfaced on leaves that are
    /// already the active tab.
    PrintActive,
    /// Open the sheet-rename modal for this leaf, preloaded with the
    /// current filename.
    OpenRenameDialog(signex_widgets::tree_view::TreeKey),
    /// Open the "Remove from Project" modal (Delete / Exclude / Cancel)
    /// for this leaf.
    OpenRemoveDialog(signex_widgets::tree_view::TreeKey),
    /// Close the entire project whose root is at this tree path. Closes
    /// every open tab backed by the project, drops the `LoadedProject`
    /// from the workspace, and promotes another project (or `None`) to
    /// active. The tree path's first index selects the project; other
    /// indices are ignored so the action is safe to fire from any node
    /// underneath a project root.
    CloseProject(signex_widgets::tree_view::TreeKey),
    /// Project root: run ERC across the project. Promotes the
    /// project to active, opens its `schematic_root` if no schematic
    /// from this project is currently active, then dispatches the
    /// existing ERC dialog.
    ValidateProject(signex_widgets::tree_view::TreeKey),
    /// Project root: open the rename modal seeded with the
    /// project name (the `.snxprj` stem). Submit renames the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` in lockstep.
    OpenProjectRenameDialog(signex_widgets::tree_view::TreeKey),
    /// Project root: Project Options… opens (or focuses) the project's
    /// Project tab (#1202).
    OpenProjectOptions(signex_widgets::tree_view::TreeKey),
    /// Project root: open a file dialog and add the picked file
    /// to the project. Files outside the project directory are copied
    /// in; files already inside just trigger a tree refresh.
    AddExistingToProject(signex_widgets::tree_view::TreeKey),
    /// Project root → Add New ▸ Schematic. Spawns a Save-As
    /// dialog scoped to the project directory; the result writes a
    /// blank `.snxsch`, registers it as a SheetEntry, marks the
    /// project dirty, and refreshes the tree (no tab opens).
    AddNewSchematic(signex_widgets::tree_view::TreeKey),
    /// Project root → Add New ▸ PCB: the project's one board, from its
    /// defaults, opened unsaved (#1312).
    AddNewPcb(signex_widgets::tree_view::TreeKey),
    /// Project root → Update PCB Document: the Update PCB dialog for that
    /// project (#1312).
    UpdatePcb(signex_widgets::tree_view::TreeKey),
    /// project-root → Add New ▸ Symbol Library. Save-As dialog
    /// scoped to the project dir, writes an empty `.snxsym`, opens
    /// the file as a primitive editor tab. Altium parity: Schematic
    /// Library is a top-level project document.
    AddProjectSymbolLibrary(signex_widgets::tree_view::TreeKey),
    /// project-root → Add New ▸ PCB Library. Save-As dialog
    /// scoped to the project dir, writes an empty `.snxfpt`, opens
    /// the file as a primitive editor tab.
    AddProjectFootprintLibrary(signex_widgets::tree_view::TreeKey),
    /// v0.11 project-root: open the Enable Version Control confirm
    /// modal. Runs `git init` at the project dir, optionally seeds
    /// `.gitattributes` for binary-model LFS, and creates the
    /// initial commit covering the entire project tree. Only
    /// enabled when the project dir has no `.git/` already.
    OpenEnableVersionControl(signex_widgets::tree_view::TreeKey),
    /// Right-click on a plain-files `.snxlib` node → opens the
    /// Enable Version Control modal scoped to that library directory.
    OpenLibraryEnableVersionControl(LibraryRowRef),
    /// Right-click on a library node → Remove from Project. Resolved by
    /// the library's identity, not its row position (#742).
    RemoveLibrary(LibraryRowRef),
    /// Right-click on a library node → Refresh All Pricing. Runs in the
    /// background; resolved by the library's identity like the rest.
    RefreshLibraryPricing(LibraryRowRef),
}

impl ProjectTreeAction {
    /// Whether the action only changes the tree an undocked window shows
    /// (expansion, rebuild), so it runs in that window's swap. Every other
    /// action opens a tab, a dialog or a file and runs on the main window.
    pub fn changes_only_the_shown_tree(&self) -> bool {
        matches!(
            self,
            Self::ToggleNode(_) | Self::ExpandAll | Self::CollapseAll | Self::Refresh
        )
    }

    /// The tree row this action targets, when it targets one.
    pub fn row_key(&self) -> Option<&signex_widgets::tree_view::TreeKey> {
        use ProjectTreeAction::*;
        match self {
            OpenNode(k)
            | ToggleNode(k)
            | RevealInExplorer(k)
            | OpenRenameDialog(k)
            | OpenRemoveDialog(k)
            | CloseProject(k)
            | ValidateProject(k)
            | OpenProjectRenameDialog(k)
            | OpenProjectOptions(k)
            | AddExistingToProject(k)
            | AddNewSchematic(k)
            | AddNewPcb(k)
            | UpdatePcb(k)
            | AddProjectSymbolLibrary(k)
            | AddProjectFootprintLibrary(k)
            | OpenEnableVersionControl(k) => Some(k),
            ExpandAll
            | CollapseAll
            | Refresh
            | CloseAllDocuments
            | PrintActive
            | OpenLibraryEnableVersionControl(_)
            | RemoveLibrary(_)
            | RefreshLibraryPricing(_) => None,
        }
    }
}

/// Stable identity of a library row in the project tree: the owning
/// project's id plus the library's resolved `.snxlib` path. Library menu
/// actions carry this instead of a row index, so a tree rebuild between
/// opening the menu and picking an action can never retarget a different
/// library (#742).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRowRef {
    pub project_id: crate::app::state::ProjectId,
    pub root: std::path::PathBuf,
}

/// State for the rename modal. Tracks the target file, the live
/// edit buffer, and the clicked tree path so we can rebuild the tree
/// after a successful rename without rediscovering the project.
#[derive(Debug, Clone)]
pub struct RenameDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    /// The owning project, captured when the dialog opened. Submit re-points
    /// `tree_path[0]` at that project's current index, so adding or removing
    /// a project while the dialog is open cannot retarget it (#855). `None`
    /// keeps `tree_path` as given.
    pub project_id: Option<crate::app::state::ProjectId>,
    pub buffer: String,
    pub error: Option<String>,
    /// `true` when this rename targets the project root — the submit
    /// handler renames the `<old>.snxprj` plus the companion
    /// `<old>.snxsch` / `<old>.snxpcb` (whichever exist on disk) so the
    /// trio stays grouped under the new project name. `false` is the
    /// per-file rename used by sheet leaves.
    pub is_project_rename: bool,
    /// Sheet symbols of the file's own project that place it, read from the
    /// Model when the dialog opened. The rename re-points them, and the
    /// dialog says so.
    pub placing_symbols: Vec<crate::app::path_ops::PlacingSymbol>,
    /// The composed new name, when another file the Model knows in the same
    /// folder already has it. Set on every edit; Rename is withheld while it
    /// stands.
    pub name_clash: Option<String>,
}

/// One file/directory entry surfaced on the Enable Version Control
/// picker. The user can opt items out of the initial commit by
/// untoggling `tracked`; untracked entries get written into a
/// generated `.gitignore` so they sit outside the repo from day one.
#[derive(Debug, Clone)]
pub struct TrackItem {
    pub absolute: std::path::PathBuf,
    pub relative: String,
    /// What the entry is; the picker shows its catalog name next to the
    /// path.
    pub kind: TrackKind,
    /// True for directory entries — drives trailing-slash in the
    /// generated `.gitignore` pattern.
    pub is_directory: bool,
    pub tracked: bool,
}

/// The kind of a [`TrackItem`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    /// The `.snxprj`.
    Project,
    /// A schematic sheet.
    Schematic,
    /// The board.
    Pcb,
    /// A project-local library folder.
    Library,
    /// A library's `library.toml`.
    Config,
    /// A library's `components.tsv`.
    Components,
    /// A library subfolder.
    Folder,
}

/// Whether the Enable Version Control modal is initialising a
/// project repo (whole-project tree) or a library repo (a single
/// `.snxlib` directory). Branches the confirm handler so it can
/// run `git init` against the right working tree and emit the
/// scope-appropriate log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionControlScope {
    Project,
    Library,
}

/// State for the "Enable Version Control" confirm modal — opened
/// from the project root context menu when the project directory
/// has no `.git/` yet, or from a plain-files `.snxlib` node's
/// right-click menu. Confirm runs `git2::Repository::init` at
/// `project_dir`, optionally writes `.gitattributes` for binary-
/// model LFS, generates a `.gitignore` from the unticked items,
/// and stages an initial commit covering the picked subset.
#[derive(Debug, Clone)]
pub struct EnableVersionControlState {
    /// Whether this dialog is scoped to a project (the `.snxprj` +
    /// surrounding tree) or to a library directory (a single
    /// `.snxlib` and its `symbols/` / `footprints/` siblings).
    pub scope: VersionControlScope,
    /// For `Project`: path to the `.snxprj` file. For `Library`:
    /// path to the `library.toml` (or equivalent manifest) inside
    /// the library directory — used only for display.
    pub project_path: std::path::PathBuf,
    /// Working tree root the new repo will live at. For projects
    /// this is the `.snxprj` parent; for libraries the `.snxlib`
    /// parent (i.e. the library's root_dir).
    pub project_dir: std::path::PathBuf,
    /// Display name for the modal header. Project: project name.
    /// Library: filename stem of the `.snxlib` (e.g. "MyLib").
    pub project_name: String,
    /// Per-entry tracking picker — tickable rows for each top-level
    /// schematic / pcb / library (project scope) or each top-level
    /// manifest / subdirectory (library scope). Untracked rows get
    /// written into `.gitignore` at confirm time.
    pub items: Vec<TrackItem>,
    /// "Track binary 3D models via Git LFS" checkbox. Off by
    /// default; only writes `.gitattributes` when on.
    pub use_lfs: bool,
    /// Subject of the first commit, prefilled from the catalog ("Initial
    /// version of <name>"). Enable is withheld while it is blank.
    pub commit_message: String,
    /// "Commit every save" (project scope): on success `enable_git` is
    /// turned on only when this is set. On by default.
    pub commit_every_save: bool,
    /// Last error from a confirm attempt — surfaces inline so the
    /// user can fix the cause (LFS not installed, etc.) and retry
    /// without reopening the modal.
    pub error: Option<String>,
    /// A confirmed run is on the blocking pool: the dialog shows it as
    /// working and ignores edits and a second Confirm until it finishes.
    pub working: bool,
    /// The row list is still being read on a worker (library scope, #859):
    /// the dialog shows no rows yet and ignores edits and Confirm until
    /// `EnableVersionControlMsg::ItemsScanned` fills them.
    pub scanning: bool,
}

/// State for the "Remove from Project" modal. `Delete` removes the file
/// from disk; `Exclude` drops it from the session's sheet list but
/// leaves the file in place.
#[derive(Debug, Clone)]
pub struct RemoveDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    /// The owning project; see [`RenameDialogState::project_id`] (#855).
    pub project_id: Option<crate::app::state::ProjectId>,
    pub display_name: String,
    /// Does the target live outside the project folder (a Shared / Global
    /// library)? Such a target is never offered for deletion (#139).
    pub is_outside_project: bool,
    /// Second stage of the same modal: the target has unsaved edits, so
    /// Exclude asks Save / Discard / Cancel before dropping it (#738).
    /// Esc and the close button still resolve to `RemoveMsg::Close`, which
    /// is the Cancel branch.
    pub unsaved_prompt: bool,
    /// Is the target a `.snxlib` directory package (deleted whole, with
    /// everything inside)? Decided when the dialog opens: `view` must not stat.
    pub is_directory_package: bool,
    /// The card the user clicked (#2262). `None` until then, and the card
    /// the modal selects is its default: Exclude when offered, else Delete
    /// (`RemoveOffer::selected`). A pick that is no longer offered falls
    /// back to that default too.
    pub choice: Option<RemoveChoice>,
}

/// User choice from the Remove-from-Project modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveChoice {
    /// Remove from project AND delete the file on disk.
    DeleteFile,
    /// Remove from project; leave the file in its folder.
    ExcludeFromProject,
}

/// User choice from the unsaved-edits stage of the Remove modal (#738).
/// Cancel is `RemoveMsg::Close`, so Esc needs no extra rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcludeDirtyChoice {
    /// Write the sheet's edits, then exclude it.
    Save,
    /// Drop the sheet's edits (logged to Messages), then exclude it.
    Discard,
}

#[derive(Debug, Clone)]
pub enum StatusBarRequest {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
    TogglePanelList,
    /// Click on the selection-summary segment opens the Properties panel
    /// scoped to the current selection.
    OpenPropertiesForSelection,
    /// Click on the "Importing …" segment cancels the running board
    /// import (#1123).
    CancelBoardImport,
    /// A click on a board counter (Routed, Unrouted, DRC, Selected) or the
    /// Dropdown switcher's layer pill, or a right-click on the layer pill or
    /// the Grid read-out, opens its popover
    /// above it, or closes it when open (#2460). The bar sends
    /// `window: None`; `Signex::status_bar_message` names the window it was
    /// clicked in.
    BoardCounterClicked {
        window: Option<iced::window::Id>,
        counter: crate::status_bar::BoardCounter,
    },
    /// An event of the open board popover; it acts in the popover's window.
    BoardPopover(crate::status_bar::BoardPopoverEvent),
    /// The 30 s tick that ages the DRC counter's "N min ago" while a board
    /// with a finished run is shown.
    BoardClockTicked,
    /// Click on the board view's angle readout opens or closes its value
    /// field (#2316).
    ViewAngleClicked,
    /// The text in the angle field changed.
    ViewAngleEdited(String),
    /// Enter in the angle field.
    ViewAngleSubmitted,
    /// Right-click on a sheet's Grid read-out: opens or closes the Grid &
    /// Snap popover above it (mockup row 6).
    GridReadoutRightClicked,
}
