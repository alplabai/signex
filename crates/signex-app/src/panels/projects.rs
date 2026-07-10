//! Projects and Navigator panel views plus the project-tree model.

use super::*;
use iced::widget::column;

/// Per-sheet info for the project tree.
#[derive(Debug, Clone)]
pub struct SheetInfo {
    #[allow(dead_code)]
    pub name: String,
    pub filename: String,
    pub sym_count: usize,
    #[allow(dead_code)]
    pub wire_count: usize,
    #[allow(dead_code)]
    pub label_count: usize,
    /// True when this sheet is currently in `document_state.tabs`.
    /// Drives the small accent-coloured dot on the tree row (Altium parity).
    pub is_open: bool,
    /// True when the open tab for this sheet has unsaved edits.
    /// Drives the bright red dot on the tree row.
    pub is_dirty: bool,
    /// True when this sheet is the document the user is currently
    /// viewing (`document_state.tabs[active_tab].path == sheet path`).
    /// Drives the highlighted row background — Altium parity.
    pub is_active: bool,
    /// F24 (2026-05-03) — `true` when the file backing this entry
    /// is registered on the project but no longer exists on disk
    /// (orphan reference, e.g. user moved/deleted outside Signex).
    /// Drives the `(missing)` suffix in `build_project_tree` so the
    /// user sees the broken state at a glance instead of having to
    /// double-click and read an error.
    pub missing: bool,
}

/// Per-project bundle surfaced to the Projects panel. One entry per
/// `LoadedProject` in `DocumentState.projects`. `build_project_tree`
/// iterates this list to emit one tree root per project.
#[derive(Debug, Clone)]
pub struct ProjectPanelInfo {
    pub id: crate::app::ProjectId,
    /// Display name (project stem — "MyBoard" from "MyBoard.standard_pro").
    pub name: String,
    /// Root schematic filename shown as the "project file" under each
    /// root, when present.
    pub project_file: Option<String>,
    /// Open / dirty state for the root schematic, mirrors the same
    /// flags that `SheetInfo` carries for inner sheets.
    pub project_file_open: bool,
    pub project_file_dirty: bool,
    pub project_file_active: bool,
    /// F24 — same `(missing)` indicator as on `SheetInfo` but for the
    /// project's root schematic.
    pub project_file_missing: bool,
    /// Companion PCB filename, when present.
    pub pcb_file: Option<String>,
    pub pcb_file_open: bool,
    pub pcb_file_dirty: bool,
    pub pcb_file_active: bool,
    /// F24 — `(missing)` indicator for the companion PCB file.
    pub pcb_file_missing: bool,
    pub sheets: Vec<SheetInfo>,
    /// Component libraries attached to this project. One entry per
    /// `Project::libraries[]`. Drives the `Libraries` branch under
    /// the project root — each entry renders as a `*.snxlib` leaf
    /// and (when the library is mounted) a small list of cached
    /// components beneath it.
    pub libraries: Vec<LibraryNodeInfo>,
    /// Whether this is the currently-active project — drives accent
    /// styling on the root node.
    pub is_active: bool,
    /// `.snxprj` dirty state — flips when in-memory project metadata
    /// (sheet list / pcb / libraries) has changed and not yet been
    /// written via `write_project`. Drives the red dirty indicator
    /// on the project root row so the user knows Save is pending.
    pub is_dirty: bool,
}

/// Per-library bundle for the project tree's `Libraries` group.
/// Mirrors what [`signex_types::project::LibraryEntry`] records on
/// the project, plus a couple of cached fields the panel pulls from
/// `LibraryState` so the view doesn't have to re-borrow the library
/// crate at render time.
///
/// The library renders as a single leaf in the project tree under
/// the v0.9 `.snxlib`-as-file model — symbols / footprints / sims
/// are not surfaced here. Browsing the library's contents is the
/// Library Browser tab's job; double-clicking the leaf opens it.
#[derive(Debug, Clone)]
pub struct LibraryNodeInfo {
    /// Display name for the row — `<entry.path>.file_name()` or the
    /// manifest name when the library is mounted.
    pub display_name: String,
    /// Absolute on-disk path of the `.snxlib` file — feeds the
    /// right-click menu (Add New ▸ Component pre-selects this path)
    /// and the double-click → Library Browser open dispatch.
    pub root: std::path::PathBuf,
    /// True when the library is currently mounted in
    /// `LibraryState::open_libraries`. Drives the icon tint —
    /// unmounted entries render in the muted "missing" colour.
    pub mounted: bool,
    /// F24 — `true` when the `.snxlib` file is registered on the
    /// project but no longer exists on disk. Drives the `(missing)`
    /// suffix in the tree so the user spots orphan references
    /// without double-clicking through to a "Library not mounted"
    /// recovery message.
    pub missing: bool,
    /// F29 — names of `.snxsym` files inside this library. Populated
    /// from `OpenLibrary::cached_symbols` when the library is
    /// mounted. Empty when the library is unmounted or has no
    /// symbols yet. Used by `build_project_tree` to surface a
    /// `Symbols` subbranch under the library node so the user can
    /// navigate to a specific symbol file directly from the tree.
    pub symbols: Vec<String>,
    /// F29 — same as `symbols` but for `.snxfpt` footprints. Used to
    /// build the `Footprints` subbranch under the library node.
    pub footprints: Vec<String>,
    /// v0.13 — `true` when this `.snxlib` is currently open as a tab
    /// (Library Browser). Drives the white open-dot indicator in the
    /// project tree, matching schematic / pcb sheet open status.
    pub is_open: bool,
    /// v0.13 — `true` when the `.snxlib` has unsaved changes. Drives
    /// the red dirty-dot indicator in the project tree.
    pub is_dirty: bool,
}

// ─── Projects Panel (TreeView) ────────────────────────────────

/// Build the project tree from panel context data. Produces one root
/// per loaded project so multi-project workspaces show all their
/// projects side by side. Single-project users see the same shape as
/// before (one root). `PanelContext::projects` is the source of truth;
/// the legacy `project_name` / `sheets` singletons are ignored here so
/// we never emit a duplicate root for the active project.
pub fn build_project_tree(ctx: &PanelContext) -> Vec<TreeNode> {
    if ctx.projects.is_empty() {
        return vec![];
    }

    ctx.projects.iter().map(project_root_node).collect()
}

/// One project root — "Source Documents" / "Libraries" / "Settings".
/// The Libraries branch lists this project's mounted `*.snxlib`
/// entries (right-click → Add New ▸ Component Library to add one);
/// it renders empty when the project has no libraries rather than
/// inheriting a workspace-wide symbol count.
fn project_root_node(project: &ProjectPanelInfo) -> TreeNode {
    let mut source_docs: Vec<TreeNode> = Vec::new();

    // F24 — surface a `(missing)` suffix on every leaf whose backing
    // file is registered on the project but absent from disk. Catches
    // orphan references (e.g. user moved/deleted a file outside Signex,
    // or a previous library-create attempt left an entry behind without
    // the file). User sees the broken state at a glance instead of
    // having to double-click and read an error.
    fn missing_label(filename: &str, is_missing: bool) -> String {
        if is_missing {
            format!("{filename}  (missing)")
        } else {
            filename.to_string()
        }
    }

    if !project.sheets.is_empty() {
        for sheet in &project.sheets {
            let icon = TreeIcon::for_path(&sheet.filename);
            source_docs.push(
                TreeNode::leaf(missing_label(&sheet.filename, sheet.missing), icon)
                    .with_open(sheet.is_open)
                    .with_dirty(sheet.is_dirty)
                    .with_active(sheet.is_active),
            );
        }
    } else if let Some(file) = &project.project_file {
        let icon = TreeIcon::for_path(file);
        source_docs.push(
            TreeNode::leaf(missing_label(file, project.project_file_missing), icon)
                .with_open(project.project_file_open)
                .with_dirty(project.project_file_dirty)
                .with_active(project.project_file_active),
        );
    }

    if let Some(pcb) = &project.pcb_file {
        let icon = TreeIcon::for_path(pcb);
        source_docs.push(
            TreeNode::leaf(missing_label(pcb, project.pcb_file_missing), icon)
                .with_open(project.pcb_file_open)
                .with_dirty(project.pcb_file_dirty)
                .with_active(project.pcb_file_active),
        );
    }

    // Render each `Project::libraries[i]` entry as a single `.snxlib`
    // leaf — no children, no chevron. Under the v0.9 `.snxlib`-as-file
    // model a library is one thing the user opens, not a folder they
    // browse. Symbols / footprints / sims are siblings on disk, but
    // surfacing them in the project tree confuses the mental model:
    // the user's contract is the `.snxlib` file, not its private
    // working directory. Double-click opens the Library Browser tab,
    // which is the proper surface for browsing the library's contents.
    //
    // When the project carries no library entries the Libraries branch
    // renders empty (matching Settings) — the user can right-click →
    // Add New ▸ Component Library to create one. We deliberately do
    // NOT mint a synthetic "N symbols loaded" child from the
    // workspace-wide library count: that pre-DBLib placeholder
    // advertised symbols the project hadn't actually mounted, which
    // caused real confusion (a project with nothing saved would show
    // "222 symbols loaded" sourced from globally-mounted libraries).
    // F29 — when a library is mounted and exposes symbols /
    // footprints, render a `Symbols` and `Footprints` subbranch
    // underneath the `.snxlib` node so the user can navigate to a
    // specific primitive directly from the tree. Unmounted /
    // missing / empty libraries collapse to a plain leaf (matches
    // the previous behaviour).
    // Libraries branch: every entry renders as a single leaf with the
    // file's full filename (incl. extension). `.snxlib` (Component
    // Libraries), `.snxsym` (Symbol Libraries), and `.snxfpt` (PCB
    // Libraries) all live as siblings under this branch — Altium
    // parity. No nested Symbols / Footprints subbranches: a `.snxlib`
    // can hold thousands of primitives and surfacing them in the tree
    // would explode the panel; opening the `.snxlib` shows the
    // browser instead.
    let lib_children: Vec<TreeNode> = project
        .libraries
        .iter()
        .map(|lib| {
            let filename = lib
                .root
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}.snxlib", lib.display_name));
            let display = if lib.missing {
                format!("{filename}  (missing)")
            } else {
                filename.clone()
            };
            let icon = TreeIcon::for_path(&filename);
            // v0.13 — surface the white open-dot / red dirty-dot
            // indicators on `.snxlib` leaves so library files match
            // the visual rhythm of `.snxsch` / `.snxpcb` sheets.
            TreeNode::leaf(display, icon)
                .with_open(lib.is_open)
                .with_dirty(lib.is_dirty)
        })
        .collect();

    // Settings holds nothing today — gated until a project actually
    // carries per-project preferences. Showing an empty branch reads
    // as "this project has settings hidden behind a toggle"; the
    // honest UI is to omit the heading.
    let settings_children: Vec<TreeNode> = Vec::new();

    // Build the project's child list, skipping any heading whose
    // children list is empty so the tree never renders a bare
    // "(empty)" placeholder under Libraries / Settings.
    let mut children: Vec<TreeNode> = Vec::new();
    if !source_docs.is_empty() {
        children.push(TreeNode::branch(
            "Source Documents".to_string(),
            TreeIcon::Folder,
            source_docs,
        ));
    }
    if !lib_children.is_empty() {
        children.push(TreeNode::branch(
            "Libraries".to_string(),
            TreeIcon::Library,
            lib_children,
        ));
    }
    if !settings_children.is_empty() {
        let mut settings =
            TreeNode::branch("Settings".to_string(), TreeIcon::File, settings_children);
        settings.expanded = false;
        children.push(settings);
    }

    TreeNode::branch(project.name.clone(), TreeIcon::Folder, children)
        .with_accent(project.is_active)
        .with_dirty(project.is_dirty)
}

pub fn view_projects<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    if ctx.project_tree.is_empty() {
        let muted = theme_ext::text_secondary(&ctx.tokens);
        column![
            text("No project open").size(11).color(muted),
            text("").size(4),
            text("File > Open to begin").size(10).color(muted),
        ]
        .spacing(2)
        .padding(6)
        .width(Length::Fill)
        .into()
    } else {
        // Render the persistent tree — toggle state is preserved.
        // Wrap in a container with a small top inset so the tree's
        // first row doesn't sit flush against the panel's tab-strip
        // border (matches the breathing room Altium leaves below its
        // panel tabs).
        container({
            let mut tree = TreeView::new(&ctx.project_tree, &ctx.tokens);
            if let Some(sel) = ctx.project_tree_selected.as_deref() {
                tree = tree.selected(sel);
            }
            tree.view().map(PanelMsg::Tree)
        })
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
    }
}

// ─── Navigator Panel ──────────────────────────────────────────

pub fn view_navigator<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(2).padding(6).width(Length::Fill);
    col = col.push(section_title("Sheets", &ctx.tokens));
    col = col.push(separator(&ctx.tokens));

    // Resolve the active project from the multi-project Vec — replaces
    // the legacy `ctx.project_name` singleton (#54 phase 2.5).
    let active_project = ctx.projects.iter().find(|p| p.is_active);
    if let Some(project) = active_project {
        let mut sheets = vec![];
        for cs in &ctx.child_sheets {
            sheets.push(TreeNode::leaf(cs.clone(), TreeIcon::Sheet));
        }
        let roots = vec![TreeNode::branch(
            project.name.clone(),
            TreeIcon::Schematic,
            sheets,
        )];
        col = col.push(
            TreeView::new(&roots, &ctx.tokens)
                .view()
                .map(PanelMsg::Tree),
        );
    } else {
        col = col.push(
            text("No project")
                .size(10)
                .color(theme_ext::text_secondary(&ctx.tokens)),
        );
    }
    container(col).width(Length::Fill).into()
}

