//! Right-click context-menu builders for the canvas and tab strip, plus
//! the grid-picker popup. (The project-tree menu lives in the sibling
//! `project_tree` module.)
//!
//! Data-to-view (#269): each menu is assembled as a pure
//! `Vec<DropdownEntry<Message>>` and handed to the shared
//! `signex_widgets::active_bar_dropdown` widget for rendering, so the
//! canvas / project-tree / tab menus share ONE row renderer with the
//! schematic + footprint active bars (ADR-0003). The pure `*_entries`
//! builders carry the data (labels, actions, enable state) and are unit-
//! tested without a GPU; the thin `&self` `view_*` shims resolve app
//! state (selection, shortcuts) and call the widget.

use super::*;

use super::items::{
    command_text, dd_cmd_disabled, dd_command, dd_disabled, dd_msg, dd_planned, dd_planned_because,
    dd_planned_submenu, dd_ruled, submenu_launcher, text,
};
use crate::app::command::CommandContext;
use crate::icons as ic;
use iced::widget::svg::Handle;
use signex_i18n::{I18n, MessageId, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

pub(super) use super::board_menu::pcb_canvas_menu_with_blocks;

/// Command ids whose keyboard hint the canvas menu shows.
pub(super) const CANVAS_SHORTCUT_IDS: &[&str] = &[
    "find",
    "find_similar_objects",
    "clear_net_highlighting",
    "undo",
    "redo",
    "highlight_net_under_cursor",
    "open_net_color_palette",
    "assign_net_class",
    "cut",
    "copy",
    "paste",
    "smart_paste",
    "duplicate",
    "select_all",
    "rotate_counterclockwise",
    "rotate_clockwise",
    "move_selection",
    "flip_selection",
    "mirror_x",
    "mirror_y",
    "delete_selection",
    "open_child_sheet",
    "create_sheet_from_sheet_symbol",
    "sync_sheet_pins",
    "sync_sheet_entries",
    "create_sheet_symbol_from_sheet",
    "open_preferences",
    "open_datasheet",
    "edit_selected_symbol_in_symbol_editor",
    "edit_selected_object_properties",
    "change_component",
    "open_project_options",
    "cross_probe",
    "board_sketch",
    "change_footprint",
    "lock_board_items",
    "unlock_board_items",
    // The board menu's own rows (#2405).
    "select_board_net",
    "highlight_board_net",
    "toggle_ratsnest",
    "gloss_selected_routes",
    "add_teardrops",
    "board_shape_from_selection",
    "keepout_from_selection",
    "arrange_in_box",
    "swap_pins",
    "swap_gates",
    "save_board_reuse_block",
];

/// Keyboard-shortcut hints for the canvas menu, keyed by command id and
/// resolved from the active keymap profile (empty when the profile does not
/// bind the command) before the pure builder runs.
#[derive(Debug, Clone, Default)]
pub(super) struct CanvasShortcuts(std::collections::HashMap<&'static str, String>);

impl CanvasShortcuts {
    /// Hints for every id in [`CANVAS_SHORTCUT_IDS`], looked up with `label_of`.
    pub(super) fn resolve(label_of: impl Fn(&str) -> String) -> Self {
        Self(
            CANVAS_SHORTCUT_IDS
                .iter()
                .map(|id| (*id, label_of(id)))
                .collect(),
        )
    }

    /// The hint for `command_id`; empty when unbound.
    pub(super) fn of(&self, command_id: &str) -> &str {
        self.0.get(command_id).map_or("", String::as_str)
    }

    #[cfg(test)]
    pub(super) fn set(&mut self, command_id: &'static str, hint: &str) {
        self.0.insert(command_id, hint.to_string());
    }
}

impl Signex {
    /// The selection shape and project row key the canvas menu is built
    /// from; the view and the row-effect tests share it. The selection is
    /// the one of the sheet `window_id` shows (#1910).
    pub(super) fn canvas_menu_inputs(
        &self,
        window_id: iced::window::Id,
    ) -> (CanvasSelection, Option<Vec<String>>) {
        let canvas = self.interaction_state.canvas_for_window(window_id);
        let selection = CanvasSelection {
            has_selection: !canvas.selected.is_empty(),
            child_sheet_selected: canvas
                .selected
                .iter()
                .any(|item| item.kind == signex_types::schematic::SelectedKind::ChildSheet),
            drawing_selected: canvas
                .selected
                .iter()
                .any(|item| item.kind == signex_types::schematic::SelectedKind::Drawing),
            placed_part_selected: self
                .document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some_and(|engine| {
                    crate::app::handlers::placed_part::selected_placed_symbol(engine, canvas)
                        .is_some()
                }),
            // The window's own facts, so a row greys exactly when the menu
            // bar's row and the keystroke for the same command would.
            commands: self.command_context(Some(window_id)),
        };
        // The project row's tree key, the same one the project tree's own
        // menu sends, so the action follows the project if rows reorder.
        let project_key = self
            .document_state
            .active_document_project()
            .map(|p| vec![p.id.to_string()]);
        (selection, project_key)
    }

    /// Canvas right-click menu. Resolves selection state + shortcut hints
    /// from `self`, then delegates to the pure [`canvas_menu_entries`].
    pub(in crate::app::view) fn view_context_menu(
        &self,
        window_id: iced::window::Id,
    ) -> super::placement::MenuCard<'_> {
        let tid = self.ui_state.effective_theme_id();
        // Shortcut hints read from the active keymap profile; a command
        // it does not bind has none.
        let sc = CanvasShortcuts::resolve(|id| self.keymap_shortcut_label(id));
        let tokens = &self.document_state.panel_ctx.tokens;
        let ui_font = &self.document_state.panel_ctx.ui_font_name;
        let active_submenu = self.interaction_state.context_submenu;
        let context = self.command_context(Some(window_id));
        if context.pcb.shown {
            let blocks = self.reuse_block_names_in_window(Some(window_id));
            let entries = pcb_canvas_menu_with_blocks(
                &self.ui_state.i18n,
                tid,
                &context,
                active_submenu,
                &sc,
                &blocks,
            );
            return super::placement::menu_card(entries, tokens, ui_font);
        }
        let (selection, project_key) = self.canvas_menu_inputs(window_id);
        let entries = canvas_menu_entries(
            &self.ui_state.i18n,
            tid,
            selection,
            active_submenu,
            project_key,
            &sc,
        );
        super::placement::menu_card(entries, tokens, ui_font)
    }

    /// Build the document-tab right-click menu. Resolves the tab title and
    /// undock state from `self`, then delegates to [`tab_menu_entries`].
    pub(in crate::app::view) fn view_tab_context_menu(
        &self,
        ctx: &crate::app::TabContextMenuState,
    ) -> Element<'_, Message> {
        let Some(tab) = self.document_state.tabs.get(ctx.tab_idx) else {
            {
                use iced::widget::Widget as _;
                return container(iced::widget::Space::new().boxed()).boxed();
            }
        };
        // A tab can be undocked iff it's not already living in its own OS
        // window. Single-tab workspaces still show the row: undocking a
        // sole tab leaves the main window empty.
        let already_undocked = self.tab_is_undocked(&tab.path);
        let close_shortcut = self.keymap_shortcut_label("close_active_document");
        let entries = tab_menu_entries(
            &self.ui_state.i18n,
            self.ui_state.effective_theme_id(),
            &TabMenuFacts {
                title: &tab.title,
                tab_idx: ctx.tab_idx,
                total_tabs: self.document_state.tabs.len(),
                strip_tabs_to_the_right: !self.strip_tabs_right_of(ctx.tab_idx).is_empty(),
                // A Project tab stays docked (#1202): an undocked window
                // would take the main window's shortcuts.
                undock_unavailable: already_undocked || tab.kind.is_project_page(),
                has_file: crate::tab_bar::tab_file_path(tab).is_some(),
                in_project: self.project_tree_key_of_file(&tab.path).is_some(),
                close_shortcut: &close_shortcut,
            },
        );
        let panel_ctx = &self.document_state.panel_ctx;
        super::placement::menu_card(entries, &panel_ctx.tokens, &panel_ctx.ui_font_name).element
    }

    /// The marks of the tab strip in `window_id`. The strip and its "+N"
    /// menu read the same marks, so the menu lists exactly the tabs the
    /// strip leaves out.
    pub(in crate::app::view) fn tab_strip_marks<'p>(
        &'p self,
        window_id: iced::window::Id,
        dirty_paths: &'p std::collections::HashSet<std::path::PathBuf>,
        read_only_paths: &'p std::collections::HashSet<std::path::PathBuf>,
    ) -> crate::tab_bar::TabMarks<'p> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let overflow_open = ui.main_window_id == Some(window_id) && ui.tab_overflow_menu_open;
        crate::tab_bar::TabMarks {
            dirty_paths,
            read_only_paths,
            // A tooltip hangs below the tab and would cover an open menu.
            tooltips_enabled: self.interaction_state.tab_context_menu.is_none() && !overflow_open,
            project_name: document
                .active_document_project()
                .map(|p| p.data.name.as_str()),
            i18n: &ui.i18n,
            projects: &document.projects,
            ui_font: crate::fonts::iced_font_for_family(&document.panel_ctx.ui_font_name),
            theme_id: ui.effective_theme_id(),
            strip_width: ui.size_of_window(window_id).0,
            overflow_open,
        }
    }

    /// The tab strip's "+N" menu (mockup row 5): the click-outside layer, then the
    /// card under the button listing the hidden tabs. Only the main window's
    /// strip has the button; empty when the menu is closed or nothing hides.
    pub(in crate::app::view) fn tab_overflow_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let Some(main) = ui.main_window_id.filter(|main| *main == window_id) else {
            return Vec::new();
        };
        if !ui.tab_overflow_menu_open {
            return Vec::new();
        }
        let document = &self.document_state;
        let tabs = &document.tabs;
        let undocked: std::collections::HashSet<std::path::PathBuf> = ui
            .windows
            .values()
            .filter_map(|kind| match kind {
                super::super::state::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let visible: Vec<usize> = (0..tabs.len())
            .filter(|&i| !undocked.contains(&tabs[i].path))
            .collect();
        let dirty = crate::tab_bar::dirty_tab_paths(self, tabs);
        let read_only = crate::tab_bar::read_only_tab_paths(tabs, &document.engines);
        let marks = self.tab_strip_marks(window_id, &dirty, &read_only);
        let fit =
            crate::tab_bar::overflow::strip_split(tabs, &visible, document.active_tab, &marks);
        if fit.hidden.is_empty() {
            return Vec::new();
        }
        let tokens = &document.panel_ctx.tokens;
        let titled = (tabs.as_slice(), document.projects.as_slice());
        let entries = crate::tab_bar::overflow::hidden_tab_entries(
            titled,
            &fit.hidden,
            &ui.i18n,
            tokens,
            |i| Message::Tab {
                window_id: main,
                msg: crate::tab_bar::TabMessage::OverflowTabPicked(i),
            },
        );
        let width = crate::tab_bar::overflow::hidden_menu_width(
            titled,
            &fit.hidden,
            &ui.i18n,
            marks.ui_font,
        );
        let anchor = (
            crate::tab_bar::overflow::button_left(fit.hidden.len(), &marks),
            super::super::canvas_region::chrome_top(tokens),
        );
        let card = signex_widgets::active_bar_dropdown::view(entries, tokens, Some(width));
        Self::menu_overlay(
            Message::Tab {
                window_id: main,
                msg: crate::tab_bar::TabMessage::OverflowMenuDismissed,
            },
            card,
            anchor,
        )
    }
}

/// What the canvas right-click landed on, for [`canvas_menu_entries`].
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CanvasSelection {
    pub(super) has_selection: bool,
    pub(super) child_sheet_selected: bool,
    pub(super) drawing_selected: bool,
    /// One non-power symbol: the placed-part actions apply (#1260).
    pub(super) placed_part_selected: bool,
    /// The window's command facts: every command row is live exactly when
    /// its own enablement rule allows it here (#2419).
    pub(super) commands: CommandContext,
}

/// Pure canvas-menu data builder. Rows and their enable state are derived
/// only from the passed selection flags, command facts and shortcut hints,
/// so the menu's shape is unit-testable without a window. Which rows show
/// follows the selection; whether a shown row is live follows the
/// command's own rule (#2419).
pub(super) fn canvas_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    selection: CanvasSelection,
    active_submenu: Option<ContextSubmenu>,
    project_key: Option<signex_widgets::tree_view::TreeKey>,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    let CanvasSelection {
        has_selection,
        drawing_selected,
        commands,
        ..
    } = selection;
    let ruled = |icon: Option<Handle>, command_id: &'static str| {
        dd_ruled(i18n, icon, command_id, sc.of(command_id), &commands, true)
    };
    let launcher = |icon: Handle, label: MessageId, kind: ContextSubmenu| {
        submenu_launcher(
            Some(icon),
            &text(i18n, label),
            kind,
            active_submenu == Some(kind),
        )
    };
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(40);

    // The sheet menu seeds from what is under the cursor, so its row is
    // live without a selection (#2416).
    v.push(dd_command(
        i18n,
        Some(ic::icon_dd_find_similar(tid)),
        "find_similar_objects",
        sc.of("find_similar_objects"),
        None,
    ));
    v.push(ruled(Some(ic::icon_chrome_search(tid)), "find"));
    // Ends the net highlight, which dims everything off the net.
    v.push(ruled(
        Some(ic::icon_dd_clear_filter(tid)),
        "clear_net_highlighting",
    ));
    v.push(DropdownEntry::Separator);
    v.push(launcher(
        ic::icon_dd_place_menu(tid),
        menu_ids::MENU_PLACE,
        ContextSubmenu::Place,
    ));
    v.push(launcher(
        ic::icon_dd_part_actions(tid),
        menu_ids::CTX_PART_ACTIONS,
        ContextSubmenu::PartActions,
    ));
    v.push(launcher(
        ic::icon_dd_sheet_actions(tid),
        menu_ids::CTX_SHEET_ACTIONS,
        ContextSubmenu::SheetActions,
    ));

    if has_selection {
        v.push(dd_planned_because(
            i18n,
            Some(ic::icon_dd_references(tid)),
            menu_ids::CTX_REFERENCES,
            menu_ids::CTX_REFERENCES_PLANNED,
            true,
        ));
        v.push(launcher(
            ic::icon_dd_align_menu(tid),
            menu_ids::CTX_ALIGN,
            ContextSubmenu::Align,
        ));
        if drawing_selected {
            v.push(launcher(
                ic::icon_dd_align_menu(tid),
                menu_ids::MENU_EDIT_ARRANGE,
                ContextSubmenu::Arrange,
            ));
        }
        v.push(dd_planned_because(
            i18n,
            Some(ic::icon_dd_unions(tid)),
            menu_ids::CTX_UNIONS,
            menu_ids::CTX_UNIONS_PLANNED,
            true,
        ));
        v.push(dd_planned_submenu(
            i18n,
            Some(ic::icon_dd_snippets(tid)),
            &text(i18n, menu_ids::CTX_SNIPPETS),
        ));
    }

    // #1312: the selected part or net on the board, opened when needed.
    v.push(ruled(Some(ic::icon_dd_cross_probe(tid)), "cross_probe"));
    v.push(DropdownEntry::Separator);
    v.push(ruled(Some(ic::icon_undo(tid)), "undo"));
    v.push(ruled(Some(ic::icon_redo(tid)), "redo"));
    v.push(DropdownEntry::Separator);
    v.extend(clipboard_rows(tid, &ruled));
    v.push(DropdownEntry::Separator);

    if has_selection {
        // Greyed when everything selected is locked: the edit would leave
        // the sheet as it is (#2419).
        v.push(ruled(
            Some(ic::icon_dd_rotate(tid)),
            "rotate_counterclockwise",
        ));
        v.push(ruled(Some(ic::icon_dd_flip_x(tid)), "mirror_x"));
        v.push(ruled(Some(ic::icon_dd_flip_y(tid)), "mirror_y"));
        v.push(ruled(Some(ic::icon_dd_delete(tid)), "delete_selection"));
        v.push(DropdownEntry::Separator);
    }

    v.extend(net_rows(i18n, tid, sc, &ruled));
    v.push(DropdownEntry::Separator);

    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_comment(tid)),
        &text(i18n, menu_ids::CTX_COMMENT),
        None,
    ));
    v.push(dd_planned_because(
        i18n,
        Some(ic::icon_dd_pin_mapping(tid)),
        menu_ids::CTX_PIN_MAPPING,
        menu_ids::CTX_PIN_MAPPING_PLANNED,
        false,
    ));
    // The project that owns the active sheet, addressed by its tree key the
    // way the project tree's own menu addresses it. No project: greyed, no
    // tooltip (the feature exists, there is just nothing to open it for).
    v.push(match project_key {
        Some(key) => dd_command(
            i18n,
            Some(ic::icon_dd_project_options(tid)),
            "open_project_options",
            sc.of("open_project_options"),
            key.first().cloned(),
        ),
        None => dd_cmd_disabled(
            i18n,
            Some(ic::icon_dd_project_options(tid)),
            "open_project_options",
            sc.of("open_project_options"),
        ),
    });
    v.push(ruled(
        Some(ic::icon_dd_preferences(tid)),
        "open_preferences",
    ));

    if has_selection {
        v.push(dd_planned_because(
            i18n,
            Some(ic::icon_dd_supplier_links(tid)),
            menu_ids::CTX_SUPPLIER_LINKS,
            menu_ids::CTX_SUPPLIER_LINKS_PLANNED,
            false,
        ));
        // Properties → ensure the Properties panel is visible; it already
        // tracks the current selection, so it populates with the
        // right-clicked item's fields once shown.
        v.push(ruled(
            Some(ic::icon_dd_properties(tid)),
            "edit_selected_object_properties",
        ));
    }

    v
}

/// The clipboard group, each row live by its own rule: Cut and Copy need a
/// selection (Cut one that is not all locked), Paste a sheet, Select All a
/// sheet.
fn clipboard_rows(
    tid: ThemeId,
    ruled: &dyn Fn(Option<Handle>, &'static str) -> DropdownEntry<Message>,
) -> Vec<DropdownEntry<Message>> {
    vec![
        ruled(Some(ic::icon_dd_cut(tid)), "cut"),
        ruled(Some(ic::icon_dd_copy(tid)), "copy"),
        ruled(Some(ic::icon_dd_paste(tid)), "paste"),
        ruled(Some(ic::icon_dd_smart_paste(tid)), "smart_paste"),
        ruled(None, "duplicate"),
        ruled(Some(ic::icon_dd_select_all(tid)), "select_all"),
    ]
}

/// The net group (#2419): highlight the net under the pointer (else the
/// selection's) on every sheet, and pick a net colour. Assign Net Class
/// takes the net and the class as operands, which this menu cannot supply
/// yet, so it is greyed and says where a class is picked today.
fn net_rows(
    i18n: &I18n,
    tid: ThemeId,
    sc: &CanvasShortcuts,
    ruled: &dyn Fn(Option<Handle>, &'static str) -> DropdownEntry<Message>,
) -> Vec<DropdownEntry<Message>> {
    let assign = match dd_cmd_disabled(i18n, None, "assign_net_class", sc.of("assign_net_class")) {
        DropdownEntry::Item(item) => {
            DropdownEntry::Item(item.tooltip(text(i18n, menu_ids::CTX_ASSIGN_NET_CLASS_ELSEWHERE)))
        }
        other => other,
    };
    vec![
        ruled(None, "highlight_net_under_cursor"),
        ruled(
            Some(ic::icon_dd_net_color_custom(tid)),
            "open_net_color_palette",
        ),
        assign,
    ]
}

/// The board canvas menu of a project with no reuse block, for the tests
/// that pin its other rows.
#[cfg(test)]
pub(super) fn pcb_canvas_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    ctx: &crate::app::command::CommandContext,
    active_submenu: Option<ContextSubmenu>,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    pcb_canvas_menu_with_blocks(i18n, tid, ctx, active_submenu, sc, &[])
}

/// What the document-tab menu is built from: the clicked tab and the facts
/// that grey its rows.
pub(super) struct TabMenuFacts<'a> {
    pub(super) title: &'a str,
    pub(super) tab_idx: usize,
    pub(super) total_tabs: usize,
    /// A tab after this one is on the main tab strip; an undocked tab has
    /// its own window and does not count.
    pub(super) strip_tabs_to_the_right: bool,
    /// The tab already has its own OS window, or it is a Project tab.
    pub(super) undock_unavailable: bool,
    /// The tab names a file (a Component Preview tab does not).
    pub(super) has_file: bool,
    /// The file is a row of an open project's tree.
    pub(super) in_project: bool,
    /// The active keymap's hint for `close_active_document` (empty: none).
    pub(super) close_shortcut: &'a str,
}

/// Pure tab-menu data builder (mockup row 5). The per-tab "Close [title]" row carries
/// the live tab title and the Close shortcut; a bulk-close row greys when it
/// would close nothing; "Open In New Window" greys when the tab cannot be
/// undocked; Copy Path greys for a tab with no file, Reveal in Projects for
/// a file that is not in an open project.
pub(super) fn tab_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    facts: &TabMenuFacts<'_>,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::TabContextAction as A;
    let idx = facts.tab_idx;
    let pick = |action| Message::ContextMenu(ContextMenuMsg::TabAction(action));
    let gated = |icon: Option<Handle>, label: String, live: bool, action| {
        if live {
            dd_msg(icon, &label, "", pick(action))
        } else {
            dd_disabled(icon, &label, None)
        }
    };
    vec![
        dd_msg(
            Some(ic::icon_dd_close(tid)),
            &i18n.message_plain(menu_ids::CTX_TAB_CLOSE, &[("title", facts.title.into())]),
            facts.close_shortcut,
            pick(A::Close(idx)),
        ),
        gated(
            None,
            text(i18n, menu_ids::CTX_TAB_CLOSE_OTHERS),
            facts.total_tabs > 1,
            A::CloseAllOthers(idx),
        ),
        gated(
            None,
            text(i18n, menu_ids::CTX_TAB_CLOSE_RIGHT),
            facts.strip_tabs_to_the_right,
            A::CloseToTheRight(idx),
        ),
        dd_msg(
            None,
            &command_text(i18n, "close_all_documents"),
            "",
            pick(A::CloseAll),
        ),
        DropdownEntry::Separator,
        gated(
            Some(ic::icon_dd_undock(tid)),
            text(i18n, menu_ids::CTX_TAB_OPEN_IN_NEW_WINDOW),
            !facts.undock_unavailable,
            A::Undock(idx),
        ),
        DropdownEntry::Separator,
        gated(
            Some(ic::icon_dd_copy(tid)),
            text(i18n, menu_ids::CTX_TAB_COPY_PATH),
            facts.has_file,
            A::CopyPath(idx),
        ),
        gated(
            Some(ic::icon_dd_folder(tid)),
            text(i18n, menu_ids::CTX_TAB_REVEAL_IN_PROJECTS),
            facts.in_project,
            A::RevealInProjects(idx),
        ),
    ]
}
