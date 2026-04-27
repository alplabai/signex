use iced::widget::{canvas, column, container, row};
use iced::{Element, Length};

mod dialogs;
mod translate;

use super::*;

// ── Submenu chevron — single source of truth ─────────────────────────
//
// Right-pointing angle quote (U+203A), NOT the BLACK RIGHT-POINTING
// TRIANGLE (U+25B6) which Windows renders via the colour emoji font.
// Same glyph the menu_bar dropdowns use; the matching size below keeps
// every submenu launcher visually aligned across the whole app
// (canvas right-click, project-tree right-click, File/Edit/View menu).
const SUBMENU_ARROW: &str = "›";
const SUBMENU_ARROW_SIZE: f32 = 18.0;

impl Signex {
    #[allow(clippy::vec_init_then_push)]
    fn view_context_menu(&self) -> Element<'_, Message> {
        use crate::icons as ic;
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(20);
        let canvas = self.interaction_state.active_canvas();
        let panel_ctx = &self.document_state.panel_ctx;
        let tid = self.ui_state.theme_id;

        items.push(self.ctx_menu_item_disabled(
            Some(ic::icon_dd_find_similar(tid)),
            "Find Similar Objects...",
            None,
        ));
        items.push(self.ctx_menu_item_msg(
            Some(ic::icon_chrome_search(tid)),
            "Find Text...",
            "Ctrl+F",
            Message::OpenFind,
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
            "Ctrl+X",
            ContextAction::Cut,
        ));
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_copy(tid)),
            "Copy",
            "Ctrl+C",
            ContextAction::Copy,
        ));
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_paste(tid)),
            "Paste",
            "Ctrl+V",
            ContextAction::Paste,
        ));
        items.push(self.ctx_menu_item_kb(
            Some(ic::icon_dd_smart_paste(tid)),
            "Smart Paste",
            "Shift+Ctrl+V",
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
            Message::OpenPreferences,
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
    fn view_project_tree_context_menu(
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
        let is_openable_leaf = matches!(
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

            items.push(self.ctx_menu_item_disabled(None, "Make Project Available Online...", Some("v3.4")));
            items.push(self.ctx_menu_item_disabled(None, "Validate Project", Some("v0.9")));
            let active_submenu = self.interaction_state.context_submenu;
            items.push(self.ctx_menu_item_submenu(
                None,
                "Add New to Project",
                ContextSubmenu::AddNewToProject,
                active_submenu == Some(ContextSubmenu::AddNewToProject),
            ));
            items.push(self.ctx_menu_item_disabled(None, "Add Existing to Project...", Some("v0.9")));
            items.push(self.save_menu_item(has_schematic));
            items.push(self.ctx_menu_item_disabled(None, "Rename...", Some("v0.9")));
            items.push(self.ctx_menu_sep());
            items.push(if has_tabs {
                self.ctx_menu_item_msg(
                    None,
                    "Close Project Documents",
                    "",
                    Message::ProjectTreeAction(A::CloseAllDocuments),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Close Project Documents", None)
            });
            items.push(self.ctx_menu_item_msg(
                None,
                "Close Project",
                "",
                Message::ProjectTreeAction(A::CloseProject(path.clone())),
            ));
            items.push(if has_project_dir {
                self.ctx_menu_item_msg(
                    None,
                    "Explore",
                    "",
                    Message::ProjectTreeAction(A::RevealInExplorer(path.clone())),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Explore", None)
            });
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_disabled(None, "Variants...", Some("v1.1")));
            items.push(self.ctx_menu_item_disabled(None, "History & Version Control", Some(SUBMENU_ARROW)));
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_disabled(None, "Project Packager...", Some("v4.2")));
            items.push(self.ctx_menu_item_disabled(None, "Project Releaser...", Some("v5.2")));
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_disabled(None, "Share...", Some("v3.4")));
            items.push(self.ctx_menu_item_disabled(None, "Project Options...", Some("v0.9")));
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
                    signex_widgets::tree_view::get_node(
                        panel_ctx.project_tree.as_slice(),
                        path,
                    )
                    .map(|n| n.label.as_str()),
                )
                .is_some_and(|(active_name, clicked_label)| active_name == clicked_label);

            items.push(self.ctx_menu_item_msg(
                None,
                "Open",
                "",
                Message::ProjectTreeAction(A::OpenNode(path.clone())),
            ));
            items.push(self.ctx_menu_sep());
            items.push(if has_project_dir {
                self.ctx_menu_item_msg(
                    None,
                    "Explore",
                    "",
                    Message::ProjectTreeAction(A::RevealInExplorer(path.clone())),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Explore", None)
            });
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_msg(
                None,
                "Rename...",
                "",
                Message::ProjectTreeAction(A::OpenRenameDialog(path.clone())),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Remove from Project",
                "",
                Message::ProjectTreeAction(A::OpenRemoveDialog(path.clone())),
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
                    Message::ProjectTreeAction(A::PrintActive),
                )
            } else {
                self.ctx_menu_item_disabled(None, "Print...", None)
            });
            items.push(self.ctx_menu_item_disabled(None, "Show Differences...", Some("v4.3")));
            items.push(self.ctx_menu_item_disabled(None, "History & Version Control", Some(SUBMENU_ARROW)));
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
                Message::ProjectTreeAction(A::ToggleNode(path.clone())),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Expand All",
                "",
                Message::ProjectTreeAction(A::ExpandAll),
            ));
            items.push(self.ctx_menu_item_msg(
                None,
                "Collapse All",
                "",
                Message::ProjectTreeAction(A::CollapseAll),
            ));
            items.push(self.ctx_menu_sep());
            items.push(self.ctx_menu_item_msg(
                None,
                "Refresh",
                "",
                Message::ProjectTreeAction(A::Refresh),
            ));
        } else {
            // Unknown role — give the user at least Refresh so a stale
            // tree can be rebuilt without restarting.
            items.push(self.ctx_menu_item_msg(
                None,
                "Refresh",
                "",
                Message::ProjectTreeAction(A::Refresh),
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
    fn view_tab_context_menu(
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
        let already_undocked = self
            .ui_state
            .windows
            .values()
            .any(|kind| matches!(kind, WindowKind::UndockedTab { path, .. } if *path == tab.path));

        items.push(self.ctx_menu_item_msg(
            None,
            &format!("Close {title}"),
            "",
            Message::TabContextAction(A::Close(ctx.tab_idx)),
        ));
        if total_tabs > 1 {
            items.push(self.ctx_menu_item_msg(
                None,
                "Close All Other Documents",
                "",
                Message::TabContextAction(A::CloseAllOthers(ctx.tab_idx)),
            ));
        } else {
            items.push(self.ctx_menu_item_disabled(None, "Close All Other Documents", None));
        }
        items.push(self.ctx_menu_item_msg(
            None,
            "Close All Documents",
            "",
            Message::TabContextAction(A::CloseAll),
        ));
        items.push(self.ctx_menu_sep());
        items.push(if already_undocked {
            self.ctx_menu_item_disabled(None, "Open In New Window", None)
        } else {
            self.ctx_menu_item_msg(
                None,
                "Open In New Window",
                "",
                Message::TabContextAction(A::Undock(ctx.tab_idx)),
            )
        });

        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }

    /// Save menu row, active only when a schematic tab is open —
    /// used by the project-tree context menu's project-root variant.
    /// Altium's right-click menus do not surface keyboard shortcuts,
    /// so the shortcut column is intentionally empty here even though
    /// Ctrl+S still fires `MenuMessage::Save` globally.
    fn save_menu_item<'a>(&self, enabled: bool) -> Element<'a, Message> {
        if enabled {
            self.ctx_menu_item_msg(
                None,
                "Save",
                "",
                Message::Menu(crate::menu_bar::MenuMessage::Save),
            )
        } else {
            self.ctx_menu_item_disabled(None, "Save", None)
        }
    }

    /// Build the 26-wide icon column for a context-menu row. Mirrors
    /// `dd_item_icon` in `active_bar.rs` so the icons in the right-
    /// click menu visually align with the dropdown menus in the bar.
    /// `None` still reserves the column so labels in icon-less rows
    /// line up with their iconed neighbours.
    fn ctx_menu_icon_slot<'a>(
        icon: Option<iced::widget::svg::Handle>,
        muted: bool,
    ) -> Element<'a, Message> {
        match icon {
            Some(h) => {
                let mut s = iced::widget::svg(h)
                    .width(20)
                    .height(20)
                    .content_fit(iced::ContentFit::Contain);
                if muted {
                    s = s.style(|_: &iced::Theme, _| iced::widget::svg::Style {
                        color: Some(iced::Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0)),
                    });
                }
                container(s)
                    .width(26)
                    .height(20)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                    .into()
            }
            None => iced::widget::Space::new().width(26).height(20).into(),
        }
    }

    fn ctx_menu_item_kb<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        shortcut: &str,
        action: ContextAction,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                Self::ctx_menu_icon_slot(icon, false),
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(Message::ContextAction(action))
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    fn ctx_menu_item_msg<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        shortcut: &str,
        message: Message,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                Self::ctx_menu_icon_slot(icon, false),
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(message)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    /// Submenu launcher row — hover to open after a 200 ms delay
    /// (subscription-driven), or click for instant open. Active state
    /// highlights the row so the user can tell which submenu is open.
    fn ctx_menu_item_submenu<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        kind: ContextSubmenu,
        active: bool,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        let active_bg = crate::styles::ti(tokens.selection);
        let arrow_c = crate::styles::ti(tokens.text_secondary);
        let btn = iced::widget::button(
            iced::widget::row![
                Self::ctx_menu_icon_slot(icon, false),
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(SUBMENU_ARROW.to_string())
                    .size(SUBMENU_ARROW_SIZE)
                    .color(arrow_c),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(Message::OpenContextSubmenu(kind))
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ if active => Some(iced::Background::Color(active_bg)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        );
        // Wrap in mouse_area for on_enter / on_exit so the hover timer
        // (handled by `Message::TickContextSubmenuHover`) can open the
        // submenu after 200 ms without the user clicking.
        iced::widget::mouse_area(btn)
            .on_enter(Message::HoverContextSubmenu(kind))
            .on_exit(Message::LeaveContextSubmenu)
            .into()
    }

    fn ctx_menu_item_disabled<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        right: Option<&str>,
    ) -> Element<'a, Message> {
        let text_secondary = crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary);
        let mut row = iced::widget::row![
            Self::ctx_menu_icon_slot(icon, true),
            iced::widget::text(label.to_string())
                .size(11)
                .color(text_secondary),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        if let Some(right_text) = right {
            // Submenu arrow (›) renders at the unified `SUBMENU_ARROW_SIZE`
            // so disabled placeholder rows showing the chevron line up
            // with live submenu launchers and the menu_bar dropdowns.
            // Every other right-column string (keyboard shortcuts,
            // "coming in vX.Y" version badges) renders at 10 to match
            // the shortcut-column sizing used by enabled rows.
            let size = if right_text == SUBMENU_ARROW {
                SUBMENU_ARROW_SIZE
            } else {
                10.0
            };
            row = row.push(
                iced::widget::text(right_text.to_string())
                    .size(size)
                    .color(text_secondary),
            );
        }

        container(row)
            .padding([4, 12])
            .width(Self::CONTEXT_MENU_WIDTH)
            .into()
    }

    /// Build the secondary submenu (Place / Align) shown to the right
    /// of the parent context menu. Each row dispatches an Active Bar
    /// action via `ContextAction::ActiveBar(...)` so the placement /
    /// transform pipelines stay shared with the toolbar.
    fn view_context_submenu(&self, kind: ContextSubmenu) -> Element<'_, Message> {
        use crate::active_bar::ActiveBarAction as A;
        use crate::icons as ic;
        let tid = self.ui_state.theme_id;
        let panel_ctx = &self.document_state.panel_ctx;
        let mut items: Vec<Element<'_, Message>> = Vec::new();
        let mk = |icon: iced::widget::svg::Handle,
                  label: &'static str,
                  action: A|
         -> Element<'_, Message> {
            self.ctx_menu_item_kb(Some(icon), label, "", ContextAction::ActiveBar(action))
        };
        match kind {
            ContextSubmenu::Place => {
                // Wires + buses + entries
                items.push(mk(ic::icon_dd_wire(tid), "Wire", A::DrawWire));
                items.push(mk(ic::icon_dd_bus(tid), "Bus", A::DrawBus));
                items.push(mk(
                    ic::icon_dd_bus_entry(tid),
                    "Bus Entry",
                    A::PlaceBusEntry,
                ));
                items.push(mk(
                    ic::icon_dd_net_label(tid),
                    "Net Label",
                    A::PlaceNetLabel,
                ));
                items.push(self.ctx_menu_sep());
                // Ports
                items.push(mk(ic::icon_dd_port(tid), "Port", A::PlacePort));
                items.push(mk(
                    ic::icon_dd_off_sheet(tid),
                    "Off Sheet Connector",
                    A::PlaceOffSheetConnector,
                ));
                items.push(self.ctx_menu_sep());
                // Power ports (the four most common)
                items.push(mk(ic::icon_dd_gnd(tid), "GND Power Port", A::PlacePowerGND));
                items.push(mk(ic::icon_dd_vcc(tid), "VCC Power Port", A::PlacePowerVCC));
                items.push(mk(
                    ic::icon_dd_pwr_plus5(tid),
                    "+5 Power Port",
                    A::PlacePowerPlus5,
                ));
                items.push(mk(
                    ic::icon_dd_pwr_plus12(tid),
                    "+12 Power Port",
                    A::PlacePowerPlus12,
                ));
                items.push(self.ctx_menu_sep());
                // Directives
                items.push(mk(
                    ic::icon_dd_param_set(tid),
                    "Parameter Set",
                    A::PlaceParameterSet,
                ));
                items.push(mk(ic::icon_dd_no_erc(tid), "Generic No ERC", A::PlaceNoERC));
                items.push(mk(
                    ic::icon_dd_diff_pair(tid),
                    "Differential Pair",
                    A::PlaceDiffPair,
                ));
                items.push(mk(ic::icon_dd_blanket(tid), "Blanket", A::PlaceBlanket));
                items.push(self.ctx_menu_sep());
                // Harness
                items.push(mk(
                    ic::icon_dd_harness(tid),
                    "Signal Harness",
                    A::PlaceSignalHarness,
                ));
                items.push(mk(
                    ic::icon_dd_harness_conn(tid),
                    "Harness Connector",
                    A::PlaceHarnessConnector,
                ));
                items.push(mk(
                    ic::icon_dd_harness_entry(tid),
                    "Harness Entry",
                    A::PlaceHarnessEntry,
                ));
                items.push(self.ctx_menu_sep());
                // Sheet symbols
                items.push(mk(
                    ic::icon_dd_sheet_symbol(tid),
                    "Sheet Symbol",
                    A::PlaceSheetSymbol,
                ));
                items.push(mk(
                    ic::icon_dd_sheet_entry(tid),
                    "Sheet Entry",
                    A::PlaceSheetEntry,
                ));
                items.push(mk(
                    ic::icon_dd_device_sheet(tid),
                    "Device Sheet Symbol",
                    A::PlaceDeviceSheetSymbol,
                ));
                items.push(self.ctx_menu_sep());
                // Component
                items.push(mk(ic::icon_component(tid), "Part", A::PlaceComponent));
                items.push(self.ctx_menu_sep());
                // Text
                items.push(mk(
                    ic::icon_dd_text_string(tid),
                    "Text String",
                    A::PlaceTextString,
                ));
                items.push(mk(
                    ic::icon_dd_text_frame(tid),
                    "Text Frame",
                    A::PlaceTextFrame,
                ));
                items.push(mk(ic::icon_dd_note(tid), "Note", A::PlaceNote));
            }
            ContextSubmenu::Align => {
                // Altium gating: pairwise aligns (Left/Right/Top/Bottom/H/V
                // Centers) need ≥2 items to make sense; Distribute needs
                // ≥3 (two endpoints + at least one item to space between
                // them); Align To Grid works on a single item too. The
                // submenu is only opened when something is selected, so
                // grid is always enabled here.
                let n = self.interaction_state.canvas.selected.len();
                let pair = n >= 2;
                let dist = n >= 3;
                let mk_or_disabled = |icon: iced::widget::svg::Handle,
                                      label: &'static str,
                                      action: A,
                                      enabled: bool|
                 -> Element<'_, Message> {
                    if enabled {
                        mk(icon, label, action)
                    } else {
                        self.ctx_menu_item_disabled(Some(icon), label, None)
                    }
                };
                items.push(mk_or_disabled(
                    ic::icon_dd_align_left(tid),
                    "Align Left",
                    A::AlignLeft,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_right(tid),
                    "Align Right",
                    A::AlignRight,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_hcenter(tid),
                    "Align Horizontal Centers",
                    A::AlignHorizontalCenters,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_dist_horiz(tid),
                    "Distribute Horizontally",
                    A::DistributeHorizontally,
                    dist,
                ));
                items.push(self.ctx_menu_sep());
                items.push(mk_or_disabled(
                    ic::icon_dd_align_top(tid),
                    "Align Top",
                    A::AlignTop,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_bottom(tid),
                    "Align Bottom",
                    A::AlignBottom,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_align_vcenter(tid),
                    "Align Vertical Centers",
                    A::AlignVerticalCenters,
                    pair,
                ));
                items.push(mk_or_disabled(
                    ic::icon_dd_dist_vert(tid),
                    "Distribute Vertically",
                    A::DistributeVertically,
                    dist,
                ));
                items.push(self.ctx_menu_sep());
                items.push(mk(
                    ic::icon_dd_align_grid(tid),
                    "Align To Grid",
                    A::AlignToGrid,
                ));
            }
            ContextSubmenu::AddNewToProject => {
                // Altium parity: this is the master "Add New" picker for
                // the active project. Every entry below requires
                // project-file write support (v0.9) plus the matching
                // editor — none ship in v0.8, so each row carries a
                // version badge and stays disabled. The submenu still
                // launches so the user can see what's coming.
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_wire(tid)),
                    "Schematic",
                    Some("v0.9"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_component(tid)),
                    "Schematic Library",
                    Some("v0.9"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_part_actions(tid)),
                    "PCB",
                    Some("v2.0"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_component(tid)),
                    "PCB Library",
                    Some("v2.0"),
                ));
                items.push(self.ctx_menu_sep());
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_string(tid)),
                    "Output Job",
                    Some("v1.3"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_frame(tid)),
                    "Design Notebook",
                    Some("v1.4"),
                ));
                items.push(self.ctx_menu_sep());
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_string(tid)),
                    "Constraint File",
                    Some("v3.x"),
                ));
                items.push(self.ctx_menu_item_disabled(
                    Some(ic::icon_dd_text_string(tid)),
                    "VHDL File",
                    Some("v3.x"),
                ));
            }
        }
        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&panel_ctx.tokens))
            .into()
    }

    fn ctx_menu_sep<'a>(&self) -> Element<'a, Message> {
        let border_c = crate::styles::ti(self.document_state.panel_ctx.tokens.border);
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(border_c.into()),
                ..container::Style::default()
            })
            .into()
    }

    /// Export-error modal — plain "something went wrong, here's the
    /// message" dialog with an OK button. Sits on top of the print-preview
    /// overlay when both would otherwise render; dismiss_layer handles
    /// click-outside-to-close.
    fn view_export_error(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, text};
        let msg = match &self.document_state.export_error {
            Some(m) => m.clone(),
            None => return iced::widget::Space::new().into(),
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let border_c = crate::styles::ti(tokens.border);
        let err_red = iced::Color::from_rgb(0.85, 0.25, 0.25);

        let ok_btn = button(text("OK").size(12).color(iced::Color::WHITE))
            .padding([6, 20])
            .on_press(Message::DismissExportError)
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(err_red.into()),
                    text_color: iced::Color::WHITE,
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            );

        let body = column![
            row![
                text("\u{26A0}").size(24).color(err_red),
                iced::widget::Space::new().width(10),
                text("Export Failed").size(14).color(text_c),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::Space::new().height(8),
            text(msg).size(12).color(text_c),
            iced::widget::Space::new().height(12),
            row![iced::widget::Space::new().width(Length::Fill), ok_btn,],
        ]
        .padding(20);

        let card = container(body)
            .max_width(480)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(panel_bg.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::from(8.0),
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..container::Style::default()
            });

        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    /// Print Preview overlay. Shows thumbnails of every rendered page on
    /// the left, the selected page full-size on the right, with Export PDF
    /// and Close buttons at the bottom. Triggered by File → Print Preview
    /// (Ctrl+P) and File → Export PDF; disappears on Close or when the
    /// export completes. In-window flavour wraps the body in `wrap_modal`
    /// for backdrop + drag-to-position.
    fn view_print_preview(&self) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        use crate::app::view::dialogs::wrap_modal;
        let body = self.view_print_preview_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&ModalId::PrintPreview)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            body,
            offset,
            self.ui_state.window_size,
            (Self::PDF_MODAL_W, Self::PDF_MODAL_H),
        )
    }

    /// Detached-window flavour — bare body, no backdrop, no in-window
    /// drag handler (the OS window-drag covers the header).
    pub(super) fn view_print_preview_body(&self) -> Element<'_, Message> {
        self.view_print_preview_inner(false)
    }

    fn view_print_preview_inner(&self, draggable: bool) -> Element<'_, Message> {
        use crate::app::state::{ModalId, PdfPreviewTab};
        use crate::app::view::dialogs::{
            close_x_button, detached_header, draggable_header, MODAL_HEADER_HEIGHT,
            MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE,
        };
        use iced::widget::{button, column, container, row, text, Space};
        let theme_id = self.ui_state.theme_id;

        let preview = match &self.document_state.preview {
            Some(p) => p,
            None => return iced::widget::Space::new().into(),
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let accent_c = crate::styles::ti(tokens.accent);

        // Header — same chrome as every other modal.
        let header_content: Element<'_, Message> = container(
            row![
                text("Export PDF").size(MODAL_HEADER_TITLE_SIZE).color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::PrintPreviewClose, theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                ModalId::PrintPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, ModalId::PrintPreview)
        };

        // Tab strip — Preview | Settings.
        let tab_strip = self.view_pdf_tab_strip(preview.active_tab);

        // Body switches by tab.
        let body: Element<'_, Message> = match preview.active_tab {
            PdfPreviewTab::Preview => self.view_pdf_preview_tab(preview),
            PdfPreviewTab::Settings => self.view_pdf_settings_tab(preview),
        };

        // Footer — page count + Export PDF.
        let export_btn = button(text("Export PDF").size(12).color(iced::Color::WHITE))
            .padding([6, 14])
            .on_press(Message::PrintPreviewExport)
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(accent_c.into()),
                    text_color: iced::Color::WHITE,
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                },
            );
        let footer_caption = if preview.pages.is_empty() {
            "No files selected for export".to_string()
        } else {
            format!("{} page(s) — preview at 96 DPI", preview.pages.len())
        };
        let footer = container(
            row![
                text(footer_caption).size(11).color(text_muted),
                Space::new().width(Length::Fill),
                export_btn,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14]);

        let dialog = container(
            column![header, tab_strip, body, footer]
                .width(Self::PDF_MODAL_W)
                .height(Self::PDF_MODAL_H),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    /// Two-tab strip — Preview | Settings — sitting just under the
    /// modal header. Uses the same `TabPill` widget the document tab
    /// bar paints with: 3-sided border (top + L/R), accent stripe on
    /// the active tab, fill that fades for inactive. `is_last=true`
    /// on the rightmost so the trailing border doesn't double up
    /// against an adjacent tab's left edge.
    fn view_pdf_tab_strip(
        &self,
        active: crate::app::state::PdfPreviewTab,
    ) -> Element<'_, Message> {
        use crate::app::state::PdfPreviewTab;
        use iced::widget::{container, mouse_area, row, text, Space};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let pill_fill = |is_active: bool| -> iced::Color {
            if is_active {
                hover_c
            } else {
                iced::Color {
                    a: hover_c.a * 0.35,
                    ..hover_c
                }
            }
        };

        let tab = |label: &'static str, this: PdfPreviewTab, is_last: bool| {
            let is_active = this == active;
            let label_color = if is_active { text_c } else { text_muted };
            let style = TabPillStyle {
                fill: pill_fill(is_active),
                border: border_c,
                accent: accent_c,
                is_active,
                is_last,
                accent_position: AccentPosition::Bottom,
            };
            let inner = container(text(label).size(12).color(label_color))
                .padding([6, 18]);
            mouse_area(TabPill::new(inner, style))
                .on_press(Message::PrintPreviewSetTab(this))
                .interaction(iced::mouse::Interaction::Pointer)
        };

        container(
            row![
                tab("Preview", PdfPreviewTab::Preview, false),
                tab("Settings", PdfPreviewTab::Settings, true),
                Space::new().width(Length::Fill),
            ]
            .spacing(0)
            .align_y(iced::Alignment::Center),
        )
        .padding([0, 14])
        .into()
    }

    /// Preview tab — top toolbar (Sheet/Colour/Pages/Output), thumb
    /// rail on the left, pan/zoom viewport on the right.
    fn view_pdf_preview_tab(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{
            button, checkbox, column, container, image, mouse_area, row, scrollable, text,
            text_input, Space,
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let accent_c = crate::styles::ti(tokens.accent);
        let hover_c = crate::styles::ti(tokens.hover);

        let mode_button = |label: &'static str, selected: bool, msg: Message| {
            let selected_bg = accent_c;
            let selected_text = iced::Color::WHITE;
            let default_bg = panel_bg;
            let default_text = text_c;
            button(text(label).size(11).color(if selected {
                selected_text
            } else {
                default_text
            }))
            .padding([4, 10])
            .on_press(msg)
            .style(
                move |_: &iced::Theme, _status| iced::widget::button::Style {
                    background: Some(if selected {
                        selected_bg.into()
                    } else {
                        default_bg.into()
                    }),
                    text_color: if selected {
                        selected_text
                    } else {
                        default_text
                    },
                    border: iced::Border {
                        color: border_c,
                        width: 1.0,
                        radius: iced::border::Radius::from(4.0),
                    },
                    ..iced::widget::button::Style::default()
                },
            )
        };

        let colour_controls = row![
            text("Colour").size(11).color(text_muted),
            mode_button(
                "Color",
                matches!(preview.pdf_options.colour_mode, signex_output::ColourMode::Colour),
                Message::PrintPreviewSetColourMode(signex_output::ColourMode::Colour),
            ),
            mode_button(
                "Gray",
                matches!(preview.pdf_options.colour_mode, signex_output::ColourMode::Grayscale),
                Message::PrintPreviewSetColourMode(signex_output::ColourMode::Grayscale),
            ),
            mode_button(
                "B/W",
                matches!(preview.pdf_options.colour_mode, signex_output::ColourMode::BlackAndWhite),
                Message::PrintPreviewSetColourMode(signex_output::ColourMode::BlackAndWhite),
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let range_controls = row![
            text("Pages").size(11).color(text_muted),
            mode_button(
                "All",
                matches!(preview.pdf_options.page_range, signex_output::PageRange::All),
                Message::PrintPreviewSetPageRangeAll,
            ),
            mode_button(
                "Current",
                matches!(preview.pdf_options.page_range, signex_output::PageRange::Current),
                Message::PrintPreviewSetPageRangeCurrent,
            ),
            mode_button(
                "Specific",
                matches!(preview.pdf_options.page_range, signex_output::PageRange::Specific(_)),
                Message::PrintPreviewSetPageRangeSpecific,
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let specific_page_input: Element<'_, Message> = if matches!(
            preview.pdf_options.page_range,
            signex_output::PageRange::Specific(_)
        ) {
            row![
                text("Page").size(11).color(text_muted),
                text_input("1", &preview.specific_page_input)
                    .on_input(Message::PrintPreviewSetSpecificPageInput)
                .padding([4, 8])
                .size(12)
                .width(80),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            iced::widget::Space::new().height(0).into()
        };

        let fit_to_page = matches!(
            preview.pdf_options.scale,
            signex_output::PdfScale::FitToPage
        );
        let toggles_row = row![
            text("Output").size(11).color(text_muted),
            row![
                checkbox(fit_to_page).on_toggle(Message::PrintPreviewSetFitToPage),
                text("Fit to Page").size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
            row![
                checkbox(preview.pdf_options.include_title_block)
                    .on_toggle(Message::PrintPreviewSetIncludeTitleBlock),
                text("Title Block").size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);

        let page_size_label = match &preview.pdf_options.page_size {
            signex_output::PageSize::IsoA0 => "ISO A0",
            signex_output::PageSize::IsoA1 => "ISO A1",
            signex_output::PageSize::IsoA2 => "ISO A2",
            signex_output::PageSize::IsoA3 => "ISO A3",
            signex_output::PageSize::IsoA4 => "ISO A4",
            signex_output::PageSize::IsoA5 => "ISO A5",
            signex_output::PageSize::AnsiA => "ANSI A",
            signex_output::PageSize::AnsiB => "ANSI B",
            signex_output::PageSize::AnsiC => "ANSI C",
            signex_output::PageSize::AnsiD => "ANSI D",
            signex_output::PageSize::AnsiE => "ANSI E",
            signex_output::PageSize::UsLetter => "US Letter",
            signex_output::PageSize::UsLegal => "US Legal",
            signex_output::PageSize::Custom { .. } => "Custom",
        };
        let orientation_label = match preview.pdf_options.orientation {
            signex_output::Orientation::Portrait => "Portrait",
            signex_output::Orientation::Landscape => "Landscape",
        };
        let summary_row = row![
            text("Sheet").size(11).color(text_muted),
            text(format!("{} • {}", page_size_label, orientation_label))
                .size(11)
                .color(text_c),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let toolbar = container(
            row![
                summary_row,
                Space::new().width(16),
                colour_controls,
                Space::new().width(12),
                range_controls,
                specific_page_input,
                Space::new().width(Length::Fill),
                toggles_row,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 14]);

        // Thumb rail.
        let mut thumbs: iced::widget::Column<'_, Message> = column![].spacing(4).padding(8);
        for (i, page) in preview.pages.iter().enumerate() {
            let selected = i == preview.selected;
            let thumb = image(preview.page_handles[i].clone())
                .content_fit(iced::ContentFit::Contain)
                .width(120)
                .height(85);
            let card_bg = if selected { hover_c } else { panel_bg };
            let card_border = if selected { accent_c } else { border_c };
            let card = container(
                column![
                    thumb,
                    text(format!("Page {}", page.page_number))
                        .size(10)
                        .color(text_c)
                ]
                .spacing(2)
                .align_x(iced::Alignment::Center),
            )
            .padding(4)
            .width(132)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(card_bg.into()),
                border: iced::Border {
                    color: card_border,
                    width: if selected { 2.0 } else { 1.0 },
                    radius: iced::border::Radius::from(4.0),
                },
                ..container::Style::default()
            });
            thumbs = thumbs.push(mouse_area(card).on_press(Message::PrintPreviewSelectPage(i)));
        }
        let thumb_rail = scrollable(thumbs).width(148).height(Length::Fill);

        // Pan/zoom viewport. The image is positioned via Translate so
        // pan delta moves it inside a clipped container — no
        // scrollbars, just drag-to-pan + wheel-zoom.
        let viewport: Element<'_, Message> = if preview.pages.is_empty() {
            container(
                text("No files selected — toggle files in Settings → Files.")
                    .size(12)
                    .color(text_muted),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Color::WHITE.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::from(2.0),
                },
                ..container::Style::default()
            })
            .into()
        } else {
            let selected_page = &preview.pages[preview.selected];
            let zoom = preview.zoom;
            let scaled_w = (selected_page.width_px as f32 * zoom).max(64.0);
            let scaled_h = (selected_page.height_px as f32 * zoom).max(64.0);
            // At zoom ≤ 1 we want the page to fill the viewport
            // preserving aspect; above 1× we render at exact scaled
            // pixels and let the user pan around.
            let img_el: Element<'_, Message> = if zoom <= 1.0 {
                image(preview.page_handles[preview.selected].clone())
                    .content_fit(iced::ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                image(preview.page_handles[preview.selected].clone())
                    .content_fit(iced::ContentFit::Fill)
                    .width(Length::Fixed(scaled_w))
                    .height(Length::Fixed(scaled_h))
                    .into()
            };
            // Position the image at the pan offset. Below 1× the pan
            // is forced to (0, 0) (see the zoom handler) so the
            // translate is a no-op.
            let positioned: Element<'_, Message> = if zoom <= 1.0 {
                img_el
            } else {
                super::view::translate::Translate::new(img_el, preview.pan).into()
            };
            let surface = container(positioned)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Color::WHITE.into()),
                    border: iced::Border {
                        color: border_c,
                        width: 1.0,
                        radius: iced::border::Radius::from(2.0),
                    },
                    ..container::Style::default()
                })
                .clip(true);
            let interaction = if zoom > 1.0 {
                if preview.panning.is_some() {
                    iced::mouse::Interaction::Grabbing
                } else {
                    iced::mouse::Interaction::Grab
                }
            } else {
                iced::mouse::Interaction::default()
            };
            iced::widget::mouse_area(surface)
                .on_press(Message::PrintPreviewPanStart)
                .on_release(Message::PrintPreviewPanFinished)
                .on_scroll(|delta| {
                    use iced::mouse::ScrollDelta;
                    let dy = match delta {
                        ScrollDelta::Lines { y, .. } => y,
                        ScrollDelta::Pixels { y, .. } => y,
                    };
                    Message::PrintPreviewZoom(dy)
                })
                .interaction(interaction)
                .into()
        };

        let zoom = preview.zoom;
        let page_caption = if preview.pages.is_empty() {
            text("—".to_string()).size(11).color(text_muted)
        } else {
            let selected_page = &preview.pages[preview.selected];
            text(format!(
                "Page {} of {} — {}×{} px · {:.0}%",
                selected_page.page_number,
                preview.pages.len(),
                selected_page.width_px,
                selected_page.height_px,
                zoom * 100.0,
            ))
            .size(11)
            .color(text_muted)
        };

        let centre = column![viewport, page_caption]
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fill);

        let body_row = container(
            row![thumb_rail, Space::new().width(8), centre]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding([0, 14])
        .width(Length::Fill)
        .height(Length::Fill);

        column![toolbar, body_row]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Settings tab — stitches the three section helpers below into a
    /// single scrollable column. Each helper owns its own widgets and
    /// reads/writes through `preview.pdf_options.*` directly so the
    /// rasterizer and exporter stay in lockstep with the UI.
    fn view_pdf_settings_tab(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{column, scrollable, Space};
        let body = column![
            self.view_pdf_files_section(preview),
            Space::new().height(10),
            self.view_pdf_structure_section(preview),
            Space::new().height(10),
            self.view_pdf_additional_section(preview),
        ]
        .spacing(0)
        .padding([10, 14]);
        scrollable(body)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    /// Section header strip — accent panel-bg with a 1 px border.
    /// Reused by every Settings-tab section.
    fn pdf_section_title(&self, label: &'static str) -> Element<'_, Message> {
        use iced::widget::{container, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);
        container(text(label).size(12).color(text_c))
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(panel_bg.into()),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: iced::border::Radius::default(),
                },
                ..container::Style::default()
            })
            .into()
    }

    /// Settings → Choose Project Files.
    fn view_pdf_files_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{button, checkbox, column, container, row, scrollable, text, Space};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let panel_bg = crate::styles::ti(tokens.panel_bg);

        let project_sheets: Vec<(std::path::PathBuf, String)> = self
            .document_state
            .active_loaded_project()
            .map(|p| {
                let dir = std::path::PathBuf::from(&p.data.dir);
                p.data
                    .sheets
                    .iter()
                    .map(|s| (dir.join(&s.filename), s.name.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let mut file_list: iced::widget::Column<'_, Message> =
            column![].spacing(2).padding([8, 12]);
        if project_sheets.is_empty() {
            file_list = file_list.push(
                text("No project loaded — load a .standard_pro to pick files.")
                    .size(11)
                    .color(text_muted),
            );
        } else {
            for (path, name) in &project_sheets {
                let is_selected = preview.selected_files.contains(path);
                let path_str = path.display().to_string();
                let row_el = row![
                    checkbox(is_selected).on_toggle({
                        let path = path.clone();
                        move |_| Message::PrintPreviewToggleFile(path.clone())
                    }),
                    column![
                        text(name.clone()).size(11).color(text_c),
                        text(path_str).size(10).color(text_muted),
                    ]
                    .spacing(1),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center);
                file_list = file_list.push(row_el);
            }
        }
        let secondary_btn_style = move |_: &iced::Theme, _status| iced::widget::button::Style {
            background: Some(panel_bg.into()),
            text_color: text_c,
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: iced::border::Radius::from(3.0),
            },
            ..iced::widget::button::Style::default()
        };
        let file_actions = row![
            button(text("Select All").size(11).color(text_c))
                .padding([3, 8])
                .on_press(Message::PrintPreviewSelectAllFiles)
                .style(secondary_btn_style),
            button(text("Clear").size(11).color(text_c))
                .padding([3, 8])
                .on_press(Message::PrintPreviewClearAllFiles)
                .style(secondary_btn_style),
        ]
        .spacing(6);

        column![
            self.pdf_section_title("Choose Project Files"),
            container(
                column![
                    text("Select the files in the project to export from the list. Multiple files can be selected.")
                        .size(11)
                        .color(text_muted),
                    Space::new().height(6),
                    container(scrollable(file_list).height(160))
                        .width(Length::Fill)
                        .style(move |_: &iced::Theme| container::Style {
                            border: iced::Border {
                                color: border_c,
                                width: 1.0,
                                radius: iced::border::Radius::default(),
                            },
                            ..container::Style::default()
                        }),
                    Space::new().height(6),
                    file_actions,
                ]
                .padding([10, 12]),
            ),
        ]
        .spacing(0)
        .into()
    }

    /// Settings → Structure Settings.
    fn view_pdf_structure_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use iced::widget::{checkbox, column, container, row, text, Space};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;

        let variant_label = opts
            .variant
            .clone()
            .unwrap_or_else(|| "Base".to_string());
        let mut variant_options: Vec<String> = vec!["Base".to_string()];
        variant_options.extend(preview.variants.clone());
        variant_options.dedup();
        let variant_picker = iced::widget::pick_list(
            variant_options,
            Some(variant_label),
            |s| {
                if s.eq_ignore_ascii_case("Base") {
                    Message::PrintPreviewSetVariant(None)
                } else {
                    Message::PrintPreviewSetVariant(Some(s))
                }
            },
        )
        .text_size(11)
        .width(220);

        let labelled_check = |label: &'static str,
                              value: bool,
                              on: fn(bool) -> Message|
         -> iced::widget::Row<'_, Message> {
            row![
                text(label).size(11).color(text_muted).width(150),
                checkbox(value).on_toggle(on),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
        };

        column![
            self.pdf_section_title("Structure Settings"),
            container(
                column![
                    text("If checked, exported sheets are expanded from logical to physical sheets. Choose a variant and which expanded names appear.")
                        .size(11)
                        .color(text_muted),
                    Space::new().height(8),
                    row![
                        checkbox(opts.use_physical_structure)
                            .on_toggle(Message::PrintPreviewSetUsePhysicalStructure),
                        text("Use Physical Structure").size(11).color(text_c),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    Space::new().height(6),
                    row![
                        text("Variant").size(11).color(text_muted).width(150),
                        variant_picker,
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    Space::new().height(8),
                    labelled_check("Designators", opts.physical_designators, Message::PrintPreviewSetPhysicalDesignators),
                    labelled_check("Net Labels", opts.physical_net_labels, Message::PrintPreviewSetPhysicalNetLabels),
                    labelled_check("Ports and Sheet Entries", opts.physical_ports, Message::PrintPreviewSetPhysicalPorts),
                    labelled_check("Sheet Number Parameter", opts.physical_sheet_number, Message::PrintPreviewSetPhysicalSheetNumber),
                    labelled_check("Document Number Parameter", opts.physical_document_number, Message::PrintPreviewSetPhysicalDocumentNumber),
                ]
                .padding([10, 12])
                .spacing(2),
            ),
        ]
        .spacing(0)
        .into()
    }

    /// Settings → Additional PDF Settings.
    fn view_pdf_additional_section(
        &self,
        preview: &crate::app::state::PreviewState,
    ) -> Element<'_, Message> {
        use crate::app::state::PdfQuality;
        use iced::widget::{checkbox, column, container, row, text, Space};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let opts = &preview.pdf_options;

        let lbl_check = move |label: &'static str, value: bool, on: fn(bool) -> Message| {
            row![
                checkbox(value).on_toggle(on),
                text(label).size(11).color(text_c),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
        };

        let zoom_slider = iced::widget::slider(
            0.0_f32..=1.0,
            opts.bookmark_zoom,
            Message::PrintPreviewSetBookmarkZoom,
        )
        .step(0.05_f32)
        .width(180);
        let zoom_col = column![
            text("Zoom").size(11).color(text_c),
            text("Slider controls the zoom level in the PDF reader when jumping to components or nets.")
                .size(10)
                .color(text_muted),
            Space::new().height(6),
            row![
                text("Far").size(10).color(text_muted),
                zoom_slider,
                text("Close").size(10).color(text_muted),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(2);

        let info_col = column![
            text("Additional Information").size(11).color(text_c),
            Space::new().height(4),
            lbl_check(
                "Generate nets information",
                opts.generate_nets_info,
                Message::PrintPreviewSetGenerateNetsInfo,
            ),
            Space::new().height(4),
            text("The following bookmarks can be created in the PDF for nets:")
                .size(10)
                .color(text_muted),
            row![
                Space::new().width(14),
                column![
                    lbl_check("Pins", opts.bookmark_pins, Message::PrintPreviewSetBookmarkPins),
                    lbl_check("Net Labels", opts.bookmark_net_labels, Message::PrintPreviewSetBookmarkNetLabels),
                    lbl_check("Ports", opts.bookmark_ports, Message::PrintPreviewSetBookmarkPorts),
                ]
                .spacing(2),
            ],
            Space::new().height(4),
            lbl_check(
                "Include Component Parameters",
                opts.include_component_parameters,
                Message::PrintPreviewSetIncludeComponentParameters,
            ),
            lbl_check(
                "Global Bookmarks for Components and Nets",
                opts.global_bookmarks,
                Message::PrintPreviewSetGlobalBookmarks,
            ),
        ]
        .spacing(2);

        let schematics_include_col = column![
            text("Schematics include").size(11).color(text_c),
            Space::new().height(4),
            lbl_check("No-ERC Markers", opts.include_no_erc_markers, Message::PrintPreviewSetIncludeNoErcMarkers),
            lbl_check("Parameter Sets", opts.include_parameter_sets, Message::PrintPreviewSetIncludeParameterSets),
            lbl_check("Probes", opts.include_probes, Message::PrintPreviewSetIncludeProbes),
            lbl_check("Blankets", opts.include_blankets, Message::PrintPreviewSetIncludeBlankets),
            lbl_check("Notes", opts.include_notes, Message::PrintPreviewSetIncludeNotes),
            row![
                Space::new().width(14),
                lbl_check("Collapsed notes", opts.include_collapsed_notes, Message::PrintPreviewSetIncludeCollapsedNotes),
            ],
            Space::new().height(8),
            text("Quality").size(11).color(text_c),
            iced::widget::pick_list(
                vec![PdfQuality::Draft72, PdfQuality::Medium300, PdfQuality::High600],
                Some(preview.quality),
                Message::PrintPreviewSetQuality,
            )
            .text_size(11)
            .width(180),
        ]
        .spacing(2);

        let radio = move |label: &'static str,
                          this: signex_output::ColourMode,
                          current: signex_output::ColourMode,
                          on: fn(signex_output::ColourMode) -> Message| {
            iced::widget::radio(label, this, Some(current), on)
                .text_size(11)
                .size(14)
        };

        let sch_color_col = column![
            text("Schematics Color Mode").size(11).color(text_c),
            Space::new().height(4),
            radio("Color", signex_output::ColourMode::Colour, opts.colour_mode, Message::PrintPreviewSetColourMode),
            radio("Greyscale", signex_output::ColourMode::Grayscale, opts.colour_mode, Message::PrintPreviewSetColourMode),
            radio("Monochrome", signex_output::ColourMode::BlackAndWhite, opts.colour_mode, Message::PrintPreviewSetColourMode),
            Space::new().height(8),
            text("PCB Color Mode").size(11).color(text_c),
            Space::new().height(4),
            radio("Color", signex_output::ColourMode::Colour, opts.pcb_colour_mode, Message::PrintPreviewSetPcbColourMode),
            radio("Greyscale", signex_output::ColourMode::Grayscale, opts.pcb_colour_mode, Message::PrintPreviewSetPcbColourMode),
            radio("Monochrome", signex_output::ColourMode::BlackAndWhite, opts.pcb_colour_mode, Message::PrintPreviewSetPcbColourMode),
        ]
        .spacing(2);

        column![
            self.pdf_section_title("Additional PDF Settings"),
            container(
                row![
                    column![zoom_col, Space::new().height(12), info_col]
                        .spacing(0)
                        .width(Length::FillPortion(2)),
                    Space::new().width(16),
                    schematics_include_col.width(Length::FillPortion(2)),
                    Space::new().width(16),
                    sch_color_col.width(Length::FillPortion(2)),
                ]
                .padding([10, 12]),
            ),
        ]
        .spacing(0)
        .into()
    }

    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        // Secondary windows (detached modals, future undocked tabs) render
        // just their own content — no menu / dock / canvas. The main
        // window's view_main drops any overlay whose modal is currently
        // detached so we don't double-render.
        if let Some(kind) = self.ui_state.windows.get(&window_id) {
            return match kind {
                super::state::WindowKind::DetachedModal(modal) => self.view_detached_modal(*modal),
                // Undocked tab = full duplicate of the main app view.
                // Shared Signex state means edits sync automatically; the
                // only difference between main and undocked is the OS
                // window id they render into.
                super::state::WindowKind::UndockedTab { .. } => self.view_main_for(window_id),
                super::state::WindowKind::DetachedPanel(kind) => {
                    let panel = crate::panels::view_panel(*kind, &self.document_state.panel_ctx)
                        .map(crate::dock::DockMessage::Panel)
                        .map(Message::Dock);
                    iced::widget::container(iced::widget::scrollable(panel))
                        .padding(8)
                        .into()
                }
            };
        }
        self.view_main_for(window_id)
    }

    /// Cursor-following translucent preview of a tab being dragged.
    /// Shape matches the real tab bar entry — rounded container with
    /// the title text, the ↗ undock indicator, and the × close icon —
    /// so it reads as "the tab itself is moving". The ghost is
    /// non-interactive; it just shows what the user is carrying.
    fn view_tab_drag_ghost(&self, title: &str) -> Element<'_, Message> {
        use iced::widget::{container, row, text};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let active_bg = crate::styles::ti(tokens.hover);
        let accent = crate::styles::ti(tokens.accent);
        // Match the live tab look: same TabPill widget, accent
        // stripe at the bottom, top-rounded corners. The previous
        // ghost showed an old style with inline ↗ undock + × close
        // glyphs that were removed when the tab right-click menu
        // landed.
        let pill_style = TabPillStyle {
            fill: iced::Color { a: 0.88, ..active_bg },
            border: crate::styles::ti(tokens.border),
            accent,
            is_active: true,
            is_last: true,
            accent_position: AccentPosition::Bottom,
        };
        let inner = container(row![text(title.to_string()).size(11).color(text_c)])
            .padding([4, 10]);
        let pill = TabPill::new(inner, pill_style);
        // Anchor near the cursor (right + below) so the pointer
        // remains visible while the ghost trails it.
        let (cx, cy) = self.interaction_state.last_mouse_pos;
        super::view::translate::Translate::new(pill, (cx + 10.0, cy + 6.0)).into()
    }

    /// Altium-style Move Selection dialog. Two numeric inputs plus
    /// OK / Cancel. No header drag region on the body itself — the
    /// modal opens borderless so the OS-window-drag handler owns that.
    fn view_move_selection_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let ms = &self.ui_state.move_selection;
        let selection_count = self.interaction_state.active_canvas().selected.len();

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Move Selection").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::CloseMoveSelectionDialog),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::MoveSelection,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        let field = |label: &'static str, value: &str, msg: fn(String) -> Message| {
            column![
                text(label).size(10).color(text_muted),
                text_input("0.00", value)
                    .on_input(msg)
                    .padding([4, 8])
                    .size(12),
            ]
            .spacing(4)
        };

        let body = container(
            column![
                text(format!("{} item(s) selected", selection_count))
                    .size(11)
                    .color(text_muted),
                Space::new().height(12),
                row![
                    field("ΔX (mm)", &ms.dx, Message::MoveSelectionDxChanged),
                    Space::new().width(14),
                    field("ΔY (mm)", &ms.dy, Message::MoveSelectionDyChanged),
                ]
                .align_y(iced::Alignment::Start),
            ]
            .spacing(0),
        )
        .padding([14, 14]);

        let ok_enabled = selection_count > 0;
        let ok_bg = if ok_enabled {
            iced::Color::from_rgb(0.00, 0.47, 0.84)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
        };
        let ok_fg = if ok_enabled {
            iced::Color::WHITE
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4)
        };
        let mut ok_btn = button(container(text("Apply").size(11).color(ok_fg)).padding([4, 14]))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(ok_bg)),
                border: iced::Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                text_color: ok_fg,
                ..iced::widget::button::Style::default()
            });
        if ok_enabled {
            ok_btn = ok_btn.on_press(Message::MoveSelectionApply);
        }

        let footer = container(
            row![
                Space::new().width(iced::Length::Fill),
                button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                    .on_press(Message::CloseMoveSelectionDialog)
                    .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04
                        ),)),
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_c,
                        },
                        text_color: text_c,
                        ..iced::widget::button::Style::default()
                    }),
                Space::new().width(8),
                ok_btn,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14]);

        container(
            column![header, body, footer]
                .width(iced::Length::Fixed(420.0))
                .height(iced::Length::Fixed(240.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Compact X close button shared by the detached-modal bodies.
    fn view_close_x(&self, message: Message) -> Element<'_, Message> {
        use iced::widget::{button, container, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text_secondary);
        let border = crate::styles::ti(tokens.border);
        button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
            .on_press(message)
            .style(
                move |_: &iced::Theme, status: iced::widget::button::Status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                        )),
                        _ => Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.03,
                        ))),
                    };
                    iced::widget::button::Style {
                        background: bg,
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border,
                        },
                        text_color: text_c,
                        ..iced::widget::button::Style::default()
                    }
                },
            )
            .into()
    }

    /// Altium F5 Net Color palette — list of net labels with a per-net
    /// color picker. Ships with a 10-swatch palette; a full ColorPicker
    /// widget can replace it later without changing the message contract.
    fn view_net_color_palette_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, button, column, container, row, scrollable, text};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Net Colors").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::CloseNetColorPalette),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::NetColorPalette,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        // Gather unique net labels from the active snapshot.
        let mut nets: Vec<String> = self
            .interaction_state
            .canvas
            .active_snapshot()
            .map(|s| {
                s.labels
                    .iter()
                    .filter(|l| {
                        matches!(
                            l.label_type,
                            signex_types::schematic::LabelType::Net
                                | signex_types::schematic::LabelType::Global
                                | signex_types::schematic::LabelType::Hierarchical
                        )
                    })
                    .map(|l| l.text.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();
        nets.sort();

        const PALETTE: &[(u8, u8, u8)] = &[
            (0xE0, 0x54, 0x54),
            (0xE0, 0xB0, 0x4A),
            (0x78, 0xC2, 0x6A),
            (0x42, 0xB8, 0xE0),
            (0x6F, 0x77, 0xE0),
            (0xB0, 0x6F, 0xE0),
            (0xE0, 0x6F, 0xB0),
            (0xC2, 0xA0, 0x78),
            (0x78, 0xC2, 0xA0),
            (0xA0, 0xA0, 0xA0),
        ];

        let mut rows_col = column![].spacing(4);
        if nets.is_empty() {
            rows_col = rows_col.push(
                text("No net labels on the active sheet.")
                    .size(11)
                    .color(text_muted),
            );
        } else {
            for net in nets {
                let current = self.ui_state.net_colors.get(&net).copied();
                let mut swatches = row![].spacing(4).align_y(iced::Alignment::Center);
                for (r, g, b) in PALETTE {
                    let is_current = current.is_some_and(|c| c.r == *r && c.g == *g && c.b == *b);
                    let swatch_color = iced::Color::from_rgb8(*r, *g, *b);
                    let border_w = if is_current { 2.0_f32 } else { 1.0_f32 };
                    let net_copy = net.clone();
                    let r_c = *r;
                    let g_c = *g;
                    let b_c = *b;
                    swatches =
                        swatches.push(
                            button(container(Space::new().width(14).height(14)).style(
                                move |_: &iced::Theme| container::Style {
                                    background: Some(iced::Background::Color(swatch_color)),
                                    border: iced::Border {
                                        width: border_w,
                                        radius: 2.0.into(),
                                        color: text_c,
                                    },
                                    ..container::Style::default()
                                },
                            ))
                            .on_press(Message::NetColorSet {
                                net: net_copy.clone(),
                                color: Some(signex_types::theme::Color {
                                    r: r_c,
                                    g: g_c,
                                    b: b_c,
                                    a: 255,
                                }),
                            })
                            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                                border: iced::Border::default(),
                                ..iced::widget::button::Style::default()
                            }),
                        );
                }
                // Clear-override button
                let net_clear = net.clone();
                swatches = swatches.push(
                    button(container(text("×").size(10).color(text_c)).padding([0, 6]))
                        .on_press(Message::NetColorSet {
                            net: net_clear,
                            color: None,
                        })
                        .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.04,
                            ))),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border_c,
                            },
                            text_color: text_c,
                            ..iced::widget::button::Style::default()
                        }),
                );

                rows_col = rows_col.push(
                    row![
                        text(net)
                            .size(11)
                            .color(text_c)
                            .width(iced::Length::FillPortion(2)),
                        swatches,
                    ]
                    .align_y(iced::Alignment::Center)
                    .padding([2, 8]),
                );
            }
        }

        container(
            column![
                header,
                container(scrollable(rows_col).height(iced::Length::Fill))
                    .padding([14, 14])
                    .height(iced::Length::Fill),
            ]
            .width(iced::Length::Fixed(520.0))
            .height(iced::Length::Fixed(480.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Altium-style Parameter Manager — a scrolling table listing every
    /// placed symbol with columns for reference / value / footprint and
    /// a "Parameter" column that reveals the union of custom fields
    /// across the design. Each cell is a text_input so the user can edit
    /// values inline. Changes route through Command::SetSymbolField so
    /// undo/redo / dirty-flagging behaves.
    fn view_parameter_manager_body(&self) -> Element<'_, Message> {
        use iced::widget::{Space, column, container, row, scrollable, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header = iced::widget::mouse_area(
            container(
                row![
                    text("Parameter Manager").size(14).color(text_c),
                    Space::new().width(iced::Length::Fill),
                    self.view_close_x(Message::CloseParameterManager),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([10, 14])
            .style(crate::styles::modal_header_strip(tokens)),
        )
        .on_press(Message::StartDetachedWindowDrag(
            super::state::ModalId::ParameterManager,
        ))
        .interaction(iced::mouse::Interaction::Grab);

        // Collect all parameter keys across symbols (besides the built-
        // in reference / value / footprint). Keeps the table compact —
        // only columns that someone actually uses show up.
        let Some(engine) = self.document_state.active_engine() else {
            return container(
                column![
                    header,
                    container(text("No active schematic.").size(11).color(text_muted))
                        .padding([14, 14]),
                ]
                .width(iced::Length::Fixed(900.0))
                .height(iced::Length::Fixed(560.0)),
            )
            .style(crate::styles::modal_card(tokens))
        .clip(true)
            .into();
        };
        let doc = engine.document();
        let mut keys: Vec<String> = doc
            .symbols
            .iter()
            .filter(|s| !s.is_power)
            .flat_map(|s| s.fields.keys().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        keys.sort();

        let header_row = {
            let mut r = row![
                text("Reference")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(100.0)),
                text("Value")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(160.0)),
                text("Footprint")
                    .size(10)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                r = r.push(
                    text(k.clone())
                        .size(10)
                        .color(text_muted)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            r.padding([4, 8])
        };

        let mut rows_col = column![].spacing(2);
        rows_col = rows_col.push(header_row);
        for sym in &doc.symbols {
            if sym.is_power {
                continue;
            }
            let mut r = row![
                text(sym.reference.clone())
                    .size(11)
                    .color(text_c)
                    .width(iced::Length::Fixed(100.0)),
                text(sym.value.clone())
                    .size(11)
                    .color(text_c)
                    .width(iced::Length::Fixed(160.0)),
                text(sym.footprint.clone())
                    .size(11)
                    .color(text_muted)
                    .width(iced::Length::Fixed(200.0)),
            ];
            for k in &keys {
                let v = sym.fields.get(k).cloned().unwrap_or_default();
                let sym_uuid = sym.uuid;
                let k_str = k.clone();
                r = r.push(
                    text_input("", &v)
                        .on_input(move |new_val| Message::ParameterManagerEdit {
                            symbol_uuid: sym_uuid,
                            key: k_str.clone(),
                            value: new_val,
                        })
                        .padding([2, 6])
                        .size(11)
                        .width(iced::Length::Fixed(140.0)),
                );
            }
            rows_col = rows_col.push(r.padding([2, 8]));
        }

        container(
            column![
                header,
                container(
                    scrollable(rows_col)
                        .direction(scrollable::Direction::Both {
                            vertical: scrollable::Scrollbar::default(),
                            horizontal: scrollable::Scrollbar::default(),
                        })
                        .height(iced::Length::Fill),
                )
                .padding([14, 14])
                .height(iced::Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                }),
            ]
            .width(iced::Length::Fixed(900.0))
            .height(iced::Length::Fixed(560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    /// Custom net-colour picker modal. Grid of quick-pick swatches on
    /// the left, precise R / G / B / hex on the right, live preview
    /// and OK / Cancel at the bottom. Ships with a 24-color palette
    /// matching the common Altium net-colour presets plus a handful of
    /// EDA-specific diagnostic colours.
    fn view_net_color_custom_picker(&self) -> Element<'_, Message> {
        use super::contracts::Channel;
        use iced::widget::{Space, button, column, container, row, text, text_input};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let draft = self.ui_state.net_color_custom.draft;

        // Expanded 48-swatch palette arranged as 6 cols × 8 rows so
        // the Quick Pick grid fills the modal's left column. First
        // three rows are "standard" hues, next three rows are shade
        // variants, and the last two rows hold greys / light pastels /
        // schematic-specific high-contrast hues.
        const PALETTE: &[(u8, u8, u8)] = &[
            // Row 1 — primaries (bright)
            (0xEF, 0x44, 0x44), // Red
            (0xF9, 0x73, 0x16), // Orange
            (0xEA, 0xB3, 0x08), // Yellow
            (0x22, 0xC5, 0x5E), // Green
            (0x06, 0xB6, 0xD4), // Cyan
            (0x3B, 0x82, 0xF6), // Blue
            // Row 2 — pinks + magentas + purples
            (0xF4, 0x72, 0xB6), // Pink 400
            (0xE1, 0x14, 0x8C), // Hot Pink
            (0xD9, 0x46, 0xEF), // Fuchsia
            (0xA8, 0x55, 0xF7), // Purple
            (0x8B, 0x5C, 0xF6), // Violet
            (0x6D, 0x28, 0xD9), // Indigo
            // Row 3 — greens + teals + lime
            (0x84, 0xCC, 0x16), // Lime
            (0x10, 0xB9, 0x81), // Emerald
            (0x14, 0xB8, 0xA6), // Teal
            (0x0E, 0xA5, 0xE9), // Sky
            (0x60, 0xA5, 0xFA), // Light Blue
            (0x2D, 0xD4, 0xBF), // Turquoise
            // Row 4 — dark variants
            (0x9F, 0x12, 0x39), // Wine
            (0xB4, 0x53, 0x09), // Rust
            (0xA1, 0x6A, 0x3C), // Brown
            (0x16, 0xA3, 0x4A), // Dark Green
            (0x15, 0x5E, 0x75), // Deep Cyan
            (0x1E, 0x40, 0xAF), // Deep Blue
            // Row 5 — extra dark / night hues
            (0x7F, 0x1D, 0x1D), // Deep Red
            (0x78, 0x35, 0x0F), // Auburn
            (0x5B, 0x21, 0xB6), // Royal Purple
            (0x3B, 0x0A, 0x45), // Eggplant
            (0x1E, 0x3A, 0x8A), // Navy
            (0x0F, 0x17, 0x2A), // Midnight
            // Row 6 — pastels
            (0xFE, 0xCA, 0xCA), // Pastel Red
            (0xFE, 0xD7, 0xAA), // Pastel Orange
            (0xFE, 0xF0, 0x8A), // Pastel Yellow
            (0xBB, 0xF7, 0xD0), // Pastel Green
            (0xA5, 0xF3, 0xFC), // Pastel Cyan
            (0xBF, 0xDB, 0xFE), // Pastel Blue
            // Row 7 — muted + desaturated
            (0x64, 0x74, 0x8B), // Slate
            (0x78, 0x71, 0x6C), // Stone
            (0x4B, 0x55, 0x63), // Dark Slate
            (0x9C, 0xA3, 0xAF), // Gray
            (0xD1, 0xD5, 0xDB), // Light Gray
            (0xFF, 0xFF, 0xFF), // White
            // Row 8 — schematic diagnostic colors
            (0xFF, 0x00, 0xFF), // Bright Magenta
            (0x00, 0xFF, 0xFF), // Bright Cyan
            (0xFF, 0xFF, 0x00), // Bright Yellow
            (0x00, 0xFF, 0x00), // Bright Green
            (0xFF, 0xA5, 0x00), // Bright Orange
            (0x1F, 0x23, 0x2A), // Ink
        ];

        let swatch_btn =
            |r: u8, g: u8, b: u8| -> Element<'_, Message> {
                let col = iced::Color::from_rgb8(r, g, b);
                let is_current = (draft.r - col.r).abs() < 0.01
                    && (draft.g - col.g).abs() < 0.01
                    && (draft.b - col.b).abs() < 0.01;
                let sw = iced::Color::from_rgb8(r, g, b);
                let border_w = if is_current { 2.0 } else { 1.0 };
                let border_col = if is_current {
                    iced::Color::WHITE
                } else {
                    iced::Color::from_rgba(0.2, 0.2, 0.22, 0.9)
                };
                button(container(Space::new().width(24).height(20)).style(
                    move |_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(sw)),
                        border: iced::Border {
                            width: border_w,
                            radius: 3.0.into(),
                            color: border_col,
                        },
                        ..container::Style::default()
                    },
                ))
                .padding(0)
                .on_press(Message::NetColorCustomDraft(col))
                .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                    border: iced::Border::default(),
                    ..iced::widget::button::Style::default()
                })
                .into()
            };

        // Build the 6 × 4 palette grid row by row.
        let mut palette_col = column![].spacing(6);
        for chunk in PALETTE.chunks(6) {
            let mut r_el = row![].spacing(6);
            for (r, g, b) in chunk {
                r_el = r_el.push(swatch_btn(*r, *g, *b));
            }
            palette_col = palette_col.push(r_el);
        }

        // RGB inputs — parse as u8, clamp on submit. Uses the
        // `draft` colour as the current value so swatch clicks and
        // text edits stay in sync.
        let channel_row =
            |label: &'static str, value: f32, chan: Channel| -> Element<'_, Message> {
                let v255 = (value * 255.0).round() as i32;
                row![
                    text(label)
                        .size(11)
                        .color(text_muted)
                        .width(iced::Length::Fixed(16.0)),
                    text_input("0", &v255.to_string())
                        .size(11)
                        .padding([3, 8])
                        .width(iced::Length::Fixed(70.0))
                        .on_input(move |s| Message::NetColorCustomChannel(chan, s)),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(6)
                .into()
            };

        let preview_hex = format!(
            "#{:02X}{:02X}{:02X}",
            (draft.r * 255.0).round() as u8,
            (draft.g * 255.0).round() as u8,
            (draft.b * 255.0).round() as u8,
        );
        let preview_box = container(Space::new().width(iced::Length::Fill).height(32)).style(
            move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(draft)),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..container::Style::default()
            },
        );

        let rgb_col = column![
            text("Custom RGB").size(11).color(text_c),
            Space::new().height(6),
            channel_row("R", draft.r, Channel::R),
            channel_row("G", draft.g, Channel::G),
            channel_row("B", draft.b, Channel::B),
            Space::new().height(10),
            preview_box,
            Space::new().height(4),
            text(preview_hex).size(10).color(text_muted),
        ]
        .spacing(4)
        .width(iced::Length::Fixed(150.0));

        let body = row![
            column![
                text("Quick Pick").size(11).color(text_c),
                Space::new().height(6),
                palette_col,
            ]
            .spacing(0)
            .width(iced::Length::Fill),
            Space::new().width(16),
            rgb_col,
        ];

        let footer = row![
            Space::new().width(iced::Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]),)
                .on_press(Message::NetColorCustomShow(false))
                .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }),
            Space::new().width(8),
            button(
                container(text("Use Color").size(11).color(iced::Color::WHITE)).padding([4, 14]),
            )
            .on_press(Message::NetColorCustomSubmit(draft))
            .style(move |_: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                border: iced::Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                text_color: iced::Color::WHITE,
                ..iced::widget::button::Style::default()
            }),
        ]
        .align_y(iced::Alignment::Center);

        let card = container(
            column![
                container(
                    row![
                        text("Pick Net Color").size(13).color(text_c),
                        Space::new().width(iced::Length::Fill),
                        self.view_close_x(Message::NetColorCustomShow(false)),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14])
                .style(crate::styles::toolbar_strip(
                    &self.document_state.panel_ctx.tokens
                )),
                container(body).padding([14, 14]),
                container(footer).padding([10, 14]),
            ]
            .width(iced::Length::Fixed(430.0)),
        )
        .style(crate::styles::context_menu(
            &self.document_state.panel_ctx.tokens,
        ));

        // Anchor below the Active Bar Net Color button (rightmost icon).
        let (ww, _wh) = self.ui_state.window_size;
        let card_w = 430.0;
        let x = ((ww - card_w) * 0.5).max(0.0);
        let y = crate::menu_bar::MENU_BAR_HEIGHT
            + if self.document_state.tabs.is_empty() {
                0.0
            } else {
                28.0
            }
            + 80.0;
        // Wrap in a mouse_area with on_press(Noop) so clicks inside the
        // card are captured and DON'T fall through to the dismiss
        // layer sitting beneath. Without this, clicking on the card's
        // background / between swatches closes the picker.
        let card_capturing = iced::widget::mouse_area(card).on_press(Message::Noop);
        super::view::translate::Translate::new(card_capturing, (x, y)).into()
    }

    /// Custom chrome for the borderless main window. Replaces the OS
    /// title bar with a 36 px strip:
    ///
    /// `[wordmark + menus] [drag] [search bar] [drag] [min│max│×]`
    ///
    /// The drag zones are the only mouse-area clickable regions — menu
    /// buttons, search, and window controls keep their own click
    /// handlers. Double-click on a drag zone toggles maximize.
    fn view_main_window_chrome<'a>(
        &self,
        menu_row: Element<'a, Message>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        use iced::widget::{Space, button, container, mouse_area, row, svg, text};
        use iced::{Alignment, Background, Border, Color, Length};

        // Window-control SVG icons resolved through `crate::icons` so the
        // accent sentinel in each SVG tints to the active theme at
        // fetch time.
        let theme_id = self.ui_state.theme_id;
        let h_min = crate::icons::icon_chrome_window_min(theme_id);
        let h_max = crate::icons::icon_chrome_window_max(theme_id);
        let h_close = crate::icons::icon_chrome_window_close(theme_id);
        let h_search = crate::icons::icon_chrome_search(theme_id);

        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let hover_c = crate::styles::ti(tokens.hover);
        let search_bg = crate::styles::ti(tokens.panel_bg);
        let search_border = crate::styles::ti(tokens.border);
        // Windows-native destructive red for the close hover — overrides
        // the theme hover so close reads as destructive at a glance.
        let close_hover = Color::from_rgba(0.78, 0.22, 0.22, 1.0);
        let btn_h = crate::menu_bar::MENU_BAR_HEIGHT;

        let chrome_btn = |handle: svg::Handle,
                          msg: Message,
                          hover_bg: Color,
                          hover_icon: Color|
         -> Element<'static, Message> {
            // 14×14 brings the X / – / □ glyphs up to native-Windows
            // chrome scale; the prior 10×10 left them visibly smaller
            // than the surrounding menu-bar text.
            let icon = svg(handle)
                .width(14)
                .height(14)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(text_c),
                });
            button(
                container(icon)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(46)
            .height(btn_h)
            .padding(0)
            .on_press(msg)
            .style(move |_: &iced::Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: if hovered {
                        Some(Background::Color(hover_bg))
                    } else {
                        None
                    },
                    text_color: if hovered { hover_icon } else { text_c },
                    border: Border::default(),
                    ..Default::default()
                }
            })
            .into()
        };

        let controls = row![
            chrome_btn(h_min.clone(), Message::MinimizeMainWindow, hover_c, text_c),
            chrome_btn(
                h_max.clone(),
                Message::ToggleMaximizeMainWindow,
                hover_c,
                text_c,
            ),
            chrome_btn(
                h_close.clone(),
                Message::CloseMainWindow,
                close_hover,
                Color::WHITE,
            ),
        ];

        // Left-pad the menu row so the wordmark doesn't sit flush against
        // the window edge; controls stay flush-right so their hover boxes
        // touch the corner like in Windows' native chrome.
        let menu_padded = container(menu_row).padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 8.0,
        });

        // Search bar placeholder — visual only for now. Matches VS Code's
        // central command palette peek: rounded rect with search icon
        // and muted prompt text.
        let search_icon =
            svg(h_search.clone())
                .width(12)
                .height(12)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(muted_c),
                });
        let search_bar: Element<'_, Message> = container(
            row![
                search_icon,
                text("Search files, symbols, commands…")
                    .size(11)
                    .color(muted_c),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0,
        })
        .width(440)
        .height(28)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(search_bg)),
            border: Border {
                color: search_border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into();

        // Drag zones on either side of the search bar. Double-click
        // toggles maximize (Windows title-bar convention).
        let drag_zone = || -> Element<'static, Message> {
            mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::StartMainWindowDrag)
            .on_double_click(Message::ToggleMaximizeMainWindow)
            .into()
        };

        // `width(Length::Fill)` on the row is load-bearing: without it, the
        // drag zones' Fill-width collapses to 0 because their parent (this
        // row) is Shrink, and the chrome loses all its draggable real
        // estate the moment menus + search + controls consume their
        // natural widths.
        let inner = row![menu_padded, drag_zone(), search_bar, drag_zone(), controls,]
            .width(Length::Fill)
            .align_y(Alignment::Center);

        container(inner)
            .width(Length::Fill)
            .height(btn_h)
            .style(crate::styles::toolbar_strip(tokens))
            .into()
    }

    fn view_detached_modal(&self, modal: super::state::ModalId) -> Element<'_, Message> {
        use super::state::ModalId;
        match modal {
            ModalId::AnnotateDialog => self.view_annotate_dialog_body(),
            ModalId::ErcDialog => self.view_erc_dialog_body(),
            ModalId::AnnotateResetConfirm => self.view_annotate_reset_confirm_body(),
            // Stubs — these modals don't yet have extractable bodies; fall
            // back to a placeholder so the window is non-empty until their
            // body helpers land.
            ModalId::MoveSelection => self.view_move_selection_body(),
            ModalId::NetColorPalette => self.view_net_color_palette_body(),
            ModalId::ParameterManager => self.view_parameter_manager_body(),
            ModalId::PrintPreview => self.view_print_preview_body(),
            ModalId::BomPreview => {
                // Stack the body underneath a 6 px edge-resize
                // overlay so the borderless OS window can be
                // resized by dragging its edges. Without this,
                // `decorations: false` strips the OS frame and
                // there's nothing to grab.
                let body = self.view_bom_preview_body();
                let resize_active = self
                    .document_state
                    .bom_preview
                    .as_ref()
                    .map(|p| p.column_resize.is_some())
                    .unwrap_or(false);
                let mut stack = iced::widget::Stack::new()
                    .push(body)
                    .push(Self::detached_modal_resize_overlay(modal));
                // While a column-resize drag is in flight, lay an
                // invisible mouse_area over the whole modal that
                // pins the cursor to ResizingHorizontally. Without
                // this, the cursor reverts to default the moment
                // it leaves the 4 px handle's hit zone — which
                // happens immediately on horizontal drag.
                if resize_active {
                    let overlay: Element<'_, Message> = iced::widget::mouse_area(
                        iced::widget::Space::new()
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_release(Message::BomPreviewColumnResizeEnd)
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .into();
                    stack = stack.push(overlay);
                }
                stack.into()
            }
            ModalId::Preferences
            | ModalId::FindReplace
            | ModalId::RenameDialog
            | ModalId::RemoveDialog => {
                iced::widget::container(iced::widget::text("Detached modal"))
                    .padding(20)
                    .into()
            }
        }
    }

    /// Same 6 px edge-resize overlay as the main window's, but
    /// emitting `StartDetachedModalResize { modal, direction }`
    /// so it dispatches to the right OS window. Used as a stack
    /// layer above the modal's body in `view_detached_modal`.
    fn detached_modal_resize_overlay<'a>(
        modal: super::state::ModalId,
    ) -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight =
            move |direction: Direction, cursor: Interaction, horizontal: bool|
                -> Element<'a, Message> {
                let (w, h) = if horizontal {
                    (Length::Fill, Length::Fixed(EDGE))
                } else {
                    (Length::Fixed(EDGE), Length::Fill)
                };
                mouse_area(Space::new().width(w).height(h))
                    .on_press(Message::StartDetachedModalResize { modal, direction })
                    .interaction(cursor)
                    .into()
            };
        let corner = move |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::StartDetachedModalResize { modal, direction })
            .interaction(cursor)
            .into()
        };

        let top = straight(Direction::North, Interaction::ResizingVertically, true);
        let bottom = straight(Direction::South, Interaction::ResizingVertically, true);
        let left = straight(Direction::West, Interaction::ResizingHorizontally, false);
        let right = straight(Direction::East, Interaction::ResizingHorizontally, false);
        let nw = corner(Direction::NorthWest, Interaction::ResizingDiagonallyDown);
        let ne = corner(Direction::NorthEast, Interaction::ResizingDiagonallyUp);
        let sw = corner(Direction::SouthWest, Interaction::ResizingDiagonallyUp);
        let se = corner(Direction::SouthEast, Interaction::ResizingDiagonallyDown);

        let middle = row![
            left,
            Space::new().width(Length::Fill).height(Length::Fill),
            right
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        column![
            row![nw, top, ne]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
            middle,
            row![sw, bottom, se]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_main_for(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        // Context-aware menu: each leaf gates on whether its action
        // makes sense in the current app state. `has_schematic` /
        // `has_selection` drive most entries; undo / redo consult
        // the engine's history so they grey out when empty.
        let menu_ctx = crate::menu_bar::MenuContext {
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            has_project: document.active_project.is_some(),
            has_selection: !interaction.canvas_for_window(window_id).selected.is_empty(),
            can_undo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_undo())
                .unwrap_or(false),
            can_redo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_redo())
                .unwrap_or(false),
            // Secondary windows (detached modal, undocked tab) borrow
            // the main window's scale. Good enough until per-window
            // scale tracking lands — it's only wrong if the user drags
            // a secondary window onto a monitor with a different DPI.
            scale_factor: ui.main_window_scale,
        };
        let menu_row = menu_bar::view(&document.panel_ctx.tokens, menu_ctx).map(Message::Menu);

        let left_has_panels = document.dock.has_panels(PanelPosition::Left);
        let right_has_panels = document.dock.has_panels(PanelPosition::Right);
        let bottom_has_panels = document.dock.has_panels(PanelPosition::Bottom);
        let left_collapsed = document.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = document.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = document.dock.is_collapsed(PanelPosition::Bottom);

        let left = self.view_dock_panel(
            PanelPosition::Left,
            left_has_panels,
            left_collapsed,
            ui.left_width,
        );
        let left_handle = self.view_resize_handle(
            DragTarget::LeftPanel,
            left_has_panels && !left_collapsed,
            true,
        );
        let center = self.view_center(window_id);
        let right_handle = self.view_resize_handle(
            DragTarget::RightPanel,
            right_has_panels && !right_collapsed,
            true,
        );
        let right = self.view_dock_panel(
            PanelPosition::Right,
            right_has_panels,
            right_collapsed,
            ui.right_width,
        );

        let center_row = row![left, left_handle, center, right_handle, right];
        let bottom_handle = self.view_resize_handle(
            DragTarget::BottomPanel,
            bottom_has_panels && !bottom_collapsed,
            false,
        );
        let bottom = self.view_dock_panel_h(
            PanelPosition::Bottom,
            bottom_has_panels,
            bottom_collapsed,
            ui.bottom_height,
        );

        let status = status_bar::view(
            ui.cursor_x,
            ui.cursor_y,
            ui.grid_visible,
            ui.snap_enabled,
            ui.zoom,
            ui.unit,
            &interaction.current_tool,
            ui.grid_size_mm,
            &document.panel_ctx.tokens,
        )
        .map(Message::StatusBar);

        // Partition tabs across windows: main owns every tab that isn't
        // currently rendered by an undocked-tab window; each undocked
        // window owns exactly its one tab. Closing a tab in one window
        // can no longer reach tabs that belong to the other.
        let all_undocked_paths: std::collections::HashSet<std::path::PathBuf> = ui
            .windows
            .values()
            .filter_map(|kind| match kind {
                super::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let is_main_window = ui.main_window_id == Some(window_id);

        // Main window is borderless: wordmark + menus + drag + search +
        // min/max/close in a single 36 px row. Undocked tab windows keep
        // their OS chrome and use the plain styled strip.
        let top_chrome: Element<'_, Message> = if is_main_window {
            self.view_main_window_chrome(menu_row, &document.panel_ctx.tokens)
        } else {
            menu_bar::wrap_plain(menu_row, &document.panel_ctx.tokens)
        };
        let mut main = column![top_chrome];
        let visible_paths: std::collections::HashSet<std::path::PathBuf> = if is_main_window {
            document
                .tabs
                .iter()
                .map(|t| t.path.clone())
                .filter(|p| !all_undocked_paths.contains(p))
                .collect()
        } else {
            match ui.windows.get(&window_id) {
                Some(super::state::WindowKind::UndockedTab { path, .. }) => {
                    std::iter::once(path.clone()).collect()
                }
                _ => std::collections::HashSet::new(),
            }
        };
        // Reserve the tab strip's vertical footprint regardless of
        // whether any document is open — opening the first document
        // would otherwise shift the entire chrome down by ~24 px,
        // which feels jarring. The 1 px chrome separator stays
        // visible too so the menu row always reads as a distinct
        // band above the tab strip.
        main = main.push(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(1)
                .style(crate::styles::chrome_separator(
                    &document.panel_ctx.tokens,
                )),
        );
        if !document.tabs.is_empty() && !visible_paths.is_empty() {
            // Resolve "really dragging" — Some only after the
            // cursor has travelled past a 6 px threshold from
            // the press origin. Without this, every click-to-
            // switch armed the drag state and flipped the
            // cursor to Grabbing instantaneously, plus flashed
            // the drag ghost.
            const DRAG_THRESHOLD_PX: f32 = 6.0;
            let dragging = ui.tab_dragging.and_then(|(idx, ox, oy)| {
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX {
                    Some(idx)
                } else {
                    None
                }
            });
            main = main.push(
                tab_bar::view(
                    &document.tabs,
                    document.active_tab,
                    dragging,
                    &visible_paths,
                    &document.panel_ctx.tokens,
                )
                .map(move |msg| Message::Tab { window_id, msg }),
            );
        } else {
            // Empty placeholder strip with the same metrics as
            // tab_bar::view: 2 px outer padding + 22 px tall inner
            // pill = 26 px total. Without this the chrome jumps
            // when the first tab opens.
            let placeholder = container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(26)
                .style(crate::styles::toolbar_strip(
                    &document.panel_ctx.tokens,
                ));
            main = main.push(placeholder);
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Borderless window needs its own edge-resize hit zones — the OS
        // frame would normally handle this, but `decorations: false`
        // removes WS_THICKFRAME on Windows. Tab windows keep OS
        // decorations so they skip the overlay entirely. The overlay is
        // applied later as a Stack layer over `main` so the content
        // keeps its natural origin and overlay y-coordinates stay
        // correct.
        let main: Element<'_, Message> = main.into();

        let has_active_bar = self.has_active_schematic();
        let dragging_tab = ui.tab_dragging.is_some();
        let needs_overlay = has_active_bar
            || interaction.editing_text.is_some()
            || interaction.context_menu.is_some()
            || interaction.project_tree_context_menu.is_some()
            || interaction.tab_context_menu.is_some()
            || interaction.active_bar_menu.is_some()
            || interaction.canvas.placement_paused
            || ui.panel_list_open
            || ui.find_replace.open
            || ui.preferences_open
            || ui.rename_dialog.is_some()
            || ui.remove_dialog.is_some()
            || ui.project_close_confirm.is_some()
            || document.bom_preview.is_some()
            || ui.annotate_dialog_open
            || ui.annotate_reset_confirm
            || ui.erc_dialog_open
            || !document.dock.floating.is_empty()
            || dragging_tab
            || ui.net_color_custom.show;

        if needs_overlay {
            let mut overlays = self.collect_overlays();
            // Tab drag ghost: only renders once the cursor has
            // travelled past the same 6 px threshold the cursor
            // gating uses (`tab_bar::view`). Mirrors that gate
            // here so press-without-move keeps the ghost off.
            if let Some((tab_idx, ox, oy)) = ui.tab_dragging
                && let Some(tab) = document.tabs.get(tab_idx)
            {
                const DRAG_GHOST_THRESHOLD_PX: f32 = 6.0;
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy
                    > DRAG_GHOST_THRESHOLD_PX * DRAG_GHOST_THRESHOLD_PX
                {
                    overlays.push(self.view_tab_drag_ghost(&tab.title));
                }
            }
            let mut stack = iced::widget::Stack::new().push(main);
            // Resize edges sit above the content but below functional
            // overlays (Active Bar, menus, modals) so the 6 px border
            // strip doesn't eat clicks on those.
            if is_main_window {
                stack = stack.push(Self::resize_edges_overlay());
            }
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else if is_main_window {
            iced::widget::Stack::new()
                .push(main)
                .push(Self::resize_edges_overlay())
                .into()
        } else {
            main.into()
        }
    }

    /// Full-window-sized Stack overlay that anchors 6 px resize hit
    /// zones at the borderless main window's edges and corners. Clicks
    /// on the edges call `iced::window::drag_resize` via
    /// `StartMainWindowResize`; anywhere in the middle is an empty
    /// `Space` so events fall through to the content layer below.
    ///
    /// Used as a stack layer over `main` rather than as a structural
    /// wrapper, so the content keeps its natural y-origin and overlay
    /// coordinates (Active Bar, text edit, net-colour picker) stay
    /// correct without a +EDGE correction everywhere.
    fn resize_edges_overlay<'a>() -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight =
            |direction: Direction, cursor: Interaction, horizontal: bool| -> Element<'a, Message> {
                let (w, h) = if horizontal {
                    (Length::Fill, Length::Fixed(EDGE))
                } else {
                    (Length::Fixed(EDGE), Length::Fill)
                };
                mouse_area(Space::new().width(w).height(h))
                    .on_press(Message::StartMainWindowResize(direction))
                    .interaction(cursor)
                    .into()
            };

        let corner = |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::StartMainWindowResize(direction))
            .interaction(cursor)
            .into()
        };

        let top = straight(Direction::North, Interaction::ResizingVertically, true);
        let bottom = straight(Direction::South, Interaction::ResizingVertically, true);
        let left = straight(Direction::West, Interaction::ResizingHorizontally, false);
        let right = straight(Direction::East, Interaction::ResizingHorizontally, false);
        let nw = corner(Direction::NorthWest, Interaction::ResizingDiagonallyDown);
        let ne = corner(Direction::NorthEast, Interaction::ResizingDiagonallyUp);
        let sw = corner(Direction::SouthWest, Interaction::ResizingDiagonallyUp);
        let se = corner(Direction::SouthEast, Interaction::ResizingDiagonallyDown);

        // Middle row: left/right edges frame a Fill/Fill empty Space so
        // the whole overlay is window-sized and the centre passes
        // clicks through.
        let middle = row![
            left,
            Space::new().width(Length::Fill).height(Length::Fill),
            right
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        column![
            row![nw, top, ne]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
            middle,
            row![sw, bottom, se]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_dock_panel(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx)
            .map(Message::Dock);
        let width = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(width)
            .height(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_dock_panel_h(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx)
            .map(Message::Dock);
        let height = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(Length::Fill)
            .height(height)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_resize_handle(
        &self,
        target: DragTarget,
        visible: bool,
        horizontal: bool,
    ) -> Element<'_, Message> {
        let size = if visible { 5 } else { 0 };
        let handle_container = if horizontal {
            container(iced::widget::Space::new())
                .width(size)
                .height(Length::Fill)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(size)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        };
        let interaction = if horizontal {
            iced::mouse::Interaction::ResizingHorizontally
        } else {
            iced::mouse::Interaction::ResizingVertically
        };
        iced::widget::mouse_area(handle_container)
            .interaction(interaction)
            .on_press(Message::DragStart(target))
            .into()
    }

    fn view_center(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let is_main = self.ui_state.main_window_id == Some(window_id);
        let has_schematic = if is_main {
            self.has_active_schematic()
        } else {
            // An undocked tab window renders if its path still has a
            // live engine in the HashMap. Falls back to the main
            // predicate when the window has already been dropped from
            // the windows map (mid-close frame).
            self.document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some()
        };
        if has_schematic {
            // Canvas events from non-main windows need to carry the
            // window_id through to the dispatch layer so the right
            // per-window canvas receives the mutation. Keyboard
            // shortcuts that synthesize `Message::CanvasEvent` keep
            // targeting the main canvas unchanged.
            let base: Element<'_, Message> =
                canvas(self.interaction_state.canvas_for_window(window_id))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            if is_main {
                base
            } else {
                base.map(move |msg| match msg {
                    Message::CanvasEvent(event) => {
                        Message::CanvasEventInWindow { window_id, event }
                    }
                    other => other,
                })
            }
        } else if self.has_active_pcb() {
            canvas(&self.interaction_state.pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            // Distinguish "nothing loaded at all" from "project loaded,
            // but no document picked yet" — the second case is what
            // the user sees right after opening a .standard_pro before
            // clicking any node in the project tree.
            let (title, hint) = if self.document_state.active_project.is_some() {
                (
                    "No document selected",
                    "Choose a schematic or PCB from the project tree",
                )
            } else {
                (
                    "No document open",
                    "Open a project with File > Open or Ctrl+O",
                )
            };
            container(
                column![
                    iced::widget::text(title).size(14).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                    iced::widget::text(hint).size(11).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
        }
    }

    fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        // Opaque semi-transparent backdrop that blocks interaction with
        // underlying content. Left-click anywhere on it triggers the
        // dismiss message.
        //
        // We intentionally do *not* wire `on_right_press` — iced's
        // `mouse_area` would `capture_event()` the right-press and
        // prevent the underlying canvas from starting a pan. Instead
        // the canvas itself owns the right-press (its pan gesture) and
        // closes the context menu once the pan actually starts moving
        // (see `canvas/mod.rs`'s `CursorMoved` handler, which fires
        // `CloseContextMenu` the moment `pan_moved` flips on).
        const BACKDROP_OPACITY: f32 = 0.55;
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.0,
                        0.0,
                        0.0,
                        BACKDROP_OPACITY,
                    ))),
                    ..container::Style::default()
                }),
        )
        .on_press(on_press)
        .into()
    }

    fn collect_overlays(&self) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let mut layers = Vec::new();

        // Export-error modal — appears when PDF / netlist / BOM export
        // hits a user-actionable failure (write permission, invalid path,
        // empty schematic). Dismiss via OK button or clicking outside.
        if document.export_error.is_some() {
            layers.push(Self::dismiss_layer(Message::DismissExportError));
            layers.push(self.view_export_error());
        }

        // Print preview overlay — Altium parity: opens as a separate OS
        // window (see `handle_print_preview_requested → handle_detach_modal`)
        // so it can be dragged outside the app's client area. Only fall
        // back to the in-window overlay if the OS window failed to open.
        let preview_detached = ui
            .windows
            .values()
            .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(super::state::ModalId::PrintPreview)));
        if document.preview.is_some() && !preview_detached {
            layers.push(self.view_print_preview());
        }

        // BOM preview overlay — same detach-first pattern as Print Preview.
        let bom_detached = ui
            .windows
            .values()
            .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(super::state::ModalId::BomPreview)));
        if document.bom_preview.is_some() && !bom_detached {
            layers.push(self.view_bom_preview());
        }

        // Custom net-colour picker. Bespoke modal (not the iced_aw
        // ColorPicker) because the user needs a quick-pick palette +
        // precise RGB inputs side-by-side.
        if ui.net_color_custom.show {
            layers.push(Self::dismiss_layer(Message::NetColorCustomShow(false)));
            layers.push(self.view_net_color_custom_picker());
        }

        // Blocking modals must own the overlay stack. If we keep adding
        // tool/menu overlays after these, they can end up visually above
        // the modal and make the dialog look broken.
        let has_blocking_modal = document.export_error.is_some()
            || document.preview.is_some()
            || ui.net_color_custom.show;
        if has_blocking_modal {
            return layers;
        }

        // Altium-style pause overlay: big centered "Placement Paused" card
        // with a Resume button. Clicking Resume clears `pre_placement`,
        // un-pauses the canvas, and drops back to the active placement tool
        // so the user can keep dropping objects with the edited properties.
        if interaction.canvas.placement_paused {
            let tokens = &document.panel_ctx.tokens;
            let panel_bg = crate::styles::ti(tokens.panel_bg);
            let text_c = crate::styles::ti(tokens.text);
            let accent_c = crate::styles::ti(tokens.accent);
            let border_c = crate::styles::ti(tokens.border);
            let card = container(
                column![
                    iced::widget::text("⏸").size(64).color(accent_c),
                    iced::widget::text("Placement Paused")
                        .size(16)
                        .color(text_c),
                    iced::widget::text(
                        "Editing properties in the panel. Click Resume to keep placing."
                    )
                    .size(11)
                    .color(text_c),
                    iced::widget::Space::new().height(6.0),
                    iced::widget::button(
                        iced::widget::text("Resume Placement")
                            .size(12)
                            .color(iced::Color::WHITE)
                    )
                    .padding([6, 18])
                    .on_press(Message::ResumePlacement)
                    .style(iced::widget::button::primary),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .padding(24)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(iced::Color {
                    a: 0.92,
                    ..panel_bg
                })),
                border: iced::Border {
                    color: border_c,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            });
            layers.push(
                container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into(),
            );
        }

        if self.has_active_schematic() {
            let y_offset: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 };
            // Active Bar overlay is only painted on the main window, so
            // the main canvas's selection set is the right gate.
            let bar_has_selection = !interaction.canvas.selected.is_empty();
            let bar_has_net_colors = !ui.net_colors.is_empty();
            let bar = crate::active_bar::view_bar(
                interaction.current_tool,
                interaction.draw_mode,
                &interaction.last_tool,
                &document.panel_ctx.tokens,
                self.ui_state.theme_id,
                bar_has_selection,
                bar_has_net_colors,
            )
            .map(Message::ActiveBar);
            layers.push(
                column![
                    iced::widget::Space::new().height(y_offset + 4.0),
                    container(bar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                ]
                .into(),
            );
        }

        if self.has_active_schematic()
            && let Some(ref edit_state) = interaction.editing_text
        {
            let text = edit_state.text.clone();
            // Convert object world position → window-absolute screen position.
            // The canvas Program publishes its latest camera into this Cell each
            // frame — that's the only way to read it from outside the Program.
            let (cam_off_x, cam_off_y, cam_scale) = interaction.canvas.live_camera.get();
            let canvas_local_x = edit_state.world_x as f32 * cam_scale + cam_off_x;
            let canvas_local_y = edit_state.world_y as f32 * cam_scale + cam_off_y;
            // Canvas top-left within the window: menu bar + tab bar above,
            // left dock + left resize handle (5px when shown) to the side.
            let tabs_h: f32 = if document.tabs.is_empty() { 0.0 } else { 28.0 };
            let y_canvas_origin: f32 = crate::menu_bar::MENU_BAR_HEIGHT + tabs_h;
            let has_left = document.dock.has_panels(PanelPosition::Left);
            let left_col = document.dock.is_collapsed(PanelPosition::Left);
            let left_dock_w: f32 = if !has_left {
                0.0
            } else if left_col {
                28.0
            } else {
                ui.left_width
            };
            let left_handle_w: f32 = if has_left && !left_col { 5.0 } else { 0.0 };
            let x_canvas_origin: f32 = left_dock_w + left_handle_w;
            // Font size in pixels matches the rendered label (10 pt ≈ 1.8 mm).
            let font_px = (cam_scale * 1.8).clamp(10.0, 64.0);
            // Estimate width from text length to keep the input snug.
            let approx_w =
                ((edit_state.text.chars().count() as f32 + 2.0) * font_px * 0.62).max(60.0);
            // Offset the input so the baseline sits on top of the label text.
            let abs_x = x_canvas_origin + canvas_local_x - 2.0;
            let abs_y = y_canvas_origin + canvas_local_y - font_px - 2.0;
            let paper_c = crate::styles::ti(document.panel_ctx.tokens.paper);
            let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
            let accent_c = crate::styles::ti(document.panel_ctx.tokens.accent);
            layers.push(
                column![
                    iced::widget::Space::new().height(abs_y.max(0.0)),
                    row![
                        iced::widget::Space::new().width(abs_x.max(0.0)),
                        container(
                            iced::widget::text_input("", &text)
                                .on_input(Message::TextEditChanged)
                                .on_submit(Message::TextEditSubmit)
                                .size(font_px)
                                .padding([1, 2])
                                .width(approx_w)
                                .style(move |_: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: iced::Background::Color(paper_c),
                                        border: iced::Border {
                                            color: accent_c,
                                            width: 1.0,
                                            radius: 0.0.into(),
                                        },
                                        icon: text_c,
                                        placeholder: text_c,
                                        value: text_c,
                                        selection: accent_c,
                                    }
                                }),
                        ),
                    ],
                ]
                .into(),
            );
        }

        if let Some(ab_menu) = interaction.active_bar_menu {
            let has_selection = !interaction.canvas.selected.is_empty();
            let has_net_colors = !ui.net_colors.is_empty();
            let dropdown = crate::active_bar::view_dropdown(
                ab_menu,
                &document.panel_ctx.tokens,
                &interaction.selection_filters,
                &interaction.custom_filter_presets,
                self.ui_state.theme_id,
                has_selection,
                has_net_colors,
            )
            .map(Message::ActiveBar);
            let x_off = crate::active_bar::dropdown_x_offset(ab_menu);
            // Bar: MENU_BAR_HEIGHT + tabs + 4 top-margin + bar-height ≈ bottom of bar.
            // Bar-height = 28 button + 6 vertical padding + 2 border = 36, plus 4
            // top margin = 40. Add a small gap so the dropdown visually touches.
            let ab_y: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if document.tabs.is_empty() { 0.0 } else { 28.0 }
                + 40.0;
            let bar_w: f32 = crate::active_bar::BAR_WIDTH_PX;
            let (ww, _) = ui.window_size;
            let adjusted_x = x_off + (ww - bar_w) / 2.0;

            layers.push(Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )));
            // Absolute-position the dropdown with Translate so the
            // column can auto-size to its widest label. The old
            // column+row+Space wrapping forced a fixed-width column
            // which clipped labels like "Elliptical Arc".
            layers
                .push(super::view::translate::Translate::new(dropdown, (adjusted_x, ab_y)).into());
        }

        if let Some(ref ctx_menu) = interaction.context_menu {
            let menu = self.view_context_menu();
            // Clamp the menu inside the window so a click near the
            // right/bottom edge doesn't push it off-screen. Estimate
            // the menu's footprint conservatively from the maximum
            // possible row count (≈ 22 rows × 22 px + padding) and
            // CONTEXT_MENU_WIDTH; flip-up / flip-left when the click
            // lands too close to an edge.
            let (win_w, win_h) = self.ui_state.window_size;
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 22.0 * 22.0 + 8.0;
            let edge_margin: f32 = 4.0;
            let x = if ctx_menu.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                ctx_menu.x
            };
            let y = if ctx_menu.y + est_menu_h + edge_margin > win_h {
                (ctx_menu.y - est_menu_h).max(0.0)
            } else {
                ctx_menu.y
            };
            layers.push(Self::dismiss_layer(Message::CloseContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
            // Submenu (Place / Align) — pop to the right of the parent
            // menu (or left if the right edge would overflow), and
            // align its top to the launcher row's y-position so the
            // first submenu item sits next to the row that opened it.
            if let Some(submenu_kind) = interaction.context_submenu {
                let submenu = self.view_context_submenu(submenu_kind);
                // Wrap in mouse_area so on_enter/on_exit on the panel
                // can extend the close timer when the cursor crosses
                // from the launcher into the submenu and back.
                let submenu = iced::widget::mouse_area(submenu)
                    .on_enter(Message::EnterContextSubmenuPanel)
                    .on_exit(Message::LeaveContextSubmenuPanel);
                let submenu_w = menu_w;
                let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                    (x - submenu_w).max(0.0)
                } else {
                    x + menu_w
                };
                // Approximate launcher-row y inside the parent menu.
                // Each ctx_menu_item_* row is ≈ 22 px tall (text + 4 px
                // top + 4 px bottom + a tiny line-height fudge); the
                // separator is rendered as a 1 px line. The numbers
                // below come from counting rows above each launcher in
                // `view_context_menu`.
                const ROW_H: f32 = 22.0;
                const SEP_H: f32 = 1.0;
                const TOP_PAD: f32 = 4.0;
                let launcher_y = match submenu_kind {
                    // Above Place: 3 always-visible rows + 1 separator.
                    ContextSubmenu::Place => TOP_PAD + 3.0 * ROW_H + SEP_H,
                    // Align is only shown when something is selected;
                    // above Align: the same 3 rows + 1 sep, then
                    // Place / Part Actions / Sheet Actions / References.
                    ContextSubmenu::Align => TOP_PAD + 7.0 * ROW_H + SEP_H,
                    // AddNewToProject only fires from the project-tree
                    // menu, never from the canvas menu — fall through
                    // to a safe placeholder if the state somehow leaks
                    // (no submenu rendered, just a 0-offset).
                    ContextSubmenu::AddNewToProject => 0.0,
                };
                let sub_y = (y + launcher_y - 4.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(sub_y),
                        row![
                            iced::widget::Space::new().width(sub_x),
                            submenu,
                            iced::widget::Space::new().width(Length::Fill),
                        ]
                        .width(Length::Fill),
                    ]
                    .into(),
                );
            }
        }

        // Document-tab right-click menu. Rendered before the project-
        // tree menu since the two are mutually exclusive — only one of
        // them can be open at a time, and opening one closes the
        // others (see Message::ShowTabContextMenu).
        if let Some(ref tab_ctx) = interaction.tab_context_menu {
            let menu = self.view_tab_context_menu(tab_ctx);
            // Conservative footprint matches the project-tree menu so
            // the two visually align.
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 5.0 * 22.0 + 8.0;
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if tab_ctx.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                tab_ctx.x
            };
            let y = if tab_ctx.y + est_menu_h + edge_margin > win_h {
                (tab_ctx.y - est_menu_h).max(0.0)
            } else {
                tab_ctx.y
            };
            layers.push(Self::dismiss_layer(Message::CloseTabContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
        }

        // Projects-panel tree right-click menu. Rendered here (after the
        // canvas context menu) so the canvas menu's dismiss layer does
        // not cover this one — the two are mutually exclusive in
        // practice since `ShowProjectTreeContextMenu` nulls out
        // `context_menu` before opening.
        if let Some(ref tree_ctx) = interaction.project_tree_context_menu {
            let menu = self.view_project_tree_context_menu(tree_ctx);
            // Conservative footprint: at most 6 rows × 22 px + 8 px
            // padding. Width matches the canvas menu so the two look
            // consistent.
            let menu_w = Self::CONTEXT_MENU_WIDTH as f32;
            let est_menu_h: f32 = 6.0 * 22.0 + 8.0;
            let (win_w, win_h) = ui.window_size;
            let edge_margin: f32 = 4.0;
            let x = if tree_ctx.x + menu_w + edge_margin > win_w {
                (win_w - menu_w - edge_margin).max(0.0)
            } else {
                tree_ctx.x
            };
            let y = if tree_ctx.y + est_menu_h + edge_margin > win_h {
                (tree_ctx.y - est_menu_h).max(0.0)
            } else {
                tree_ctx.y
            };
            layers.push(Self::dismiss_layer(Message::CloseProjectTreeContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(y),
                    row![
                        iced::widget::Space::new().width(x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
            // Adjacent submenu (currently only AddNewToProject opens
            // from this menu). Mirrors the canvas-menu submenu logic
            // above — pop to the right of the parent (or left if the
            // right edge would overflow), align top to the launcher
            // row's y inside the parent menu.
            if let Some(ContextSubmenu::AddNewToProject) = interaction.context_submenu {
                let submenu = self.view_context_submenu(ContextSubmenu::AddNewToProject);
                let submenu = iced::widget::mouse_area(submenu)
                    .on_enter(Message::EnterContextSubmenuPanel)
                    .on_exit(Message::LeaveContextSubmenuPanel);
                let submenu_w = menu_w;
                let sub_x = if x + menu_w + submenu_w + edge_margin > win_w {
                    (x - submenu_w).max(0.0)
                } else {
                    x + menu_w
                };
                // Launcher position inside the project-tree menu:
                // `Make Project Available Online...` (row 0)
                // `Validate Project`                 (row 1)
                // `Add New to Project ›`             (row 2) ← target
                // → top + 2 rows, no separator above the launcher.
                const ROW_H: f32 = 22.0;
                const TOP_PAD: f32 = 4.0;
                let launcher_y = TOP_PAD + 2.0 * ROW_H;
                let sub_y = (y + launcher_y - 4.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(sub_y),
                        row![
                            iced::widget::Space::new().width(sub_x),
                            submenu,
                            iced::widget::Space::new().width(Length::Fill),
                        ]
                        .width(Length::Fill),
                    ]
                    .into(),
                );
            }
        }

        if ui.panel_list_open {
            let text_c = crate::styles::ti(document.panel_ctx.tokens.text);
            let text_muted = crate::styles::ti(document.panel_ctx.tokens.text_secondary);
            let has_sch = document.panel_ctx.has_schematic;
            let has_pcb = document.panel_ctx.has_pcb;
            // Build a lookup of currently-open panel kinds so each row
            // can show a ✓ mark. A panel counts as "open" if it lives in
            // any dock region, floats on top, or owns a detached OS
            // window.
            let docked: std::collections::HashSet<crate::panels::PanelKind> = [
                crate::dock::PanelPosition::Left,
                crate::dock::PanelPosition::Right,
                crate::dock::PanelPosition::Bottom,
            ]
            .iter()
            .flat_map(|pos| document.dock.panel_kinds(*pos).to_vec())
            .collect();
            let floating: std::collections::HashSet<crate::panels::PanelKind> =
                document.dock.floating.iter().map(|fp| fp.kind).collect();
            let detached: std::collections::HashSet<crate::panels::PanelKind> = ui
                .windows
                .values()
                .filter_map(|w| match w {
                    super::state::WindowKind::DetachedPanel(k) => Some(*k),
                    _ => None,
                })
                .collect();
            let is_open = |k: crate::panels::PanelKind| {
                docked.contains(&k) || floating.contains(&k) || detached.contains(&k)
            };
            let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&kind| {
                    (!kind.needs_schematic() || has_sch) && (!kind.needs_pcb() || has_pcb)
                })
                .map(|&kind| {
                    // Altium parity: a leading ✓ column marks open panels
                    // so the user can see at a glance which ones are
                    // already somewhere on screen. Clicking an open panel
                    // still fires OpenPanel — the dock brings it forward.
                    let check = if is_open(kind) { "\u{2713}" } else { "" };
                    iced::widget::button(
                        iced::widget::row![
                            iced::widget::container(
                                iced::widget::text(check.to_string())
                                    .size(11)
                                    .color(text_muted),
                            )
                            .width(Length::Fixed(16.0)),
                            iced::widget::text(kind.label().to_string())
                                .size(11)
                                .color(text_c),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 12])
                    .width(Length::Fill)
                    .on_press(Message::OpenPanel(kind))
                    .style(crate::styles::menu_item(&document.panel_ctx.tokens))
                    .into()
                })
                .collect();

            // Drop the scrollable wrapper — the list fits the window at
            // full height (15-ish panels × 21 px each = ~315 px) and a
            // menu-style popup reads cleaner without a scrollbar.
            let popup = container(column(panel_items).spacing(0).width(210))
                .padding([6, 0])
                .style(crate::styles::context_menu(&document.panel_ctx.tokens));

            layers.push(Self::dismiss_layer(Message::TogglePanelList));
            // Anchor the popup directly above the "Panels" button in the
            // bottom-right of the status bar. Approx: popup 210 px wide,
            // 22 px per row × visible rows + 12 px vertical padding.
            // Status bar sits at y = wh - 22, so we place the popup so
            // its bottom edge lands just above it.
            let (ww, wh) = ui.window_size;
            let visible_rows = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&k| (!k.needs_schematic() || has_sch) && (!k.needs_pcb() || has_pcb))
                .count() as f32;
            let popup_w = 210.0_f32;
            let popup_h = visible_rows * 22.0 + 12.0;
            let left = (ww - popup_w - 10.0).max(0.0);
            let top = (wh - popup_h - 26.0).max(0.0);
            layers.push(translate::Translate::new(Element::from(popup), (left, top)).into());
        }

        if let Some(fp) = document.dock.floating.iter().find(|fp| fp.dragging) {
            let (ww, wh) = ui.window_size;
            let zone = 120.0;
            let cx = fp.x + fp.width / 2.0;
            let cy = fp.y + fp.height / 4.0;
            let zone_style = crate::styles::dock_zone_highlight(&document.panel_ctx.tokens);
            if cx < zone {
                layers.push(
                    container(iced::widget::Space::new())
                        .width(ui.left_width)
                        .height(Length::Fill)
                        .style(zone_style)
                        .into(),
                );
            } else if cx > ww - zone {
                layers.push(
                    row![
                        iced::widget::Space::new().width(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(ui.right_width)
                            .height(Length::Fill)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            } else if cy > wh - zone {
                layers.push(
                    column![
                        iced::widget::Space::new().height(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(ui.bottom_height)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            }
        }

        for i in 0..document.dock.floating.len() {
            if let Some(panel_widget) = document.dock.view_floating_panel(i, &document.panel_ctx) {
                let fp = &document.dock.floating[i];
                // No clamp — panels follow Altium behaviour and may be
                // dragged anywhere, even past the window edge. The OS clips
                // at the window boundary; within that, Translate renders
                // the panel at fp.(x, y) without resizing it.
                layers.push(
                    translate::Translate::new(panel_widget.map(Message::Dock), (fp.x, fp.y)).into(),
                );
            }
        }

        if ui.preferences_open {
            let pref_view = crate::preferences::view(
                ui.preferences_nav,
                ui.preferences_draft_theme,
                ui.theme_id,
                &ui.preferences_draft_font,
                ui.preferences_draft_power_port_style,
                ui.preferences_draft_label_style,
                ui.preferences_draft_multisheet_style,
                ui.preferences_draft_grid_style,
                ui.custom_theme.as_ref().map(|c| c.name.as_str()),
                ui.preferences_dirty,
                &ui.erc_severity_override,
            )
            .map(Message::PreferencesMsg);
            layers.push(pref_view);
        }

        if ui.find_replace.open {
            let dialog = crate::find_replace::view(&ui.find_replace, &document.panel_ctx.tokens)
                .map(Message::FindReplaceMsg);
            layers.push(dialog);
        }

        if ui.rename_dialog.is_some() {
            layers.push(self.view_rename_dialog());
        }
        if ui.remove_dialog.is_some() {
            layers.push(self.view_remove_dialog());
        }
        if ui.project_close_confirm.is_some() {
            layers.push(self.view_project_close_confirm());
        }

        // Skip overlay rendering for any modal whose detached OS window
        // owns the view. Without this guard the user sees the modal in
        // both the main window and the popped-out window at the same
        // time.
        let modal_detached = |m: super::state::ModalId| -> bool {
            ui.windows
                .values()
                .any(|kind| matches!(kind, super::state::WindowKind::DetachedModal(x) if *x == m))
        };

        if ui.annotate_dialog_open && !modal_detached(super::state::ModalId::AnnotateDialog) {
            layers.push(self.view_annotate_dialog());
        }
        if ui.annotate_reset_confirm && !modal_detached(super::state::ModalId::AnnotateResetConfirm)
        {
            layers.push(self.view_annotate_reset_confirm());
        }
        if ui.erc_dialog_open && !modal_detached(super::state::ModalId::ErcDialog) {
            layers.push(self.view_erc_dialog());
        }

        layers
    }
}
