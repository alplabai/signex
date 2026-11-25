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
use iced::widget::svg::Handle;
use signex_i18n::{I18n, MessageId, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

#[cfg(test)]
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
            child_sheet_file_missing: self.selected_sheet_file_missing(window_id),
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
            // #1261: the swap the clicked pin or the selected units ask for.
            swap: self
                .document_state
                .engine_for_window(window_id, &self.ui_state)
                .map(|engine| self.sheet_swap_ready(engine.document(), canvas))
                .unwrap_or_default(),
            // Mockup row 158: the selection holds a member of a union.
            union_selected: self
                .document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some_and(|engine| {
                    !signex_engine::unions::unions_of_items(engine.document(), &canvas.selected)
                        .is_empty()
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
            let entries = super::board_menu::board_canvas_menu(
                &self.ui_state.i18n,
                tid,
                &context,
                active_submenu,
                &sc,
                &blocks,
                self.board_menu_on_pour(Some(window_id)),
            );
            return super::placement::menu_card(entries, tokens, ui_font);
        }
        let (selection, project_key) = self.canvas_menu_inputs(window_id);
        let mut entries = vec![DropdownEntry::Header(context_line(
            &self.ui_state.i18n,
            &self.canvas_menu_context(window_id),
        ))];
        let variant = self.canvas_menu_variant(window_id);
        entries.extend(canvas_menu_with_variant(
            &self.ui_state.i18n,
            tid,
            selection,
            active_submenu,
            project_key,
            &sc,
            variant.as_ref(),
        ));
        super::placement::menu_card(entries, tokens, ui_font)
    }

    /// The variant the "In variant" row names (mockup row 62): the active
    /// one of the project that owns the sheet `window_id` shows, when that
    /// project has a variant besides Base. `None` hides the row.
    pub(super) fn canvas_menu_variant(&self, window_id: iced::window::Id) -> Option<MenuVariant> {
        let project = self
            .document_state
            .sheet_path_for_window(window_id, &self.ui_state)
            .and_then(|path| self.document_state.document_project(path))?;
        if project.data.variant_definitions.is_empty() {
            return None;
        }
        Some(MenuVariant {
            project_id: project.id,
            name: project.data.active_variant_name().to_string(),
        })
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
                // The Gerber Viewer draws in the main window only.
                undock_unavailable: already_undocked
                    || tab.kind.is_project_page()
                    || tab.kind.is_gerber_viewer(),
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
    /// The selected sheet symbol's file is missing: Open Sheet is greyed and
    /// says why (#2847).
    pub(super) child_sheet_file_missing: bool,
    pub(super) drawing_selected: bool,
    /// One non-power symbol: the placed-part actions apply (#1260).
    pub(super) placed_part_selected: bool,
    /// Swap Pins and Swap Gates have a swap to make (#1261).
    pub(super) swap: crate::app::sheet_swap::SwapReady,
    /// The selection holds a member of a union: Add / Remove / Break apply
    /// (mockup row 158).
    pub(super) union_selected: bool,
    /// The window's command facts: every command row is live exactly when
    /// its own enablement rule allows it here (#2419).
    pub(super) commands: CommandContext,
    /// The right-click landed on an object: Find Similar and Highlight Net
    /// have something to seed from.
    pub(super) hit_object: bool,
    /// A net highlight is on, which Clear Filter ends.
    pub(super) net_highlighted: bool,
}

/// The project variant the selection menu's "In variant" row names
/// (mockup row 62).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MenuVariant {
    /// The project whose Variant Manager the flyout's rows open.
    pub(super) project_id: crate::app::ProjectId,
    /// The active variant's name, Base when none is.
    pub(super) name: String,
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

/// The sheet menu of a project with no variant besides Base, for the tests
/// that pin its other rows.
#[cfg(test)]
pub(super) fn canvas_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    selection: CanvasSelection,
    active_submenu: Option<ContextSubmenu>,
    project_key: Option<signex_widgets::tree_view::TreeKey>,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    canvas_menu_with_variant(i18n, tid, selection, active_submenu, project_key, sc, None)
}

/// Pure sheet-menu data builder (Graphite mockup #62). Rows and their enable state come
/// only from the passed selection flags, command facts and shortcut hints,
/// so the menu's shape is unit-testable without a window. With nothing
/// selected it is the short sheet menu; with a selection it opens on the
/// selection's own rows. Which rows show follows the selection and what the
/// right-click hit; whether a shown row is live follows the command's own
/// rule (#2419). With `variant` the selection menu gains "In variant:
/// <name> ›" after Part Actions (mockup row 62).
pub(super) fn canvas_menu_with_variant(
    i18n: &I18n,
    tid: ThemeId,
    selection: CanvasSelection,
    active_submenu: Option<ContextSubmenu>,
    project_key: Option<signex_widgets::tree_view::TreeKey>,
    sc: &CanvasShortcuts,
    variant: Option<&MenuVariant>,
) -> Vec<DropdownEntry<Message>> {
    let rows = SheetRows {
        i18n,
        tid,
        sc,
        commands: selection.commands,
        active_submenu,
        variant,
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
    /// The project's active variant; `None` hides the In variant row.
    variant: Option<&'a MenuVariant>,
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
    /// and Cross Probe; the edits; the transforms, Align and Unions; the
    /// Part, Sheet and Place flyouts; the net rows; Undo; then the planned
    /// features gathered in one greyed flyout.
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
        // Mockup row 158: right-click ▸ Unions ▸ Create Union… (it left the Planned flyout).
        v.push(self.launcher(
            ic::icon_dd_unions(tid),
            menu_ids::CTX_UNIONS,
            ContextSubmenu::Unions,
        ));
        v.extend([
            DropdownEntry::Separator,
            self.launcher(
                ic::icon_dd_part_actions(tid),
                menu_ids::CTX_PART_ACTIONS,
                ContextSubmenu::PartActions,
            ),
        ]);
        if let Some(variant) = self.variant {
            v.push(self.in_variant_launcher(variant));
        }
        v.extend([
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

    /// "In variant: <name> ›" (mockup row 62): the flyout names the active
    /// variant, so the row says which variant its rows change.
    fn in_variant_launcher(&self, variant: &MenuVariant) -> DropdownEntry<Message> {
        let kind = ContextSubmenu::InVariant;
        let label = self.i18n.message_plain(
            menu_ids::CTX_IN_VARIANT,
            &[("variant", variant.name.clone().into())],
        );
        submenu_launcher(
            Some(ic::icon_dd_layers(self.tid)),
            &label,
            kind,
            self.active_submenu == Some(kind),
        )
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
        let icon = Some(ic::icon_menu_sliders(self.tid));
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

/// Unions submenu (mockup row 158): Create Union… and Add Selection to Union (the
/// union picked in the Unions panel) with a selection; Remove and Break
/// while the selection holds a member of a union. Each row is also gated by
/// its command's own rule.
pub(super) fn unions_entries(
    i18n: &I18n,
    selection: CanvasSelection,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<Message>> {
    let row = |id: &'static str, applies: bool| {
        dd_ruled(i18n, None, id, &hint(id), &selection.commands, applies)
    };
    vec![
        row("create_union", selection.has_selection),
        row("add_to_union", selection.has_selection),
        row("remove_from_union", selection.union_selected),
        DropdownEntry::Separator,
        row("break_union", selection.union_selected),
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
