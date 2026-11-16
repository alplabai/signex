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
    command_text, dd_cmd_disabled, dd_command, dd_disabled, dd_msg, dd_planned, dd_planned_submenu,
    submenu_launcher, text,
};
use crate::icons as ic;
use crate::keymap::AppCommandId;
use signex_i18n::{I18n, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Command ids whose keyboard hint the canvas menu shows.
pub(super) const CANVAS_SHORTCUT_IDS: &[&str] = &[
    "find",
    "find_similar_objects",
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
    "open_preferences",
    "open_datasheet",
    "edit_selected_symbol_in_symbol_editor",
    "edit_selected_object_properties",
    "change_component",
    "open_project_options",
    "cross_probe",
    "board_sketch",
    "change_footprint",
    // The board menu's own rows (#2405).
    "undo",
    "redo",
    "select_board_net",
    "highlight_board_net",
    "toggle_ratsnest",
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
            let entries =
                pcb_canvas_menu_entries(&self.ui_state.i18n, tid, &context, active_submenu, &sc);
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
        // window. Single-tab workspaces still show the row (Altium does
        // too) — undocking a sole tab leaves the main window empty.
        use super::super::state::WindowKind;
        let already_undocked =
            self.ui_state.windows.values().any(
                |kind| matches!(kind, WindowKind::UndockedTab { path, .. } if *path == tab.path),
            );
        let entries = tab_menu_entries(
            &self.ui_state.i18n,
            &tab.title,
            ctx.tab_idx,
            self.document_state.tabs.len(),
            // A Project tab stays docked (#1202): an undocked window would
            // take the main window's shortcuts.
            already_undocked || tab.kind.as_project().is_some(),
        );
        let panel_ctx = &self.document_state.panel_ctx;
        super::placement::menu_card(entries, &panel_ctx.tokens, &panel_ctx.ui_font_name).element
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
}

/// Pure canvas-menu data builder. Rows and their enable state are derived
/// only from the passed selection flags + shortcut hints, so the menu's
/// shape is unit-testable without a window.
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
        child_sheet_selected,
        drawing_selected,
        placed_part_selected,
    } = selection;
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(24);

    v.push(dd_command(
        i18n,
        Some(ic::icon_dd_find_similar(tid)),
        "find_similar_objects",
        sc.of("find_similar_objects"),
        None,
    ));
    v.push(dd_command(
        i18n,
        Some(ic::icon_chrome_search(tid)),
        "find",
        sc.of("find"),
        None,
    ));
    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_clear_filter(tid)),
        &text(i18n, menu_ids::CTX_CLEAR_FILTER),
        Some("Shift+C"),
    ));
    v.push(DropdownEntry::Separator);
    v.push(submenu_launcher(
        Some(ic::icon_dd_place_menu(tid)),
        &text(i18n, menu_ids::MENU_PLACE),
        ContextSubmenu::Place,
        active_submenu == Some(ContextSubmenu::Place),
    ));
    v.push(dd_planned_submenu(
        i18n,
        Some(ic::icon_dd_part_actions(tid)),
        &text(i18n, menu_ids::CTX_PART_ACTIONS),
    ));
    v.push(dd_planned_submenu(
        i18n,
        Some(ic::icon_dd_sheet_actions(tid)),
        &text(i18n, menu_ids::CTX_SHEET_ACTIONS),
    ));

    if has_selection {
        v.push(dd_planned_submenu(
            i18n,
            Some(ic::icon_dd_references(tid)),
            &text(i18n, menu_ids::CTX_REFERENCES),
        ));
        v.push(submenu_launcher(
            Some(ic::icon_dd_align_menu(tid)),
            &text(i18n, menu_ids::CTX_ALIGN),
            ContextSubmenu::Align,
            active_submenu == Some(ContextSubmenu::Align),
        ));
        if drawing_selected {
            v.push(submenu_launcher(
                Some(ic::icon_dd_align_menu(tid)),
                &text(i18n, menu_ids::MENU_EDIT_ARRANGE),
                ContextSubmenu::Arrange,
                active_submenu == Some(ContextSubmenu::Arrange),
            ));
        }
        v.push(dd_planned_submenu(
            i18n,
            Some(ic::icon_dd_unions(tid)),
            &text(i18n, menu_ids::CTX_UNIONS),
        ));
        v.push(dd_planned_submenu(
            i18n,
            Some(ic::icon_dd_snippets(tid)),
            &text(i18n, menu_ids::CTX_SNIPPETS),
        ));
    }

    if child_sheet_selected {
        v.push(dd_command(
            i18n,
            Some(ic::icon_dd_open_child_sheet(tid)),
            "open_child_sheet",
            sc.of("open_child_sheet"),
            None,
        ));
        for command_id in ["create_sheet_from_sheet_symbol", "sync_sheet_pins"] {
            v.push(dd_command(i18n, None, command_id, sc.of(command_id), None));
        }
        v.push(DropdownEntry::Separator);
    }

    // #1312: the selected part or net on the board, opened when needed.
    let probe_icon = Some(ic::icon_dd_cross_probe(tid));
    v.push(if has_selection {
        dd_command(i18n, probe_icon, "cross_probe", sc.of("cross_probe"), None)
    } else {
        dd_cmd_disabled(i18n, probe_icon, "cross_probe", sc.of("cross_probe"))
    });
    v.push(DropdownEntry::Separator);
    // Cut and Copy have nothing to act on without a selection: greyed, as
    // the Edit menu greys them and as the keys do nothing there.
    for (icon, command_id) in [
        (ic::icon_dd_cut(tid), "cut"),
        (ic::icon_dd_copy(tid), "copy"),
    ] {
        v.push(if has_selection {
            dd_command(i18n, Some(icon), command_id, sc.of(command_id), None)
        } else {
            dd_cmd_disabled(i18n, Some(icon), command_id, sc.of(command_id))
        });
    }
    v.push(dd_command(
        i18n,
        Some(ic::icon_dd_paste(tid)),
        "paste",
        sc.of("paste"),
        None,
    ));
    v.push(dd_command(
        i18n,
        Some(ic::icon_dd_smart_paste(tid)),
        "smart_paste",
        sc.of("smart_paste"),
        None,
    ));
    v.push(DropdownEntry::Separator);

    if has_selection {
        v.push(dd_command(
            i18n,
            Some(ic::icon_dd_rotate(tid)),
            "rotate_counterclockwise",
            sc.of("rotate_counterclockwise"),
            None,
        ));
        v.push(dd_command(
            i18n,
            Some(ic::icon_dd_flip_x(tid)),
            "mirror_x",
            sc.of("mirror_x"),
            None,
        ));
        v.push(dd_command(
            i18n,
            Some(ic::icon_dd_flip_y(tid)),
            "mirror_y",
            sc.of("mirror_y"),
            None,
        ));
        v.push(dd_command(
            i18n,
            Some(ic::icon_dd_delete(tid)),
            "delete_selection",
            sc.of("delete_selection"),
            None,
        ));
        v.push(DropdownEntry::Separator);
    }

    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_comment(tid)),
        &text(i18n, menu_ids::CTX_COMMENT),
        None,
    ));
    v.push(dd_planned(
        i18n,
        Some(ic::icon_dd_pin_mapping(tid)),
        &text(i18n, menu_ids::CTX_PIN_MAPPING),
        None,
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
    v.push(dd_command(
        i18n,
        Some(ic::icon_dd_preferences(tid)),
        "open_preferences",
        sc.of("open_preferences"),
        None,
    ));

    if placed_part_selected {
        // The placed-part actions, all catalog commands.
        for command_id in [
            "open_datasheet",
            "edit_selected_symbol_in_symbol_editor",
            "change_component",
        ] {
            v.push(dd_command(i18n, None, command_id, sc.of(command_id), None));
        }
    }
    if has_selection {
        v.push(dd_planned(
            i18n,
            Some(ic::icon_dd_supplier_links(tid)),
            &text(i18n, menu_ids::CTX_SUPPLIER_LINKS),
            None,
        ));
        // Properties → ensure the Properties panel is visible; it already
        // tracks the current selection, so it populates with the
        // right-clicked item's fields once shown.
        v.push(dd_command(
            i18n,
            Some(ic::icon_dd_properties(tid)),
            "edit_selected_object_properties",
            sc.of("edit_selected_object_properties"),
            None,
        ));
    }

    v
}

/// The board canvas menu (#2240, #2405): Find Similar and Find; the Place
/// and Align flyouts; Undo and Redo; the clipboard; move, turn, flip and
/// mirror; Delete; Board Sketch; the net of the selection; Cross Probe and
/// Properties; then the View group's Toggle Ratsnest. Each row runs its
/// catalog command like the keys and the Edit menu do, and is live exactly
/// when that command is enabled in `ctx`: on a read-only board every editing
/// row is greyed. Align is offered with a selection, as on a sheet.
pub(super) fn pcb_canvas_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    ctx: &crate::app::command::CommandContext,
    active_submenu: Option<ContextSubmenu>,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    let row = |icon: Option<iced::widget::svg::Handle>, command_id: &'static str| {
        let live =
            AppCommandId::new(command_id).is_ok_and(|id| crate::app::command::is_enabled(&id, ctx));
        if live {
            dd_command(i18n, icon, command_id, sc.of(command_id), None)
        } else {
            dd_cmd_disabled(i18n, icon, command_id, sc.of(command_id))
        }
    };
    let launcher = |kind: ContextSubmenu, icon: iced::widget::svg::Handle, label: String| {
        submenu_launcher(Some(icon), &label, kind, active_submenu == Some(kind))
    };
    let mut v = vec![
        row(Some(ic::icon_dd_find_similar(tid)), "find_similar_objects"),
        row(Some(ic::icon_chrome_search(tid)), "find"),
        DropdownEntry::Separator,
        launcher(
            ContextSubmenu::Place,
            ic::icon_dd_place_menu(tid),
            text(i18n, menu_ids::MENU_PLACE),
        ),
    ];
    if ctx.pcb.any {
        v.push(launcher(
            ContextSubmenu::Align,
            ic::icon_dd_align_menu(tid),
            text(i18n, menu_ids::CTX_ALIGN),
        ));
    }
    v.extend([
        DropdownEntry::Separator,
        row(None, "undo"),
        row(None, "redo"),
        DropdownEntry::Separator,
        // The clipboard group (#2361), as on a sheet.
        row(Some(ic::icon_dd_cut(tid)), "cut"),
        row(Some(ic::icon_dd_copy(tid)), "copy"),
        row(Some(ic::icon_dd_paste(tid)), "paste"),
        row(Some(ic::icon_dd_smart_paste(tid)), "smart_paste"),
        row(None, "duplicate"),
        row(Some(ic::icon_dd_select_all(tid)), "select_all"),
        DropdownEntry::Separator,
        row(Some(ic::icon_dd_move_sel(tid)), "move_selection"),
        DropdownEntry::Separator,
        row(Some(ic::icon_dd_rotate(tid)), "rotate_counterclockwise"),
        row(Some(ic::icon_dd_rotate_cw(tid)), "rotate_clockwise"),
        row(Some(ic::icon_dd_flip_x(tid)), "flip_selection"),
        row(Some(ic::icon_dd_flip_x(tid)), "mirror_x"),
        row(Some(ic::icon_dd_flip_y(tid)), "mirror_y"),
        // Picks a library footprint for the selected components (#2366).
        row(None, "change_footprint"),
        DropdownEntry::Separator,
        row(Some(ic::icon_dd_delete(tid)), "delete_selection"),
        DropdownEntry::Separator,
        // Enter or leave the board's sketch mode (#2234).
        row(Some(ic::icon_dd_rect(tid)), "board_sketch"),
        DropdownEntry::Separator,
        // The net of the selection or of what is under the cursor (#2403).
        row(None, "select_board_net"),
        row(None, "highlight_board_net"),
        row(Some(ic::icon_dd_cross_probe(tid)), "cross_probe"),
        // Shows the Properties panel, where one selected footprint's
        // designator and value are edited (#1312).
        row(
            Some(ic::icon_dd_properties(tid)),
            "edit_selected_object_properties",
        ),
        DropdownEntry::Separator,
        // View group (#2223): the View menu's board-view row.
        row(None, "toggle_ratsnest"),
    ]);
    v
}

/// Pure tab-menu data builder. The per-tab "Close [title]" row carries the
/// live tab title; the bulk-close rows are gated on whether they'd be
/// no-ops (single tab open → no "others" to close); "Open In New Window"
/// greys out when the tab cannot be undocked: it already lives in its own
/// OS window, or it is a Project tab.
pub(super) fn tab_menu_entries(
    i18n: &I18n,
    title: &str,
    tab_idx: usize,
    total_tabs: usize,
    undock_unavailable: bool,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::TabContextAction as A;
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(5);

    v.push(dd_msg(
        None,
        &i18n.message_plain(menu_ids::CTX_TAB_CLOSE, &[("title", title.into())]),
        "",
        Message::ContextMenu(ContextMenuMsg::TabAction(A::Close(tab_idx))),
    ));
    if total_tabs > 1 {
        v.push(dd_msg(
            None,
            &text(i18n, menu_ids::CTX_TAB_CLOSE_OTHERS),
            "",
            Message::ContextMenu(ContextMenuMsg::TabAction(A::CloseAllOthers(tab_idx))),
        ));
    } else {
        v.push(dd_disabled(
            None,
            &text(i18n, menu_ids::CTX_TAB_CLOSE_OTHERS),
            None,
        ));
    }
    v.push(dd_msg(
        None,
        &command_text(i18n, "close_all_documents"),
        "",
        Message::ContextMenu(ContextMenuMsg::TabAction(A::CloseAll)),
    ));
    v.push(DropdownEntry::Separator);
    v.push(if undock_unavailable {
        dd_disabled(
            None,
            &text(i18n, menu_ids::CTX_TAB_OPEN_IN_NEW_WINDOW),
            None,
        )
    } else {
        dd_msg(
            None,
            &text(i18n, menu_ids::CTX_TAB_OPEN_IN_NEW_WINDOW),
            "",
            Message::ContextMenu(ContextMenuMsg::TabAction(A::Undock(tab_idx))),
        )
    });

    v
}
