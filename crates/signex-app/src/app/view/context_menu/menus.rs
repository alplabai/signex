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
    command_text, dd_cmd_disabled, dd_command, dd_disabled, dd_msg, dd_ruled, submenu_launcher,
    text,
};
use crate::app::command::CommandContext;
use crate::icons as ic;
use crate::keymap::AppCommandId;
use iced::widget::svg::Handle;
use signex_i18n::{I18n, MessageId, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

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
            hit_object: self.context_menu_hit(window_id),
            net_highlighted: self.document_state.net_nav.highlight.is_some(),
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
        let mut entries = vec![DropdownEntry::Header(context_line(
            &self.ui_state.i18n,
            &self.canvas_menu_context(window_id),
        ))];
        entries.extend(canvas_menu_entries(
            &self.ui_state.i18n,
            tid,
            selection,
            active_submenu,
            project_key,
            &sc,
        ));
        super::placement::menu_card(entries, tokens, ui_font)
    }

    /// Whether the canvas menu opened in `window_id` was opened over an
    /// object of that window's sheet.
    fn context_menu_hit(&self, window_id: iced::window::Id) -> bool {
        let Some((x, y)) = self
            .interaction_state
            .context_menu
            .as_ref()
            .and_then(|menu| menu.world)
        else {
            return false;
        };
        self.interaction_state
            .canvas_for_window(window_id)
            .hit_item_at(
                x,
                y,
                self.active_wire_widths(),
                self.active_class_revision(),
            )
            .is_some()
    }

    /// What the canvas menu opened in `window_id` stands on: the selection
    /// (one part named by designator and value), else the sheet's file.
    pub(super) fn canvas_menu_context(&self, window_id: iced::window::Id) -> MenuContext {
        let canvas = self.interaction_state.canvas_for_window(window_id);
        if canvas.selected.is_empty() {
            let file = self
                .document_state
                .sheet_path_for_window(window_id, &self.ui_state)
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            return MenuContext::Sheet { file };
        }
        let part = self
            .document_state
            .engine_for_window(window_id, &self.ui_state)
            .and_then(|engine| {
                crate::app::handlers::placed_part::selected_placed_symbol(engine, canvas)
            })
            .map(|symbol| (symbol.reference.clone(), symbol.value.clone()));
        MenuContext::Selection {
            part,
            count: canvas.selected.len(),
        }
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
            already_undocked || tab.kind.is_project_page(),
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
    /// The window's command facts: every command row is live exactly when
    /// its own enablement rule allows it here (#2419).
    pub(super) commands: CommandContext,
    /// The right-click landed on an object: Find Similar and Highlight Net
    /// have something to seed from.
    pub(super) hit_object: bool,
    /// A net highlight is on, which Clear Filter ends.
    pub(super) net_highlighted: bool,
}

/// What the canvas menu stands on, named by its muted first line.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum MenuContext {
    /// Nothing selected: the sheet, by its file name.
    Sheet { file: String },
    /// `count` objects selected; `part` is the designator and value of the
    /// one selected part.
    Selection {
        part: Option<(String, String)>,
        count: usize,
    },
}

/// The muted first line of the canvas menu for `context`:
/// `schematic sheet · dcdc.snxsch` or `R3 · 100k · 1 item selected`.
pub(super) fn context_line(i18n: &I18n, context: &MenuContext) -> String {
    match context {
        MenuContext::Sheet { file } => i18n.message_plain(
            menu_ids::CTX_SHEET_CONTEXT,
            &[("file", file.clone().into())],
        ),
        MenuContext::Selection { part, count } => {
            let selected = i18n.message_plain(
                menu_ids::CTX_SELECTION_COUNT,
                &[("count", (*count as i64).into())],
            );
            match part {
                Some((reference, value)) => i18n.message_plain(
                    menu_ids::CTX_SELECTION_PART,
                    &[
                        ("reference", reference.clone().into()),
                        ("value", value.clone().into()),
                        ("selected", selected.into()),
                    ],
                ),
                None => selected,
            }
        }
    }
}

/// Pure sheet-menu data builder (#62). Rows and their enable state come
/// only from the passed selection flags, command facts and shortcut hints,
/// so the menu's shape is unit-testable without a window. With nothing
/// selected it is the short sheet menu; with a selection it opens on the
/// selection's own rows. Which rows show follows the selection and what the
/// right-click hit; whether a shown row is live follows the command's own
/// rule (#2419).
pub(super) fn canvas_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    selection: CanvasSelection,
    active_submenu: Option<ContextSubmenu>,
    project_key: Option<signex_widgets::tree_view::TreeKey>,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    let rows = SheetRows {
        i18n,
        tid,
        sc,
        commands: selection.commands,
        active_submenu,
    };
    if selection.has_selection {
        rows.selection_menu(selection)
    } else {
        rows.sheet_menu(selection, project_key)
    }
}

/// What every row of the sheet menu is built from.
struct SheetRows<'a> {
    i18n: &'a I18n,
    tid: ThemeId,
    sc: &'a CanvasShortcuts,
    commands: CommandContext,
    active_submenu: Option<ContextSubmenu>,
}

impl SheetRows<'_> {
    /// A row for `command_id`, live by the command's own rule.
    fn ruled(&self, icon: Option<Handle>, command_id: &'static str) -> DropdownEntry<Message> {
        dd_ruled(
            self.i18n,
            icon,
            command_id,
            self.sc.of(command_id),
            &self.commands,
            true,
        )
    }

    fn launcher(
        &self,
        icon: Handle,
        label: MessageId,
        kind: ContextSubmenu,
    ) -> DropdownEntry<Message> {
        submenu_launcher(
            Some(icon),
            &text(self.i18n, label),
            kind,
            self.active_submenu == Some(kind),
        )
    }

    /// Find Similar seeds from the selection, else from what is under the
    /// cursor, so it is live whenever it is shown (#2416).
    fn find_similar(&self) -> DropdownEntry<Message> {
        dd_command(
            self.i18n,
            Some(ic::icon_dd_find_similar(self.tid)),
            "find_similar_objects",
            self.sc.of("find_similar_objects"),
            None,
        )
    }

    fn net_color(&self) -> DropdownEntry<Message> {
        self.ruled(
            Some(ic::icon_dd_net_color_custom(self.tid)),
            "open_net_color_palette",
        )
    }

    /// Clear Filter ends the net highlight that dims the sheet.
    fn clear_filter(&self) -> DropdownEntry<Message> {
        self.ruled(
            Some(ic::icon_dd_clear_filter(self.tid)),
            "clear_net_highlighting",
        )
    }

    /// The short menu of an empty spot: Find, Place and Sheet Actions, Undo
    /// and Redo, Paste, Paste Special and Select All, Net Color, Project
    /// Options and Preferences. Find Similar and Highlight Net join them only
    /// when the right-click hit an object, Clear Filter only while a net
    /// highlight is on.
    fn sheet_menu(
        &self,
        s: CanvasSelection,
        project_key: Option<signex_widgets::tree_view::TreeKey>,
    ) -> Vec<DropdownEntry<Message>> {
        let tid = self.tid;
        let mut v = vec![self.ruled(Some(ic::icon_chrome_search(tid)), "find")];
        if s.hit_object {
            v.push(self.find_similar());
        }
        v.extend([
            DropdownEntry::Separator,
            self.launcher(
                ic::icon_dd_place_menu(tid),
                menu_ids::MENU_PLACE,
                ContextSubmenu::Place,
            ),
            self.launcher(
                ic::icon_dd_sheet_actions(tid),
                menu_ids::CTX_SHEET_ACTIONS,
                ContextSubmenu::SheetActions,
            ),
            DropdownEntry::Separator,
            self.ruled(Some(ic::icon_undo(tid)), "undo"),
            self.ruled(Some(ic::icon_redo(tid)), "redo"),
            DropdownEntry::Separator,
            self.ruled(Some(ic::icon_dd_paste(tid)), "paste"),
            self.ruled(Some(ic::icon_dd_smart_paste(tid)), "smart_paste"),
            self.ruled(Some(ic::icon_dd_select_all(tid)), "select_all"),
            DropdownEntry::Separator,
        ]);
        if s.hit_object {
            v.push(self.ruled(None, "highlight_net_under_cursor"));
        }
        v.push(self.net_color());
        if s.net_highlighted {
            v.push(self.clear_filter());
        }
        v.extend([
            DropdownEntry::Separator,
            self.project_options(project_key),
            self.ruled(Some(ic::icon_dd_preferences(tid)), "open_preferences"),
        ]);
        v
    }

    /// The menu of a selection: Properties (the default row), Find Similar
    /// and Cross Probe; the edits; the transforms and Align; the Part, Sheet
    /// and Place flyouts; the net rows; Undo; then the planned features
    /// gathered in one greyed flyout.
    fn selection_menu(&self, s: CanvasSelection) -> Vec<DropdownEntry<Message>> {
        let tid = self.tid;
        let properties = match self.ruled(
            Some(ic::icon_dd_properties(tid)),
            "edit_selected_object_properties",
        ) {
            DropdownEntry::Item(item) => DropdownEntry::Item(item.strong()),
            other => other,
        };
        let mut v = vec![
            properties,
            self.find_similar(),
            // #1312: the selected part or net on the board, opened when needed.
            self.ruled(Some(ic::icon_dd_cross_probe(tid)), "cross_probe"),
            DropdownEntry::Separator,
            self.ruled(Some(ic::icon_dd_cut(tid)), "cut"),
            self.ruled(Some(ic::icon_dd_copy(tid)), "copy"),
            self.ruled(None, "duplicate"),
            // Greyed when everything selected is locked: the edit would leave
            // the sheet as it is (#2419).
            self.ruled(Some(ic::icon_dd_delete(tid)), "delete_selection"),
            DropdownEntry::Separator,
            self.ruled(Some(ic::icon_dd_rotate(tid)), "rotate_counterclockwise"),
            self.ruled(Some(ic::icon_dd_flip_x(tid)), "mirror_x"),
            self.ruled(Some(ic::icon_dd_flip_y(tid)), "mirror_y"),
            self.launcher(
                ic::icon_dd_align_menu(tid),
                menu_ids::CTX_ALIGN,
                ContextSubmenu::Align,
            ),
        ];
        if s.drawing_selected {
            v.push(self.launcher(
                ic::icon_dd_align_menu(tid),
                menu_ids::MENU_EDIT_ARRANGE,
                ContextSubmenu::Arrange,
            ));
        }
        v.extend([
            DropdownEntry::Separator,
            self.launcher(
                ic::icon_dd_part_actions(tid),
                menu_ids::CTX_PART_ACTIONS,
                ContextSubmenu::PartActions,
            ),
            self.launcher(
                ic::icon_dd_sheet_actions(tid),
                menu_ids::CTX_SHEET_ACTIONS,
                ContextSubmenu::SheetActions,
            ),
            self.launcher(
                ic::icon_dd_place_menu(tid),
                menu_ids::MENU_PLACE,
                ContextSubmenu::Place,
            ),
            DropdownEntry::Separator,
            self.ruled(None, "highlight_net_under_cursor"),
            self.net_color(),
            self.assign_net_class(),
        ]);
        if s.net_highlighted {
            v.push(self.clear_filter());
        }
        v.extend([
            DropdownEntry::Separator,
            self.ruled(Some(ic::icon_undo(tid)), "undo"),
            self.planned_launcher(),
        ]);
        v
    }

    /// Assign Net Class takes the net and the class as operands, which this
    /// menu cannot supply yet, so it is greyed and says where a class is
    /// picked today (#2419).
    fn assign_net_class(&self) -> DropdownEntry<Message> {
        let i18n = self.i18n;
        let hint = self.sc.of("assign_net_class");
        match dd_cmd_disabled(i18n, None, "assign_net_class", hint) {
            DropdownEntry::Item(item) => DropdownEntry::Item(
                item.tooltip(text(i18n, menu_ids::CTX_ASSIGN_NET_CLASS_ELSEWHERE)),
            ),
            other => other,
        }
    }

    /// The project that owns the active sheet, addressed by its tree key the
    /// way the project tree's own menu addresses it. No project: greyed, no
    /// tooltip (the feature exists, there is just nothing to open it for).
    fn project_options(
        &self,
        project_key: Option<signex_widgets::tree_view::TreeKey>,
    ) -> DropdownEntry<Message> {
        let icon = Some(ic::icon_dd_project_options(self.tid));
        let hint = self.sc.of("open_project_options");
        match project_key {
            Some(key) => dd_command(
                self.i18n,
                icon,
                "open_project_options",
                hint,
                key.first().cloned(),
            ),
            None => dd_cmd_disabled(self.i18n, icon, "open_project_options", hint),
        }
    }

    /// The greyed "Planned (N) ›" launcher: hovering it opens the planned
    /// rows, each with what it will do; clicking it does nothing.
    fn planned_launcher(&self) -> DropdownEntry<Message> {
        let count = super::submenu::planned_entries(self.i18n, self.tid).len();
        let label = self.i18n.message_plain(
            menu_ids::CTX_PLANNED_GROUP,
            &[("count", (count as i64).into())],
        );
        let kind = ContextSubmenu::Planned;
        DropdownEntry::Item(
            signex_widgets::active_bar_dropdown::DropdownItem::passive(label)
                .disabled(true)
                .submenu()
                .hover(
                    Message::ContextMenu(ContextMenuMsg::SubmenuHover(kind)),
                    Message::ContextMenu(ContextMenuMsg::SubmenuLeave),
                )
                .active(self.active_submenu == Some(kind)),
        )
    }
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

/// The board canvas menu (#2240, #2405): Find Similar and Find; the Place
/// and Align flyouts; Undo and Redo; the clipboard; move, turn, flip and
/// mirror; the pin swap and reuse block group (#2410); Delete; Board Sketch;
/// the net of the selection; Cross Probe and
/// Properties; then the View group's Toggle Ratsnest. Each row runs its
/// catalog command like the keys and the Edit menu do, and is live exactly
/// when that command is enabled in `ctx`: on a read-only board every editing
/// row is greyed. Align is offered with a selection, as on a sheet.
pub(super) fn pcb_canvas_menu_with_blocks(
    i18n: &I18n,
    tid: ThemeId,
    ctx: &crate::app::command::CommandContext,
    active_submenu: Option<ContextSubmenu>,
    sc: &CanvasShortcuts,
    blocks: &[String],
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
    // A row live only when the selection holds what it acts on (the lock
    // rows #2407, the board Tools edits #2408, #2409), and its command is
    // enabled.
    let gated_row = |command_id: &'static str, live: bool| {
        if live {
            row(None, command_id)
        } else {
            dd_cmd_disabled(i18n, None, command_id, sc.of(command_id))
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
        // Lock or unlock the selected tracks and vias (#2407).
        gated_row("lock_board_items", ctx.pcb.lockable),
        gated_row("unlock_board_items", ctx.pcb.unlockable),
    ]);
    // The pin swap and reuse block group (#2410), whose Place rows name the
    // project's `blocks`.
    v.extend(super::board_part_rows::part_rows(i18n, ctx.pcb, sc, blocks));
    v.extend([
        DropdownEntry::Separator,
        row(Some(ic::icon_dd_delete(tid)), "delete_selection"),
        DropdownEntry::Separator,
        // Enter or leave the board's sketch mode (#2234).
        row(Some(ic::icon_dd_rect(tid)), "board_sketch"),
        DropdownEntry::Separator,
        // The board Tools edits on the selection (#2408, #2409), as in Tools.
        gated_row("gloss_selected_routes", ctx.pcb.any),
        gated_row("add_teardrops", true),
        gated_row("board_shape_from_selection", ctx.pcb.any),
        gated_row("keepout_from_selection", ctx.pcb.any),
        gated_row("arrange_in_box", ctx.pcb.placeable),
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
