//! Right-click / dropdown context-menu builders for the canvas, project
//! tree, and tab strip, plus the shared context-menu item helpers.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;

impl Signex {
    /// v0.18.10 — Altium-style grid picker popup body. Renders the
    /// standard 1mil…2.5mm ladder; clicking a row sends
    /// `Message::Ui(UiMsg::GridPickerSelect(step_mm))` and closes the popup.
    pub(super) fn view_grid_picker_menu(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let primary = signex_widgets::theme_ext::text_primary(tokens);
        let muted = signex_widgets::theme_ext::text_secondary(tokens);
        let panel_bg = signex_widgets::theme_ext::to_color(&tokens.panel_bg);
        let border_c = signex_widgets::theme_ext::border_color(tokens);
        let active_step = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => {
                    self.document_state.footprint_editors.get(p)
                }
                _ => None,
            })
            .map(|e| e.state.snap_options.grid_step_mm);

        // Altium-standard ladder. Mil entries first (imperial designs
        // anchor on 50mil), metric second.
        const LADDER: &[(&str, f64)] = &[
            ("1 Mil", 0.0254),
            ("5 Mil", 0.127),
            ("10 Mil", 0.254),
            ("20 Mil", 0.508),
            ("25 Mil", 0.635),
            ("50 Mil", 1.27),
            ("100 Mil", 2.54),
            ("0.025 mm", 0.025),
            ("0.100 mm", 0.100),
            ("0.250 mm", 0.250),
            ("0.500 mm", 0.500),
            ("1.000 mm", 1.000),
            ("2.500 mm", 2.500),
        ];

        let mut col = column![].spacing(0).width(iced::Length::Fixed(200.0));
        for (label, step_mm) in LADDER {
            let is_active = active_step
                .map(|s| (s - step_mm).abs() < 1e-9)
                .unwrap_or(false);
            let lbl_color = if is_active { primary } else { muted };
            let row_label = *label;
            let row_step = *step_mm;
            let btn = button(
                container(text(row_label).size(11).color(lbl_color))
                    .padding([4, 12])
                    .width(iced::Length::Fill),
            )
            .padding(0)
            .on_press(Message::Ui(UiMsg::GridPickerSelect(row_step)))
            .style(move |_: &iced::Theme, status| iced::widget::button::Style {
                background: match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                    )),
                    _ => Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                },
                border: iced::Border {
                    width: 0.0,
                    radius: 0.0.into(),
                    color: iced::Color::TRANSPARENT,
                },
                ..iced::widget::button::Style::default()
            })
            .width(iced::Length::Fill);
            col = col.push(btn);
        }

        container(col)
            .padding(4)
            .style(move |_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(panel_bg)),
                border: iced::Border {
                    width: 1.0,
                    radius: 4.0.into(),
                    color: border_c,
                },
                ..iced::widget::container::Style::default()
            })
            .into()
    }

    #[allow(clippy::vec_init_then_push)]
    pub(super) fn view_context_menu(&self) -> Element<'_, Message> {
        use crate::icons as ic;
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(20);
        let canvas = self.interaction_state.active_canvas();
        let panel_ctx = &self.document_state.panel_ctx;
        let tid = self.ui_state.theme_id;
        // Shortcut hints read from the active keymap profile, falling
        // back to the historic Altium defaults when the profile leaves a
        // command unbound.
        let find_shortcut = self.keymap_shortcut_label("find", "Ctrl+F");
        let cut_shortcut = self.keymap_shortcut_label("cut", "Ctrl+X");
        let copy_shortcut = self.keymap_shortcut_label("copy", "Ctrl+C");
        let paste_shortcut = self.keymap_shortcut_label("paste", "Ctrl+V");
        let smart_paste_shortcut = self.keymap_shortcut_label("smart_paste", "Shift+Ctrl+V");

        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_find_similar(tid)),
            "Find Similar Objects...",
            None,
        ));
        items.push(self.ctx_menu_item_msg(
            Some(ic::icon_chrome_search(tid)),
            "Find Text...",
            &find_shortcut,
            Message::Overlay(OverlayMsg::OpenFind),
        ));
        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_clear_filter(tid)),
            "Clear Filter",
            Some("Shift+C"),
        ));
        items.push(self.ctx_menu_sep());
        let active_submenu = self.interaction_state.context_submenu;
        items.push(self.ctx_menu_item_submenu(
            Some(ic::icon_dd_place_menu(tid)),
            "Place",
            ContextSubmenu::Place,
            active_submenu == Some(ContextSubmenu::Place),
        ));
        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_part_actions(tid)),
            "Part Actions",
            Some(SUBMENU_ARROW),
        ));
        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_sheet_actions(tid)),
            "Sheet Actions",
            Some(SUBMENU_ARROW),
        ));

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled(
                Some(ic::icon_dd_references(tid)),
                "References",
                Some(SUBMENU_ARROW),
            ));
            items.push(self.ctx_menu_item_submenu(
                Some(ic::icon_dd_align_menu(tid)),
                "Align",
                ContextSubmenu::Align,
                active_submenu == Some(ContextSubmenu::Align),
            ));
            items.push(self.ctx_menu_item_disabled(
                Some(ic::icon_dd_unions(tid)),
                "Unions",
                Some(SUBMENU_ARROW),
            ));
            items.push(self.ctx_menu_item_disabled(
                Some(ic::icon_dd_snippets(tid)),
                "Snippets",
                Some(SUBMENU_ARROW),
            ));
        }

        let child_sheet_selected = canvas
            .selected
            .iter()
            .any(|item| item.kind == signex_types::schematic::SelectedKind::ChildSheet);
        if child_sheet_selected {
            items.push(self.ctx_menu_item_kb(
                Some(ic::icon_dd_open_child_sheet(tid)),
                "Open Child Sheet",
                "Enter",
                ContextAction::OpenChildSheet,
            ));
            items.push(self.ctx_menu_sep());
        }

        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_cross_probe(tid)),
            "Cross Probe",
            None,
        ));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_cut(tid)),
            "Cut",
            &cut_shortcut,
            ContextAction::Cut,
        ));
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_copy(tid)),
            "Copy",
            &copy_shortcut,
            ContextAction::Copy,
        ));
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_paste(tid)),
            "Paste",
            &paste_shortcut,
            ContextAction::Paste,
        ));
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_smart_paste(tid)),
            "Paste Special",
            &smart_paste_shortcut,
            ContextAction::SmartPaste,
        ));
        items.push(self.ctx_menu_sep());

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_kb(
                Some(ic::icon_dd_rotate(tid)),
                "Rotate",
                "Space",
                ContextAction::RotateSelected,
            ));
            items.push(self.ctx_menu_item_kb(
                Some(ic::icon_dd_flip_x(tid)),
                "Mirror X",
                "X",
                ContextAction::MirrorX,
            ));
            items.push(self.ctx_menu_item_kb(
                Some(ic::icon_dd_flip_y(tid)),
                "Mirror Y",
                "Y",
                ContextAction::MirrorY,
            ));
            items.push(self.ctx_menu_item_kb(
                Some(ic::icon_dd_delete(tid)),
                "Delete",
                "Del",
                ContextAction::Delete,
            ));
            items.push(self.ctx_menu_sep());
        }

        items.push(self.ctx_menu_item_disabled(Some(ic::icon_dd_comment(tid)), "Comment...", None));
        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_pin_mapping(tid)),
            "Pin Mapping...",
            None,
        ));
        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_project_options(tid)),
            "Project Options...",
            None,
        ));
        items.push(self.ctx_menu_item_msg(
            Some(ic::icon_dd_preferences(tid)),
            "Preferences...",
            "",
            Message::Preferences(PreferencesMsg::Open),
        ));

        if !canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled(
                Some(ic::icon_dd_supplier_links(tid)),
                "Supplier Links...",
                None,
            ));
            // Properties → ensure the Properties panel is visible. The
            // panel already tracks the current selection, so it populates
            // with the right-clicked item's fields once shown.
            items.push(self.ctx_menu_item_msg(
                Some(ic::icon_dd_properties(tid)),
                "Properties...",
                "F11",
                Message::Menu(menu_bar::MenuMessage::OpenPropertiesPanel),
            ));
        }

        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }

    /// Build the Projects-panel tree-view right-click menu. The item
    /// set is derived from the clicked node's role in the tree —
    /// project root vs openable leaf vs container branch — so the
    /// menu matches what Altium shows in each context. Empty-area
    /// clicks are filtered out upstream (no menu shown), so `path`
    /// is guaranteed `Some` whenever this function runs.
    pub(super) fn view_project_tree_context_menu(
        &self,
        ctx: &crate::app::ProjectTreeContextMenuState,
    ) -> Element<'_, Message> {
        use crate::app::ProjectTreeAction as A;
        use signex_widgets::tree_view::{TreeIcon, get_node};

        let panel_ctx = &self.document_state.panel_ctx;
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(18);

        let Some(path) = ctx.path.as_ref() else {
            // Background (empty-area) clicks currently produce no menu —
            // Altium shows a "New Project..." flow there which we do
            // not ship yet. Return an empty placeholder container so
            // the overlay layer above has something to render into.
            return container(iced::widget::Space::new()).into();
        };
        let Some(node) = get_node(panel_ctx.project_tree.as_slice(), path) else {
            return container(iced::widget::Space::new()).into();
        };

        // Role detection:
        // - Root: the single top-level node created in `build_project_tree`,
        //   always at path `[0]`. Represents the whole project.
        // - Openable leaf: file-backed tree entry the project-navigation
        //   handler knows how to open (see `handle_dock_project_navigation_panel_message`).
        // - Container branch: everything else with children (Source
        //   Documents, Libraries, Settings folders).
        let is_root = path.len() == 1;
        // Library leaves under the project tree's
        // `Libraries ▸ <name>.snxlib` row carry the SnxLibrary icon.
        // The right-click menu for those rows is the Altium-style
        // `Add New ▸ Component / Symbol / Footprint` submenu,
        // which is distinct from the per-file open / explore / rename
        // menu the rest of the openable-leaf icons share. Detect by
        // icon + tree depth — the Libraries group sits two levels
        // below the project root
        // (path = `[project_idx, libraries_idx, library_idx]`).
        let is_library_node = matches!(node.icon, TreeIcon::SnxLibrary) && path.len() == 3;
        let is_openable_leaf = !is_library_node
            && matches!(
                node.icon,
                TreeIcon::Schematic
                    | TreeIcon::Pcb
                    | TreeIcon::SnxSchematic
                    | TreeIcon::SnxPcb
                    | TreeIcon::SnxProject
                    | TreeIcon::SnxFootprint
                    | TreeIcon::SnxSimulation
                    | TreeIcon::SnxLibrary
                    | TreeIcon::SnxSymbol
            );
        let is_container = !node.children.is_empty();
        let has_schematic = panel_ctx.has_schematic;

        if is_root {
            // Project root — Altium's "right-click project" menu. Items
            // we have not wired yet carry a "vX.Y" right-column badge
            // so the user knows which release lands the feature rather
            // than staring at a silently-greyed row.
            let has_tabs = !self.document_state.tabs.is_empty();
            // Multi-project: gate on the *clicked* project's directory,
            // not the active project's. Right-clicking project B's root
            // while project A is active should still enable Explore for
            // B. (#54)
            let has_project_dir = path
                .first()
                .and_then(|idx| self.document_state.projects.get(*idx))
                .and_then(|p| p.path.parent())
                .is_some();
            // Save is enabled when *either* a schematic is active
            // (saves the schematic) *or* the right-clicked project's
            // .snxprj is dirty (saves the project metadata after an
            // Add Existing). The combined gate keeps Save reachable
            // when the user has only added files and hasn't opened
            // any schematic tab.
            let project_path = path
                .first()
                .and_then(|idx| self.document_state.projects.get(*idx))
                .map(|p| p.path.clone());
            let project_is_dirty = project_path
                .as_ref()
                .map(|p| self.document_state.dirty_paths.contains(p))
                .unwrap_or(false);
            let can_save = has_schematic || project_is_dirty;

            items.push(self.ctx_menu_item_disabled(
                None,
                "Make Project Available Online...",
                Some("v3.4"),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Validate Project",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ValidateProject(
                    path.clone(),
                ))),
            ));
            let active_submenu = self.interaction_state.context_submenu;
            items.push(self.ctx_menu_item_submenu(
                None,
                "Add New to Project",
                ContextSubmenu::AddNewToProject,
                active_submenu == Some(ContextSubmenu::AddNewToProject),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Add Existing to Project...",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::AddExistingToProject(
                    path.clone(),
                ))),
            ));
            items.push(self.save_menu_item(can_save));
            items.push(self.ctx_menu_item_msg(
                None,
                "Rename...",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                    A::OpenProjectRenameDialog(path.clone()),
                )),
            ));
            items.push(self.ctx_menu_sep());
            items.push(if has_tabs {
                self.ctx_menu_item_msg(
                    None,
                    "Close Project Documents",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CloseAllDocuments)),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Close Project Documents", None)
            });
            items.push(self.ctx_menu_item_msg(
                None,
                "Close Project",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CloseProject(
                    path.clone(),
                ))),
            ));
            items.push(if has_project_dir {
                self.ctx_menu_item_msg(
                    None,
                    "Explore",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::RevealInExplorer(
                        path.clone(),
                    ))),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Explore", None)
            });
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_disabled(None, "Variants...", Some("v1.1")));
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_disabled(None, "Project Packager...", Some("v4.2")));
            items.push(self.ctx_menu_item_disabled(None, "Project Releaser...", Some("v5.2")));
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_disabled(None, "Share...", Some("v3.4")));
            items.push(self.ctx_menu_item_msg(
                None,
                "Project Options...",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenProjectOptions(
                    path.clone(),
                ))),
            ));
            // Enable Version Control — only surfaced when the
            // project dir has no `.git/` yet. After opt-in the row
            // disappears (the project already has version control;
            // future per-file tracking edits live in a different
            // surface).
            let no_git_yet = path
                .first()
                .and_then(|idx| self.document_state.projects.get(*idx))
                .and_then(|p| p.path.parent())
                .map(|dir| !dir.join(".git").exists())
                .unwrap_or(false);
            if no_git_yet {
                items.push(self.ctx_menu_item_msg(
                    None,
                    "Enable Version Control...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        A::OpenEnableVersionControl(path.clone()),
                    )),
                ));
            }
        } else if is_library_node {
            // Library node menu — mirrors Altium's "Add New ▸"
            // submenu: Component opens the New Component modal;
            // Symbol / Footprint mint a fresh primitive via the
            // adapter and open it as a standalone editor tab
            // (see `handle_add_library_primitive`). The basic
            // expand / refresh actions stay so empty libraries are
            // still navigable from the keyboard.
            items.push(self.ctx_menu_item_msg(
                None,
                "Add New ▸ Component",
                "",
                Message::Menu(crate::menu_bar::MenuMessage::AddLibraryComponent),
            ));
            // F31 (2026-05-03) — these create FILES, not individual
            // primitives. A `.snxsym` file holds many symbols (Altium
            // parity); user edits it via the SCH Library panel after
            // opening. Labels reworded so the user doesn't expect a
            // single-symbol creation flow here.
            items.push(self.ctx_menu_item_msg(
                None,
                "Add New ▸ Symbol Library",
                "",
                Message::Menu(crate::menu_bar::MenuMessage::AddLibrarySymbol),
            ));
            // v0.13.0 — footprint editor gated off; hide the create
            // entry so the user never reaches a dead flow.
            if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
                items.push(self.ctx_menu_item_msg(
                    None,
                    "Add New ▸ Footprint Library",
                    "",
                    Message::Menu(crate::menu_bar::MenuMessage::AddLibraryFootprint),
                ));
            }
            items.push(self.ctx_menu_sep());
            // Stage 18 distributor refresh stub — fires
            // `LibraryRefreshAllPricing` so the wiring is observable
            // even before the real adapter loop ships. Resolves the
            // clicked library's `.snxlib` file via the project tree.
            if let Some(lib_path) = self.library_node_path_from_tree(path.as_slice()) {
                items.push(self.ctx_menu_item_msg(
                    None,
                    "Refresh All Pricing",
                    "",
                    Message::Library(crate::library::LibraryMessage::LibraryRefreshAllPricing(
                        lib_path,
                    )),
                ));
            } else {
                items.push(self.ctx_menu_item_disabled(None, "Refresh All Pricing", None));
            }
            items.push(self.ctx_menu_sep());
            // Enable Version Control on the library directory itself —
            // mirrors the project-root row, but scoped to the
            // `.snxlib` parent. Hidden once the library has a `.git/`.
            if let Some(lib_file) = self.library_node_path_from_tree(path.as_slice())
                && let Some(lib_dir) = lib_file.parent()
                && !lib_dir.join(".git").exists()
            {
                items.push(self.ctx_menu_item_msg(
                    None,
                    "Enable Version Control...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                        A::OpenLibraryEnableVersionControl(path.clone()),
                    )),
                ));
                items.push(self.ctx_menu_sep());
            }
            let toggle_label = if node.expanded { "Collapse" } else { "Expand" };
            items.push(self.ctx_menu_item_msg(
                None,
                toggle_label,
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ToggleNode(
                    path.clone(),
                ))),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Refresh",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::Refresh)),
            ));
            // F23 — surface "Remove from Project" on library nodes.
            // Reuses the same RemoveDialog flow as sheet leaves;
            // `handle_remove_confirm` handles directory deletes
            // (.snxlib is a dir), library-entry removal from
            // `project.data.libraries`, and the orphan case where
            // the file doesn't exist on disk.
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_msg(
                None,
                "Remove from Project...",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRemoveDialog(
                    path.clone(),
                ))),
            ));
        } else if is_openable_leaf {
            // Sheet / PCB / library leaf — Altium's per-document menu.
            // Rows match the Altium screenshot exactly: Open + Explore
            // are wired; Print fires only when the clicked leaf is the
            // active tab (the print-preview flow renders the active
            // document). Other items are disabled stubs until the
            // matching engine actions land.
            // Same multi-project gate as above — leaf rows resolve
            // against their owning project, not the active one.
            let has_project_dir = path
                .first()
                .and_then(|idx| self.document_state.projects.get(*idx))
                .and_then(|p| p.path.parent())
                .is_some();
            let is_active_tab = self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .and_then(|tab| tab.path.file_name())
                .and_then(|f| f.to_str())
                .zip(
                    signex_widgets::tree_view::get_node(panel_ctx.project_tree.as_slice(), path)
                        .map(|n| n.label.as_str()),
                )
                .is_some_and(|(active_name, clicked_label)| active_name == clicked_label);

            items.push(self.ctx_menu_item_msg(
                None,
                "Open",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenNode(path.clone()))),
            ));
            items.push(self.ctx_menu_sep());
            items.push(if has_project_dir {
                self.ctx_menu_item_msg(
                    None,
                    "Explore",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::RevealInExplorer(
                        path.clone(),
                    ))),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Explore", None)
            });
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_msg(
                None,
                "Rename...",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRenameDialog(
                    path.clone(),
                ))),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Remove from Project",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::OpenRemoveDialog(
                    path.clone(),
                ))),
            ));
            items.push(self.ctx_menu_sep());
            // Print follows the Altium convention: it exports / prints
            // the active document. Enabled whenever a schematic tab is
            // open, regardless of which tree row was clicked — the
            // previous label-match gate silently disabled the row when
            // the tree label differed from the active tab's filename.
            let _ = is_active_tab;
            items.push(if has_schematic {
                self.ctx_menu_item_msg(
                    None,
                    "Print...",
                    "",
                    Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::PrintActive)),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Print...", None)
            });
            items.push(self.ctx_menu_item_disabled(None, "Show Differences...", Some("v4.3")));
        } else if is_container {
            // Source Documents / Libraries / Settings folders. These have
            // no Altium direct analogue (Altium groups these under the
            // project root); the minimum useful action is the tree-
            // widget's own expand/collapse.
            let label = if node.expanded { "Collapse" } else { "Expand" };
            items.push(self.ctx_menu_item_msg(
                None,
                label,
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ToggleNode(
                    path.clone(),
                ))),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Expand All",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::ExpandAll)),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Collapse All",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::CollapseAll)),
            ));
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_msg(
                None,
                "Refresh",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::Refresh)),
            ));
        } else {
            // Unknown role — give the user at least Refresh so a stale
            // tree can be rebuilt without restarting.
            items.push(self.ctx_menu_item_msg(
                None,
                "Refresh",
                "",
                Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(A::Refresh)),
            ));
        }

        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }

    /// Build the document-tab right-click menu. Items are derived
    /// from the clicked tab index — the per-tab "Close [filename]"
    /// row carries the live tab title, and the bulk-close rows are
    /// gated on whether they'd be no-ops (single tab open → no
    /// "others" to close). The split / tile / merge rows from
    /// Altium's screenshot are intentionally left out: Signex's
    /// editor doesn't support split-pane layout yet.
    pub(super) fn view_tab_context_menu(
        &self,
        ctx: &crate::app::TabContextMenuState,
    ) -> Element<'_, Message> {
        use crate::app::TabContextAction as A;

        let panel_ctx = &self.document_state.panel_ctx;
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(6);

        let Some(tab) = self.document_state.tabs.get(ctx.tab_idx) else {
            return container(iced::widget::Space::new()).into();
        };

        let title = tab.title.clone();
        let total_tabs = self.document_state.tabs.len();
        // A tab can be undocked iff it's not already living in its own
        // OS window. Single-tab workspaces still show the row (Altium
        // does too) — undocking a sole tab leaves the main window
        // empty, which is fine.
        use super::state::WindowKind;
        let already_undocked =
            self.ui_state.windows.values().any(
                |kind| matches!(kind, WindowKind::UndockedTab { path, .. } if *path == tab.path),
            );

        items.push(self.ctx_menu_item_msg_no_icon(
            &format!("Close {title}"),
            "",
            Message::ContextMenu(ContextMenuMsg::TabAction(A::Close(ctx.tab_idx))),
        ));
        if total_tabs > 1 {
            items.push(self.ctx_menu_item_msg_no_icon(
                "Close All Other Documents",
                "",
                Message::ContextMenu(ContextMenuMsg::TabAction(A::CloseAllOthers(ctx.tab_idx))),
            ));
        } else {
            items.push(self.ctx_menu_item_disabled_no_icon("Close All Other Documents"));
        }
        items.push(self.ctx_menu_item_msg_no_icon(
            "Close All Documents",
            "",
            Message::ContextMenu(ContextMenuMsg::TabAction(A::CloseAll)),
        ));
        items.push(self.ctx_menu_sep());
        items.push(if already_undocked {
            self.ctx_menu_item_disabled_no_icon("Open In New Window")
        } else {
            self.ctx_menu_item_msg_no_icon(
                "Open In New Window",
                "",
                Message::ContextMenu(ContextMenuMsg::TabAction(A::Undock(ctx.tab_idx))),
            )
        });

        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }
}
