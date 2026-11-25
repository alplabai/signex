use super::super::*;
use std::path::PathBuf;

use super::footprint_ctx::build_footprint_editor_panel_ctx;
use super::symbol_ctx::build_symbol_editor_panel_ctx;
use crate::library::row_summaries::{FootprintOutcome, RowSummaryRequest, SymbolOutcome};

/// `(lib_symbol_names, placed_symbols)` as held by the panel context.
type SymbolLists = (Vec<String>, Vec<(String, String, String, String)>);

impl Signex {
    /// The library row selected in the active Library Browser tab:
    /// `(library path, table, row)`.
    fn selected_library_row(&self) -> Option<(PathBuf, String, &signex_library::ComponentRow)> {
        let active_tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let library_path = match &active_tab.kind {
            crate::app::TabKind::LibraryBrowser(p) => p.clone(),
            _ => return None,
        };
        let browser = self.library.library_browsers.get(&library_path)?;
        let table = browser.active_table.clone()?;
        let row_id = browser.selected_row?;
        let lib = self.library.library_at(&library_path)?;
        let row = lib
            .tables
            .get(&table)?
            .iter()
            .find(|r| signex_library::RowId::from_uuid(r.row_id) == row_id)?;
        Some((library_path, table, row))
    }

    /// The active Component editor tab's row, for the Graphite Properties dock.
    fn compute_component_sheet(&self) -> Option<crate::panels::ComponentSheet> {
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let editor = tab.kind.as_component_editor()?;
        let lib = self.library.library_at(&editor.library_path)?;
        let row = lib
            .tables
            .get(&editor.table)?
            .iter()
            .find(|r| signex_library::RowId::from_uuid(r.row_id) == editor.row_id)?;
        Some(crate::panels::ComponentSheet {
            internal_pn: row.internal_pn.as_str().to_string(),
            manufacturer: row.primary_mpn.manufacturer.clone(),
            mpn: row.primary_mpn.mpn.clone(),
            lifecycle: crate::library::editor::kit::lifecycle_label(row.state).to_string(),
            version: row.version.clone(),
            library: editor
                .library_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            table: editor.table.clone(),
            updated: row.updated.format("%Y-%m-%d").to_string(),
        })
    }

    /// Record which row's bindings the Properties panel needs summarised;
    /// `start_row_summary_read` fetches them. Never reads the disk.
    pub(super) fn want_row_summaries(&mut self) {
        let request = self
            .selected_library_row()
            .map(|(path, _, row)| RowSummaryRequest::for_row(row, path, self.library.data_version));
        self.library.row_summary_job.want(request);
    }

    /// F15 — When the active tab is a Library Browser AND a row is
    /// selected in that tab's browser state, build the
    /// [`crate::panels::LibraryRowDetail`] the Properties panel
    /// renders. Returns `None` for any other active tab kind, or
    /// when the browser tab has no row selected.
    fn compute_library_row_detail(&self) -> Option<crate::panels::LibraryRowDetail> {
        let (library_path, table, row) = self.selected_library_row()?;
        let summaries = self.library.row_summary_job.summaries_for(row);
        // A read failure and an absent UUID are different diagnoses.
        // Reporting "UUID not in mounted libraries" for a corrupt
        // `.snxsym` or a library server returning 500 sends the user
        // off to re-bind a perfectly good UUID, so the error arm gets
        // its own wording (and a Messages-panel record, once per read).
        use signex_i18n::{ids, t};
        let i18n = &self.ui_state.i18n;
        let symbol_summary = if row.is_symbol_unbound() {
            t!(i18n, ids::LIBRARY_ROW_NO_SYMBOL)
        } else {
            match summaries.and_then(|s| s.symbol.as_ref()) {
                Some(SymbolOutcome::Pins(pins)) => {
                    t!(i18n, ids::LIBRARY_ROW_SYMBOL_PINS, "pins" => *pins)
                }
                Some(SymbolOutcome::Absent) => t!(i18n, ids::LIBRARY_ROW_SYMBOL_ABSENT),
                Some(SymbolOutcome::Failed(e)) => {
                    t!(i18n, ids::LIBRARY_ROW_SYMBOL_FAILED, "error" => e.as_str())
                }
                None => t!(i18n, ids::LIBRARY_ROW_SYMBOL_READING),
            }
        };
        let symbol_summary = match row.alternate_symbols.len() {
            0 => symbol_summary,
            n => t!(
                i18n,
                ids::LIBRARY_ROW_ALTERNATE_SYMBOLS,
                "summary" => symbol_summary.into_string(),
                "count" => n
            ),
        };
        let footprint_summary = match &row.footprint_ref {
            Some(_) if row.is_footprint_unbound() => t!(i18n, ids::LIBRARY_ROW_NO_FOOTPRINT),
            Some(_) => match summaries.and_then(|s| s.footprint.as_ref()) {
                Some(FootprintOutcome::Found) => t!(i18n, ids::LIBRARY_ROW_FOOTPRINT_BOUND),
                Some(FootprintOutcome::Absent) => t!(i18n, ids::LIBRARY_ROW_FOOTPRINT_ABSENT),
                Some(FootprintOutcome::Failed(e)) => {
                    t!(i18n, ids::LIBRARY_ROW_FOOTPRINT_FAILED, "error" => e.as_str())
                }
                None => t!(i18n, ids::LIBRARY_ROW_FOOTPRINT_READING),
            },
            None => t!(i18n, ids::LIBRARY_ROW_NO_FOOTPRINT),
        };

        // An unbound, unresolved or unreadable binding turns its card amber
        // (mockup row 127); one still being read does not.
        let symbol_warn = row.is_symbol_unbound()
            || matches!(
                summaries.and_then(|s| s.symbol.as_ref()),
                Some(SymbolOutcome::Absent | SymbolOutcome::Failed(_))
            );
        let footprint_warn = row.footprint_ref.is_none()
            || row.is_footprint_unbound()
            || matches!(
                summaries.and_then(|s| s.footprint.as_ref()),
                Some(FootprintOutcome::Absent | FootprintOutcome::Failed(_))
            );
        let open = self.library.library_at(&library_path);
        let position = open
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().position(|r| r.row_id == row.row_id))
            .map(|index| index + 1);
        let library = open.map(|lib| lib.display_name.clone()).unwrap_or_default();
        let read_only = self.library.is_read_only(&library_path);
        Some(crate::panels::LibraryRowDetail {
            library_path,
            library,
            table,
            row_id: row.row_id,
            position,
            internal_pn: row.internal_pn.as_str().to_string(),
            class: row.class.as_str().to_string(),
            lifecycle: crate::library::editor::kit::lifecycle_label(row.state).to_string(),
            lifecycle_tone: crate::library::editor::kit::lifecycle_tone(row.state),
            version: row.version.clone(),
            mpn: row.primary_mpn.mpn.clone(),
            manufacturer: row.primary_mpn.manufacturer.clone(),
            updated: row.updated.format("%Y-%m-%d %H:%M").to_string(),
            symbol_warn,
            footprint_warn,
            symbol_summary,
            footprint_summary,
            read_only,
        })
    }

    /// Every path and directory the project tree reads from the disk. Pure:
    /// the paths come from project data, so building it does no IO.
    fn project_fs_request(&self) -> crate::app::state::project_fs::ProjectFsRequest {
        use crate::app::state::project_fs::git_marker;
        let mut request = crate::app::state::project_fs::ProjectFsRequest::default();
        for p in &self.document_state.projects {
            let dir = std::path::Path::new(&p.data.dir);
            let files = p
                .data
                .schematic_root
                .iter()
                .chain(p.data.pcb_file.iter())
                .chain(p.data.sheets.iter().map(|s| &s.filename));
            request.exists.extend(files.map(|f| dir.join(f)));
            if let Some(project_dir) = p.path.parent() {
                request.exists.push(git_marker(project_dir));
            }
            for entry in &p.data.libraries {
                let resolved = p.data.resolve_library_path(entry);
                if let Some(lib_dir) = resolved.parent() {
                    request.exists.push(git_marker(lib_dir));
                }
                request.exists.push(resolved);
            }
        }
        request.hierarchy = self.hierarchy_request();
        request.exists.extend(self.hierarchy_exists_paths());
        request.exists.sort();
        request.exists.dedup();
        request
    }

    /// The Net Classes section's state: the panel's own selection and drafts
    /// carried over, the table and history flags read from the active project.
    fn net_class_panel_state(&self) -> crate::panels::NetClassPanel {
        let document = &self.document_state;
        let mut panel = document.panel_ctx.net_classes.clone();
        let project = document
            .active_project
            .and_then(|id| document.projects.iter().find(|p| p.id == id));
        match project {
            Some(p) => {
                let history = document.net_class_history.get(&p.id);
                if panel.project != Some(p.id) {
                    panel.project = Some(p.id);
                    panel.selected = 0;
                    panel.draft = p
                        .data
                        .net_classes
                        .first()
                        .map(crate::panels::NetClassDraft::from_class)
                        .unwrap_or_default();
                    panel.notice = None;
                }
                panel.available = true;
                panel.classes = p.data.net_classes.clone();
                panel.can_undo = history.is_some_and(|h| h.can_undo());
                panel.can_redo = history.is_some_and(|h| h.can_redo());
                let selected = panel.selected.min(panel.classes.len().saturating_sub(1));
                if selected != panel.selected {
                    panel.selected = selected;
                    panel.draft = panel
                        .classes
                        .get(selected)
                        .map(crate::panels::NetClassDraft::from_class)
                        .unwrap_or_default();
                }
            }
            None => panel = Default::default(),
        }
        panel.label = self.selected_label_class_pick();
        panel
    }

    /// Fill `lib_symbol_names` / `placed_symbols`: the lists kept from the
    /// previous refresh when the engine is unchanged, else rebuilt (#1383).
    pub(in crate::app) fn fill_symbol_lists(&mut self, kept: Option<SymbolLists>) {
        let (names, placed) = match kept {
            Some(lists) => lists,
            None => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    (
                        snapshot.lib_symbols.keys().cloned().collect(),
                        snapshot
                            .symbols
                            .iter()
                            .map(|s| {
                                (
                                    s.reference.clone(),
                                    s.value.clone(),
                                    s.footprint.clone(),
                                    s.lib_id.clone(),
                                )
                            })
                            .collect(),
                    )
                } else {
                    let placed = self
                        .active_pcb_snapshot()
                        .map(|pcb| {
                            pcb.footprints
                                .iter()
                                .map(|f| {
                                    (
                                        f.reference.clone(),
                                        f.value.clone(),
                                        f.footprint_id.clone(),
                                        f.layer.clone(),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (Vec::new(), placed)
                }
            }
        };
        let ctx = &mut self.document_state.panel_ctx;
        ctx.lib_symbol_names = names;
        ctx.placed_symbols = placed;
    }

    /// Who owns the numeric box `key` and the value it stores now, from the
    /// same projection the Properties panel renders; `None` when the box is
    /// not on screen (other selection, other tab, no placement).
    pub(crate) fn numeric_field_state(
        &self,
        key: crate::panels::NumericFieldKey,
    ) -> Option<(crate::panels::NumericOwner, f64)> {
        let active_tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| &tab.path);
        crate::panels::numeric_field_state(
            key,
            active_tab,
            build_symbol_editor_panel_ctx(self).as_ref(),
            self.document_state.panel_ctx.pre_placement.as_ref(),
        )
    }

    /// The rows of the empty Projects panel's Recent projects list: the
    /// Start screen's cap over `UiState::recent_projects`, newest first.
    pub(crate) fn recent_projects_panel_rows(&self) -> Vec<PathBuf> {
        crate::start_screen::visible_recent(self.ui_state.recent_projects.entries()).to_vec()
    }

    /// Rebuild the panel contexts: the main window's, then those of the
    /// undocked windows, each from the sheet its window shows.
    pub(crate) fn refresh_panel_ctx(&mut self) {
        self.refresh_main_panel_ctx();
        self.refresh_window_panel_ctxs(None);
    }

    /// Rebuild the context in the main slot from the active document.
    pub(crate) fn refresh_main_panel_ctx(&mut self) {
        // The clock is read here, in the runtime layer, and kept in state:
        // `${CurrentTime}` shows the time the panel was last built, to the
        // minute, and `view` never reads the clock.
        self.document_state.times.now = Some(chrono::Local::now());
        // The app-wide requests (project probe, row summaries, Navigator
        // assembly, a pending profile update) belong to the main window. A
        // window's refresh runs with its sheet swapped in and must not
        // retarget them at that sheet's project.
        let main_level = self.interaction_state.acting_window.is_none();
        if main_level {
            // Before `polygon_facts` reads the fills (mockup row 178).
            self.fill_polygon_manager_board();
            let context = match self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .map(|tab| &tab.kind)
            {
                Some(crate::app::TabKind::FootprintEditor(_)) => {
                    Some(crate::dock::EditorContext::Footprint)
                }
                Some(crate::app::TabKind::SymbolEditor(_)) => {
                    Some(crate::dock::EditorContext::Symbol)
                }
                Some(crate::app::TabKind::Pcb) => Some(crate::dock::EditorContext::Pcb),
                Some(crate::app::TabKind::ComponentEditor(_)) => {
                    Some(crate::dock::EditorContext::Component)
                }
                _ => None,
            };
            if let Some(context) = context {
                self.document_state.dock.seed_for_editor(context);
            }
        }
        let previous_navigator = std::mem::take(&mut self.document_state.panel_ctx.navigator);
        let navigator = if main_level {
            self.drop_pending_profile_update_for_other_project();
            let fs_request = self.project_fs_request();
            self.document_state.project_fs.want(fs_request);
            self.want_row_summaries();
            let navigator = self.next_navigator_model(previous_navigator);
            self.want_window_hierarchies();
            navigator
        } else {
            previous_navigator
        };
        // Build per-project panel info from every loaded project in the
        // workspace. The `project_name` / `project_file` / `pcb_file` /
        // `sheets` legacy singletons mirror whichever entry is `is_active`
        // — kept around for the few panels that haven't migrated to read
        // `panel_ctx.projects` directly yet.
        let active_id = self.document_state.active_project;
        // Open-state lookup is "is there a tab pointing at this path?"
        // Dirty-state lookup is per document, not per tab, so it persists
        // across tab close: closing a tab keeps the file dirty until the
        // project is saved or the edits are discarded.
        let open_paths: std::collections::HashSet<std::path::PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let dirty_paths = self.document_state.dirty_document_paths();
        // Active-tab path drives the per-row highlight and accent edge in
        // the tree, the "you are here" cue. None when no tabs open.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());
        let marks = super::sheet_tree::RowMarks {
            open: &open_paths,
            dirty: &dirty_paths,
            active: active_path.as_ref(),
        };
        let projects_panel: Vec<crate::panels::ProjectPanelInfo> = self
            .document_state
            .projects
            .iter()
            .map(|p| {
                let project_dir = std::path::Path::new(&p.data.dir);
                let active_path_ref = active_path.as_ref();
                let lookup = |filename: &str| -> (bool, bool, bool) {
                    let abs = project_dir.join(filename);
                    let is_open = open_paths.contains(&abs);
                    let is_dirty = dirty_paths.contains(&abs);
                    let is_active = active_path_ref == Some(&abs);
                    (is_open, is_dirty, is_active)
                };
                let (project_file_open, project_file_dirty, project_file_active) = p
                    .data
                    .schematic_root
                    .as_deref()
                    .map(lookup)
                    .unwrap_or((false, false, false));
                let (pcb_file_open, pcb_file_dirty, pcb_file_active) = p
                    .data
                    .pcb_file
                    .as_deref()
                    .map(lookup)
                    .unwrap_or((false, false, false));

                // F24 — a file the project lists that is neither on disk nor
                // held by an open or unsaved document shows as `(missing)`.
                // Read from the cache, never the disk (#111): the off-thread
                // probe keeps it current.
                let fs_facts = &self.document_state.project_fs.facts;
                let missing =
                    |filename: &str| marks.is_missing(fs_facts, &project_dir.join(filename));
                let project_file_missing = p.data.schematic_root.as_deref().is_some_and(missing);
                let pcb_file_missing = p.data.pcb_file.as_deref().is_some_and(missing);
                // Flatten `Project::libraries` into the panel struct
                // alongside the sheet list. Each entry resolves to an
                // absolute path so the right-click menu can dispatch
                // back to the correct library; the library renders as
                // a single leaf in the project tree, so we don't
                // enumerate the library's primitive files here.
                let mut libraries: Vec<crate::panels::LibraryNodeInfo> = p
                    .data
                    .libraries
                    .iter()
                    .map(|entry| {
                        let resolved = p.data.resolve_library_path(entry);
                        let mounted_lib = self.library.library_at(&resolved);
                        let display_name = match mounted_lib {
                            Some(lib) => lib.display_name.clone(),
                            None => entry
                                .path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(str::to_string)
                                .unwrap_or_else(|| entry.path.display().to_string()),
                        };
                        let missing = fs_facts.is_missing(&resolved);
                        // F30 — list `.snxsym` / `.snxfpt` FILES, not
                        // individual primitives. Each file can hold
                        // hundreds-to-thousands of symbols, so
                        // enumerating cached_symbols would explode the
                        // tree. The user opens a `.snxsym` file to
                        // browse the symbols inside it via the SCH
                        // Library panel, not via the project tree.
                        //
                        // F34 (2026-05-03) — Add New ▸ Symbol Library
                        // now opens a Save-As dialog so the user names
                        // the file at create time and the `.snxsym`
                        // lands on disk immediately. No in-memory
                        // merging needed; the next refresh after the
                        // dialog confirm picks up the new file.
                        let resolved_clone = resolved.clone();
                        let is_open = open_paths.contains(&resolved_clone);
                        let is_dirty = dirty_paths.contains(&resolved_clone);
                        crate::panels::LibraryNodeInfo {
                            display_name,
                            root: resolved,
                            mounted: mounted_lib.is_some(),
                            missing,
                            is_open,
                            is_dirty,
                        }
                    })
                    .collect();

                // Pending (yet-to-be-materialised) libraries appear in
                // the tree as un-mounted nodes with a "(pending)"
                // suffix so the user can see what Save will create.
                // Disk write happens at project-save time via
                // `commands::create_pending_library`.
                for spec in p.pending_libraries.values() {
                    libraries.push(crate::panels::LibraryNodeInfo {
                        display_name: format!("{} (pending)", spec.display_name),
                        root: spec.lib_path.clone(),
                        mounted: false,
                        // Pending entries are intentionally absent on
                        // disk (they materialise at save time) — not
                        // a missing-orphan situation.
                        missing: false,
                        is_open: false,
                        is_dirty: false,
                    });
                }
                crate::panels::ProjectPanelInfo {
                    id: p.id,
                    case_fold: p.case_fold,
                    name: p.data.name.clone(),
                    project_file: p.data.schematic_root.clone(),
                    project_file_open,
                    project_file_dirty,
                    project_file_active,
                    project_file_missing,
                    pcb_file: p.data.pcb_file.clone(),
                    pcb_file_open,
                    pcb_file_dirty,
                    pcb_file_active,
                    pcb_file_missing,
                    sheets: super::sheet_tree::project_sheet_rows(p, fs_facts, &marks),
                    libraries,
                    is_active: Some(p.id) == active_id,
                    is_dirty: dirty_paths.contains(&p.path),
                    version_control: p.data.enable_git,
                }
            })
            .collect();

        // The two symbol lists are O(symbols) string clones; reuse the
        // previous ones while the active engine is unchanged (#1383).
        // Keyed on the render cache the lists are built from, not the engine:
        // the cache can lag an engine edit. Revision 0 means "unknown".
        let symbol_lists_key = self.document_state.active_engine().and_then(|engine| {
            let cache = self
                .interaction_state
                .active_canvas()
                .active_render_cache()?;
            (cache.revision() != 0).then(|| (engine.instance_id(), cache.revision()))
        });
        let reuse_symbol_lists = symbol_lists_key.is_some()
            && symbol_lists_key == self.document_state.panel_ctx.symbol_lists_key;
        let kept_symbol_lists = reuse_symbol_lists.then(|| {
            let ctx = &mut self.document_state.panel_ctx;
            (
                std::mem::take(&mut ctx.lib_symbol_names),
                std::mem::take(&mut ctx.placed_symbols),
            )
        });

        // The tree is a pure function of `projects`; keep the previous one
        // (and its expand/collapse state) while they are unchanged (#1383).
        let (kept_tree, previous_projects, project_tree_builds) = {
            let ctx = &mut self.document_state.panel_ctx;
            (
                std::mem::take(&mut ctx.project_tree),
                std::mem::take(&mut ctx.projects),
                ctx.project_tree_builds,
            )
        };
        let tree_unchanged = previous_projects == projects_panel
            && (project_tree_builds > 0 || projects_panel.is_empty());
        let kept_project_tree = if tree_unchanged { kept_tree } else { vec![] };
        // Taken before the snapshot below borrows `self`.
        let directive_panel = self.document_state.panel_ctx.directive_panel.take();
        let active_schematic_snapshot = self.active_render_snapshot();
        let active_pcb_snapshot = self.active_pcb_snapshot();

        let canvas_font_popup_open = self.document_state.panel_ctx.canvas_font_popup_open;
        let properties_tab = self.document_state.panel_ctx.properties_tab;
        let document_options = self.document_state.panel_ctx.document_options.clone();
        let projects_view = self.document_state.panel_ctx.projects_view.clone();
        let mut pcb_layers_ui = self.document_state.panel_ctx.pcb_layers_ui.clone();
        let pcb_panel = self.document_state.panel_ctx.pcb_panel.clone();
        let selection_count = self.document_state.panel_ctx.selection_count;
        let selected_uuid = self.document_state.panel_ctx.selected_uuid;
        let selected_kind = self.document_state.panel_ctx.selected_kind;
        let selection_info = self.document_state.panel_ctx.selection_info.clone();
        let multi_groups = self.document_state.panel_ctx.multi_groups.clone();
        let placed = self.document_state.panel_ctx.placed.clone();
        let multi_draft = self.document_state.panel_ctx.multi_draft.clone();
        let multi_footprint_choices = self
            .document_state
            .panel_ctx
            .multi_footprint_choices
            .clone();
        let visible_grid_draft = self.document_state.panel_ctx.visible_grid_draft.clone();
        let snap_grid_draft = self.document_state.panel_ctx.snap_grid_draft.clone();
        let symbol_choices = self.document_state.panel_ctx.symbol_choices.clone();
        let footprint_choices = self.document_state.panel_ctx.footprint_choices.clone();
        let footprint_card = self.document_state.panel_ctx.footprint_card.clone();
        let drawing_edit_buf_page =
            active_schematic_snapshot.map(|sheet| (sheet.page.origin, sheet.paper_size.clone()));
        let drawing_edit_buf =
            if self.document_state.panel_ctx.drawing_edit_buf_page == drawing_edit_buf_page {
                self.document_state.panel_ctx.drawing_edit_buf.clone()
            } else {
                Default::default()
            };
        let drawing_edit_buf_for = self.document_state.panel_ctx.drawing_edit_buf_for;
        let selected_drawing = self.document_state.panel_ctx.selected_drawing.clone();
        let selected_drawing_placement = self.document_state.panel_ctx.selected_drawing_placement;
        let annotation = self.annotation_panel_state();
        let selected_child_sheet = self.document_state.panel_ctx.selected_child_sheet.clone();
        let selected_child_sheet_file = self
            .document_state
            .panel_ctx
            .selected_child_sheet_file
            .clone();
        let child_sheet_border_picker_open =
            self.document_state.panel_ctx.child_sheet_border_picker_open;
        let wire_color_picker = self.document_state.panel_ctx.wire_color_picker;
        let selected_wire_style = self.document_state.panel_ctx.selected_wire_style;
        let child_sheet_fill_picker_open =
            self.document_state.panel_ctx.child_sheet_fill_picker_open;
        let child_sheet_border_advanced_open = self
            .document_state
            .panel_ctx
            .child_sheet_border_advanced_open;
        let child_sheet_fill_advanced_open =
            self.document_state.panel_ctx.child_sheet_fill_advanced_open;
        let child_sheet_stroke_width_buf = self
            .document_state
            .panel_ctx
            .child_sheet_stroke_width_buf
            .clone();
        let child_sheet_name_buf = self.document_state.panel_ctx.child_sheet_name_buf.clone();
        let child_sheet_file_buf = self.document_state.panel_ctx.child_sheet_file_buf.clone();
        let title_block: std::collections::BTreeMap<String, String> = active_schematic_snapshot
            .map(|snapshot| {
                snapshot
                    .title_block
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let sheet_variables = active_schematic_snapshot
            .map(|snapshot| snapshot.variables.clone())
            .unwrap_or_default();
        let project_parameters = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|path| self.document_state.document_project(path))
            .map(|project| project.data.parameters.clone())
            .unwrap_or_default();
        // The read-only Parameters rows, resolved by the special strings the
        // canvas and the PDF use.
        let builtin_parameters: Vec<(crate::panels::BuiltinParameter, String)> = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|path| self.special_strings_for_path(path))
            .map(|special| {
                crate::panels::BuiltinParameter::ALL
                    .into_iter()
                    .map(|parameter| (parameter, special.resolve(parameter.token())))
                    .collect()
            })
            .unwrap_or_default();
        // Drafts stay with the sheet they were typed on (shown and committed
        // only there); a closed sheet's drafts are dropped.
        self.document_state
            .drafts
            .retain_sheets(|sheet| self.document_state.engines.contains_key(sheet));
        let drafts = self.document_state.drafts.clone();
        let numeric_draft = self
            .document_state
            .panel_ctx
            .numeric_draft
            .clone()
            .filter(|draft| draft.is_live(self.numeric_field_state(draft.key)));
        let title_block_sheet = self.document_state.active_path.clone();
        let collapsed_sections = self.document_state.panel_ctx.collapsed_sections.clone();
        let part_panel = self.document_state.panel_ctx.part_panel.clone();
        let pre_placement = self.document_state.panel_ctx.pre_placement.clone();
        // A typed hex code lives only while its row is on screen.
        let has_tab_form = pre_placement.is_some();
        let color_hex_draft = (self.document_state.panel_ctx.color_hex_draft.clone())
            .filter(|draft| draft.key.is_shown_for(selected_uuid, has_tab_form));
        let page_format_mode = self.document_state.panel_ctx.page_format_mode;
        let erc_diagnostics = self.build_erc_diagnostic_entries();
        let erc_waivers = self.erc_waiver_entries();
        let erc_violation_waivers = self.erc_violation_waiver_entries();

        let net_classes = self.with_board_net_classes(self.net_class_panel_state());
        let pcb_layers = self.pcb_layers_panel_state();
        let shown_board = self.document_state.panel_ctx.pcb_layers.board.as_deref();
        pcb_layers_ui.follow_board(shown_board, &pcb_layers);

        let (panel_tokens, panel_theme_id) = self.panel_theme();
        self.document_state.panel_ctx = crate::panels::PanelContext {
            projects: projects_panel,
            sym_count: active_schematic_snapshot
                .map(|snapshot| snapshot.symbols.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.footprints.len()))
                .unwrap_or(0),
            wire_count: active_schematic_snapshot
                .map(|snapshot| snapshot.wires.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.segments.len()))
                .unwrap_or(0),
            label_count: active_schematic_snapshot
                .map(|snapshot| snapshot.labels.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.texts.len()))
                .unwrap_or(0),
            junction_count: active_schematic_snapshot
                .map(|snapshot| snapshot.junctions.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.vias.len()))
                .unwrap_or(0),
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            paper_size: active_schematic_snapshot
                .map(|snapshot| crate::app::projector::effective_paper_size(&snapshot.paper_size))
                .or_else(|| {
                    active_pcb_snapshot
                        .map(|snapshot| format!("PCB • {} layers", snapshot.layers.len()))
                })
                .unwrap_or_else(|| "A4".to_string()),
            document_font_family: active_schematic_snapshot
                .and_then(|snapshot| snapshot.font_family.clone()),
            lib_symbol_count: active_schematic_snapshot
                .map(|snapshot| snapshot.lib_symbols.len())
                .unwrap_or(0),
            lib_symbol_names: Vec::new(),
            placed_symbols: Vec::new(),
            symbol_lists_key,
            tokens: panel_tokens,
            theme_id: panel_theme_id,
            i18n: std::sync::Arc::clone(&self.ui_state.i18n),
            canvas_colors: self.canvas_colors_for(panel_theme_id),
            device_px_per_logical_px: self.ui_state.main_device_px_per_logical_px(),
            unit: self.ui_state.unit,
            grid_visible: self.ui_state.grid_visible,
            snap_enabled: self.ui_state.snap_enabled,
            grid_size_mm: self.ui_state.grid_size_mm,
            visible_grid_mm: self.ui_state.visible_grid_mm,
            // Preserve the effective (possibly live-preview) style — a
            // panel_ctx rebuild mid-Preferences must not snap the Symbol
            // Editor grid back to the committed value. Seeded from
            // `ui_state.symbol_grid_style` at boot; every later write
            // goes through the Preferences handler.
            symbol_grid_style: self.document_state.panel_ctx.symbol_grid_style,
            snap_hotspots: self.ui_state.snap_hotspots,
            ui_font_name: self.ui_state.ui_font_name.clone(),
            component_classes: self.ui_state.component_classes.clone(),
            canvas_font_name: self.ui_state.canvas_font_name.clone(),
            canvas_font_size: self.ui_state.canvas_font_size,
            canvas_font_bold: self.ui_state.canvas_font_bold,
            canvas_font_italic: self.ui_state.canvas_font_italic,
            canvas_font_popup_open,
            properties_tab,
            document_options,
            projects_view,
            recent_projects: self.recent_projects_panel_rows(),
            net_classes,
            pcb_layers,
            pcb_layers_ui,
            project_tree: kept_project_tree,
            project_tree_builds,
            project_tree_selected: self.document_state.panel_ctx.project_tree_selected.clone(),
            navigator,
            library_row_detail: self.compute_library_row_detail(),
            component_sheet: self.compute_component_sheet(),
            pcb_nets: self.pcb_nets_summary(),
            pcb_panel,
            // Kept in step with the net index by `refresh_net_nav`.
            net_inspector: self.document_state.panel_ctx.net_inspector.clone(),
            // Kept: `sync_unions_panel` and `refresh_harness_panel` own these.
            unions: self.document_state.panel_ctx.unions.clone(),
            harness: self.document_state.panel_ctx.harness.clone(),
            compiled_design: Box::new(self.compiled_design_view()),
            // Kept in step with the documents by `sync_object_list`.
            object_list: self.document_state.panel_ctx.object_list.clone(),
            // Replaced by `sync_command_log_panel_ctx` after each change.
            command_log: std::sync::Arc::clone(&self.document_state.panel_ctx.command_log),
            // Kept in step with the scan and filters by `handlers/annotations.rs`.
            annotations: std::sync::Arc::clone(&self.document_state.panel_ctx.annotations),
            pcb_board: self.pcb_board_summary().map(Box::new),
            layer_stack: self.layer_stack_facts(),
            polygons: self.polygon_facts(),
            selection_count,
            selected_uuid,
            selected_kind,
            selection_info,
            multi_groups,
            placed,
            multi_draft,
            multi_footprint_choices,
            visible_grid_draft,
            snap_grid_draft,
            symbol_choices,
            footprint_choices,
            footprint_card,
            drawing_edit_buf,
            drawing_edit_buf_for,
            drawing_edit_buf_page,
            selected_drawing,
            selected_drawing_placement,
            annotation,
            selected_child_sheet,
            selected_child_sheet_file,
            directive_panel,
            child_sheet_border_picker_open,
            child_sheet_fill_picker_open,
            child_sheet_border_advanced_open,
            child_sheet_fill_advanced_open,
            wire_color_picker,
            selected_wire_style,
            child_sheet_stroke_width_buf,
            child_sheet_name_buf,
            child_sheet_file_buf,
            title_block,
            project_parameters,
            sheet_variables,
            builtin_parameters,
            drafts,
            numeric_draft,
            color_hex_draft,
            title_block_sheet,
            collapsed_sections,
            part_panel,
            pre_placement,
            erc_diagnostics,
            erc_waivers,
            erc_violation_waivers,
            erc_waive_draft: self.ui_state.erc_waive_draft.clone(),
            erc_focus_index: self.ui_state.erc_focus_global_index,
            erc_running: self.ui_state.erc_running,
            erc_results_stale: self.erc_results_stale(),
            erc_ran: !self.ui_state.erc_violations_by_path.is_empty(),
            validation: self.validation_panels_view(),
            diagnostics_level: crate::diagnostics::configured_level_label().to_string(),
            diagnostics: crate::diagnostics::recent_entries(),
            pcb_drc: self.pcb_drc_panel(),
            selection_filters: self.interaction_state.selection_filters.clone(),
            custom_filter_presets: self.interaction_state.custom_filter_presets.clone(),
            active_custom_filter_tab: self.interaction_state.active_custom_filter_tab,
            footprint_filter_presets: self.interaction_state.footprint_filter_presets.clone(),
            page_format_mode,
            sheet_frame: active_schematic_snapshot
                .map(|snapshot| snapshot.frame)
                .unwrap_or_default(),
            page_origin: active_schematic_snapshot
                .map(|snapshot| snapshot.page.origin)
                .unwrap_or_default(),
            sheet_color: crate::panels::SchematicSheetColor::of(
                active_schematic_snapshot.and_then(|snapshot| snapshot.page.color),
            ),
            symbol_editor: build_symbol_editor_panel_ctx(self),
            footprint_editor: build_footprint_editor_panel_ctx(self).map(Box::new),
            history: self.document_state.history.clone(),
            selection_filter: self.selection_filter_facts(),
            illustration_hover: self.document_state.panel_ctx.illustration_hover.clone(),
            selection_form: crate::panels::SelectionForm {
                kinds: crate::panels::SelectionKinds::of(
                    &self.interaction_state.active_canvas().selected,
                ),
                ..self.document_state.panel_ctx.selection_form.clone()
            },
        };
        self.fill_symbol_lists(kept_symbol_lists);
        if !tree_unchanged {
            let ctx = &mut self.document_state.panel_ctx;
            ctx.project_tree = crate::panels::build_project_tree(ctx);
            ctx.project_tree_builds = ctx.project_tree_builds.wrapping_add(1);
        }
        // Sheet N of M and the project's title block can change with it.
        self.refresh_canvas_sheet_frame();
        self.refresh_canvas_designators();
        // The active variant, its settings or the sheet may have changed.
        self.refresh_canvas_not_fitted();
    }
}

#[cfg(test)]
#[path = "panel_ctx_tests.rs"]
mod tests;
