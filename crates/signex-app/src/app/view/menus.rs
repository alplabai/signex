use iced::widget::{column, container};
use iced::{Element, Length};

use super::*;

const SUBMENU_ARROW: &str = "›";
const SUBMENU_ARROW_SIZE: f32 = 18.0;

impl Signex {
    #[allow(clippy::vec_init_then_push)]
    pub(super) fn view_context_menu(&self) -> Element<'_, Message> {
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
    pub(super) fn view_context_submenu(&self, kind: ContextSubmenu) -> Element<'_, Message> {
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

}
