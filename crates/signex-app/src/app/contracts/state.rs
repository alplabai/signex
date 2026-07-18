//! Context actions + modal/dialog state structs.


/// R / G / B channel selector for the custom net-colour picker inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ContextAction {
    Copy,
    Cut,
    Paste,
    SmartPaste,
    OpenChildSheet,
    Delete,
    SelectAll,
    ZoomFit,
    RotateSelected,
    MirrorX,
    MirrorY,
    /// Run an Active Bar action from a context-menu submenu (Place /
    /// Align). Closes both menus and dispatches the action through the
    /// existing Active Bar handler so all the placement / transform
    /// logic stays in one place.
    ActiveBar(crate::active_bar::ActiveBarAction),
}

/// Which click-to-open submenu is currently expanded inside the right-
/// click context menu, if any. Owned by `InteractionState` and cleared
/// alongside `context_menu` whenever the menu closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSubmenu {
    Place,
    Align,
    /// Project-tree → "Add New to Project ›" launcher. Items are
    /// version-tagged placeholders today — actual document creation
    /// lands with project-write support in v0.9.
    AddNewToProject,
}

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
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
}

/// v0.18.10 — Altium-style grid picker popup state. Anchors the
/// floating menu at the cursor position when `G` is pressed.
#[derive(Debug, Clone)]
pub struct GridPickerState {
    pub x: f32,
    pub y: f32,
}

/// v0.18.11 — Cartesian Grid Editor modal state. Carries the
/// in-flight Step X / Step Y string buffers + the X/Y link toggle.
/// Writes happen on `GridPropertiesApply`; close discards.
///
/// v0.18.19 added Fine / Coarse display + Multiplier draft fields.
#[derive(Debug, Clone)]
pub struct GridPropertiesState {
    pub step_x_mm: String,
    pub step_y_mm: String,
    pub link_xy: bool,
    pub fine_display: crate::library::editor::footprint::state::GridDisplay,
    pub coarse_display: crate::library::editor::footprint::state::GridDisplay,
    pub multiplier: u32,
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
}

/// State for the Projects-panel tree-view right-click menu. The menu's
/// action set is computed from `path` (leaf vs branch vs empty) at render
/// time, so we only need to store the anchor coordinates + the clicked
/// path (or `None` for the background menu).
#[derive(Debug, Clone)]
pub struct ProjectTreeContextMenuState {
    pub x: f32,
    pub y: f32,
    /// `Some(path)` = right-click on a specific node; `None` = right-click
    /// in empty tree area, offering only the generic actions.
    pub path: Option<Vec<usize>>,
}

/// State for the "Close Project — Unsaved Edits" confirmation modal.
/// Opens only when the user closes a project that has at least one
/// entry in `DocumentState.dirty_paths` rooted in the project's
/// directory; the modal lists every dirty file by filename so the
/// user can see what they're about to lose.
#[derive(Debug, Clone)]
pub struct ProjectCloseConfirmState {
    /// Project root tree path the close was requested for. Stored so
    /// the modal's confirm action can dispatch back to
    /// `close_project_at_tree_path` without re-resolving from the
    /// project list (which may shift if the user closes another
    /// project while this modal is up — Altium's modal is dismiss-
    /// only, so this is defence-in-depth).
    pub tree_path: Vec<usize>,
    /// Project display name shown in the modal header.
    pub project_name: String,
    /// Absolute paths of dirty files inside the project's directory.
    /// The view layer renders the file basenames; the handler uses
    /// the absolute paths to locate the engines for save / discard.
    pub dirty_paths: Vec<std::path::PathBuf>,
}

/// State for the "Exit Signex — Unsaved Edits" confirmation modal.
/// Opens when the user requests app exit (chrome ✕, File ▸ Exit,
/// Alt+F4) while `DocumentState.dirty_paths` is non-empty. Lists
/// every dirty file across the whole workspace so the user sees what
/// they are about to lose before choosing Save All / Discard All /
/// Cancel. Reuses `ProjectCloseChoice` for the three outcomes.
#[derive(Debug, Clone)]
pub struct AppQuitConfirmState {
    /// Absolute paths of every dirty file in the workspace, sorted
    /// for a stable display order. The view renders the basenames.
    pub dirty_paths: Vec<std::path::PathBuf>,
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
}

/// Concrete actions dispatched when the user picks a menu item in the
/// Projects-panel tree-view context menu.
#[derive(Debug, Clone)]
pub enum ProjectTreeAction {
    /// Open the file backed by this leaf in the current document slot.
    OpenNode(Vec<usize>),
    /// Expand (or collapse) a specific branch node.
    ToggleNode(Vec<usize>),
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
    RevealInExplorer(Vec<usize>),
    /// Fire the print preview flow — only surfaced on leaves that are
    /// already the active tab.
    PrintActive,
    /// Open the sheet-rename modal for this leaf, preloaded with the
    /// current filename.
    OpenRenameDialog(Vec<usize>),
    /// Open the "Remove from Project" modal (Delete / Exclude / Cancel)
    /// for this leaf.
    OpenRemoveDialog(Vec<usize>),
    /// Close the entire project whose root is at this tree path. Closes
    /// every open tab backed by the project, drops the `LoadedProject`
    /// from the workspace, and promotes another project (or `None`) to
    /// active. The tree path's first index selects the project; other
    /// indices are ignored so the action is safe to fire from any node
    /// underneath a project root.
    CloseProject(Vec<usize>),
    /// v0.9 project-root: run ERC across the project. Promotes the
    /// project to active, opens its `schematic_root` if no schematic
    /// from this project is currently active, then dispatches the
    /// existing ERC dialog.
    ValidateProject(Vec<usize>),
    /// v0.9 project-root: open the rename modal seeded with the
    /// project name (the `.snxprj` stem). Submit renames the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` in lockstep.
    OpenProjectRenameDialog(Vec<usize>),
    /// v0.9 project-root: open the Project Options metadata modal.
    OpenProjectOptions(Vec<usize>),
    /// v0.9 project-root: open a file dialog and add the picked file
    /// to the project. Files outside the project directory are copied
    /// in; files already inside just trigger a tree refresh.
    AddExistingToProject(Vec<usize>),
    /// v0.9 project-root → Add New ▸ Schematic. Spawns a Save-As
    /// dialog scoped to the project directory; the result writes a
    /// blank `.snxsch`, registers it as a SheetEntry, marks the
    /// project dirty, and refreshes the tree (no tab opens).
    AddNewSchematic(Vec<usize>),
    /// project-root → Add New ▸ Symbol Library. Save-As dialog
    /// scoped to the project dir, writes an empty `.snxsym`, opens
    /// the file as a primitive editor tab. Altium parity: Schematic
    /// Library is a top-level project document.
    AddProjectSymbolLibrary(Vec<usize>),
    /// project-root → Add New ▸ PCB Library. Save-As dialog
    /// scoped to the project dir, writes an empty `.snxfpt`, opens
    /// the file as a primitive editor tab.
    AddProjectFootprintLibrary(Vec<usize>),
    /// v0.11 project-root: open the Enable Version Control confirm
    /// modal. Runs `git init` at the project dir, optionally seeds
    /// `.gitattributes` for binary-model LFS, and creates the
    /// initial commit covering the entire project tree. Only
    /// enabled when the project dir has no `.git/` already.
    OpenEnableVersionControl(Vec<usize>),
    /// Right-click on a plain-files `.snxlib` node → opens the
    /// Enable Version Control modal scoped to that library directory.
    OpenLibraryEnableVersionControl(Vec<usize>),
}

/// State for the rename modal. Tracks the target file, the live
/// edit buffer, and the clicked tree path so we can rebuild the tree
/// after a successful rename without rediscovering the project.
#[derive(Debug, Clone)]
pub struct RenameDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    pub buffer: String,
    pub error: Option<String>,
    /// `true` when this rename targets the project root — the submit
    /// handler renames the `<old>.snxprj` plus the companion
    /// `<old>.snxsch` / `<old>.snxpcb` (whichever exist on disk) so the
    /// trio stays grouped under the new project name. `false` is the
    /// per-file rename used by sheet leaves.
    pub is_project_rename: bool,
}

/// One file/directory entry surfaced on the Enable Version Control
/// picker. The user can opt items out of the initial commit by
/// untoggling `tracked`; untracked entries get written into a
/// generated `.gitignore` so they sit outside the repo from day one.
#[derive(Debug, Clone)]
pub struct TrackItem {
    pub absolute: std::path::PathBuf,
    pub relative: String,
    /// Short kind badge ("Schematic", "PCB", "Library", "Folder",
    /// "Config", etc.) shown next to the path in the picker.
    pub label: String,
    /// True for directory entries — drives trailing-slash in the
    /// generated `.gitignore` pattern.
    pub is_directory: bool,
    pub tracked: bool,
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
    /// Pre-formatted intro paragraph that interpolates the working
    /// tree path. Computed once at modal-open time so the view
    /// doesn't allocate a fresh `String` on every render frame.
    pub intro_text: String,
    /// Last error from a confirm attempt — surfaces inline so the
    /// user can fix the cause (LFS not installed, etc.) and retry
    /// without reopening the modal.
    pub error: Option<String>,
}

/// State for the read-only "Project Options" modal — the v0.9 surface
/// is a metadata summary (name / dir / schematic root / pcb file /
/// libraries). Editing happens through the dedicated rename / library
/// flows; a future revision can promote this to a full editor.
#[derive(Debug, Clone)]
pub struct ProjectOptionsState {
    pub project_idx: usize,
    pub name: String,
    pub directory: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    pub library_count: usize,
}

/// State for the "Remove from Project" modal. `Delete` removes the file
/// from disk; `Exclude` drops it from the session's sheet list but
/// leaves the file in place.
#[derive(Debug, Clone)]
pub struct RemoveDialogState {
    pub target_path: std::path::PathBuf,
    pub tree_path: Vec<usize>,
    pub display_name: String,
}

/// User choice from the Remove-from-Project modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveChoice {
    /// Remove from project AND delete the file on disk.
    DeleteFile,
    /// Remove from project; leave the file in its folder.
    ExcludeFromProject,
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
}
