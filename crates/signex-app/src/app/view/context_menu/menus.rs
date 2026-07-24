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

use super::items::{dd_disabled, dd_kb, dd_msg, submenu_launcher};
use crate::icons as ic;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Keyboard-shortcut hints for the canvas menu, resolved from the active
/// keymap profile (with the historic Altium defaults as fallbacks) before
/// the pure builder runs.
pub(super) struct CanvasShortcuts {
    pub find: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub smart_paste: String,
}

impl Signex {
    /// v0.18.10 — Altium-style grid picker popup body. Renders the
    /// standard 1mil…2.5mm ladder; clicking a row sends
    /// `Message::Ui(UiMsg::GridPickerSelect(step_mm))` and closes the popup.
    pub(in crate::app::view) fn view_grid_picker_menu(&self) -> Element<'_, Message> {
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

    /// Canvas right-click menu. Resolves selection state + shortcut hints
    /// from `self`, then delegates to the pure [`canvas_menu_entries`].
    pub(in crate::app::view) fn view_context_menu(&self) -> Element<'_, Message> {
        let tid = self.ui_state.theme_id;
        let canvas = self.interaction_state.active_canvas();
        let has_selection = !canvas.selected.is_empty();
        let child_sheet_selected = canvas
            .selected
            .iter()
            .any(|item| item.kind == signex_types::schematic::SelectedKind::ChildSheet);
        let active_submenu = self.interaction_state.context_submenu;
        // Shortcut hints read from the active keymap profile, falling
        // back to the historic Altium defaults when a command is unbound.
        let sc = CanvasShortcuts {
            find: self.keymap_shortcut_label("find", "Ctrl+F"),
            cut: self.keymap_shortcut_label("cut", "Ctrl+X"),
            copy: self.keymap_shortcut_label("copy", "Ctrl+C"),
            paste: self.keymap_shortcut_label("paste", "Ctrl+V"),
            smart_paste: self.keymap_shortcut_label("smart_paste", "Shift+Ctrl+V"),
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let entries = canvas_menu_entries(
            tid,
            tokens,
            has_selection,
            child_sheet_selected,
            active_submenu,
            &sc,
        );
        signex_widgets::active_bar_dropdown::view(entries, tokens, Some(Self::CONTEXT_MENU_WIDTH))
    }

    /// Build the document-tab right-click menu. Resolves the tab title and
    /// undock state from `self`, then delegates to [`tab_menu_entries`].
    pub(in crate::app::view) fn view_tab_context_menu(
        &self,
        ctx: &crate::app::TabContextMenuState,
    ) -> Element<'_, Message> {
        let Some(tab) = self.document_state.tabs.get(ctx.tab_idx) else {
            return container(iced::widget::Space::new()).into();
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
            &tab.title,
            ctx.tab_idx,
            self.document_state.tabs.len(),
            already_undocked,
        );
        let tokens = &self.document_state.panel_ctx.tokens;
        signex_widgets::active_bar_dropdown::view(entries, tokens, Some(Self::CONTEXT_MENU_WIDTH))
    }
}

/// Pure canvas-menu data builder. Rows and their enable state are derived
/// only from the passed selection flags + shortcut hints, so the menu's
/// shape is unit-testable without a window.
#[allow(clippy::vec_init_then_push)]
pub(super) fn canvas_menu_entries(
    tid: ThemeId,
    tokens: &ThemeTokens,
    has_selection: bool,
    child_sheet_selected: bool,
    active_submenu: Option<ContextSubmenu>,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(24);

    v.push(dd_disabled(
        Some(ic::icon_dd_find_similar(tid)),
        "Find Similar Objects...",
        None,
    ));
    v.push(dd_msg(
        Some(ic::icon_chrome_search(tid)),
        "Find Text...",
        &sc.find,
        Message::Overlay(OverlayMsg::OpenFind),
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_clear_filter(tid)),
        "Clear Filter",
        Some("Shift+C"),
    ));
    v.push(DropdownEntry::Separator);
    v.push(submenu_launcher(
        tokens,
        Some(ic::icon_dd_place_menu(tid)),
        "Place",
        ContextSubmenu::Place,
        active_submenu == Some(ContextSubmenu::Place),
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_part_actions(tid)),
        "Part Actions",
        Some(SUBMENU_ARROW),
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_sheet_actions(tid)),
        "Sheet Actions",
        Some(SUBMENU_ARROW),
    ));

    if has_selection {
        v.push(dd_disabled(
            Some(ic::icon_dd_references(tid)),
            "References",
            Some(SUBMENU_ARROW),
        ));
        v.push(submenu_launcher(
            tokens,
            Some(ic::icon_dd_align_menu(tid)),
            "Align",
            ContextSubmenu::Align,
            active_submenu == Some(ContextSubmenu::Align),
        ));
        v.push(dd_disabled(
            Some(ic::icon_dd_unions(tid)),
            "Unions",
            Some(SUBMENU_ARROW),
        ));
        v.push(dd_disabled(
            Some(ic::icon_dd_snippets(tid)),
            "Snippets",
            Some(SUBMENU_ARROW),
        ));
    }

    if child_sheet_selected {
        v.push(dd_kb(
            Some(ic::icon_dd_open_child_sheet(tid)),
            "Open Child Sheet",
            "Enter",
            ContextAction::OpenChildSheet,
        ));
        v.push(DropdownEntry::Separator);
    }

    v.push(dd_disabled(
        Some(ic::icon_dd_cross_probe(tid)),
        "Cross Probe",
        None,
    ));
    v.push(DropdownEntry::Separator);
    v.push(dd_kb(
        Some(ic::icon_dd_cut(tid)),
        "Cut",
        &sc.cut,
        ContextAction::Cut,
    ));
    v.push(dd_kb(
        Some(ic::icon_dd_copy(tid)),
        "Copy",
        &sc.copy,
        ContextAction::Copy,
    ));
    v.push(dd_kb(
        Some(ic::icon_dd_paste(tid)),
        "Paste",
        &sc.paste,
        ContextAction::Paste,
    ));
    v.push(dd_kb(
        Some(ic::icon_dd_smart_paste(tid)),
        "Paste Special",
        &sc.smart_paste,
        ContextAction::SmartPaste,
    ));
    v.push(DropdownEntry::Separator);

    if has_selection {
        v.push(dd_kb(
            Some(ic::icon_dd_rotate(tid)),
            "Rotate",
            "Space",
            ContextAction::RotateSelected,
        ));
        v.push(dd_kb(
            Some(ic::icon_dd_flip_x(tid)),
            "Mirror X",
            "X",
            ContextAction::MirrorX,
        ));
        v.push(dd_kb(
            Some(ic::icon_dd_flip_y(tid)),
            "Mirror Y",
            "Y",
            ContextAction::MirrorY,
        ));
        v.push(dd_kb(
            Some(ic::icon_dd_delete(tid)),
            "Delete",
            "Del",
            ContextAction::Delete,
        ));
        v.push(DropdownEntry::Separator);
    }

    v.push(dd_disabled(
        Some(ic::icon_dd_comment(tid)),
        "Comment...",
        None,
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_pin_mapping(tid)),
        "Pin Mapping...",
        None,
    ));
    v.push(dd_disabled(
        Some(ic::icon_dd_project_options(tid)),
        "Project Options...",
        None,
    ));
    v.push(dd_msg(
        Some(ic::icon_dd_preferences(tid)),
        "Preferences...",
        "",
        Message::Preferences(PreferencesMsg::Open),
    ));

    if has_selection {
        v.push(dd_disabled(
            Some(ic::icon_dd_supplier_links(tid)),
            "Supplier Links...",
            None,
        ));
        // Properties → ensure the Properties panel is visible; it already
        // tracks the current selection, so it populates with the
        // right-clicked item's fields once shown.
        v.push(dd_msg(
            Some(ic::icon_dd_properties(tid)),
            "Properties...",
            "F11",
            Message::Menu(menu_bar::MenuMessage::OpenPropertiesPanel),
        ));
    }

    v
}

/// Pure tab-menu data builder. The per-tab "Close [title]" row carries the
/// live tab title; the bulk-close rows are gated on whether they'd be
/// no-ops (single tab open → no "others" to close); "Open In New Window"
/// greys out when the tab already lives in its own OS window.
pub(super) fn tab_menu_entries(
    title: &str,
    tab_idx: usize,
    total_tabs: usize,
    already_undocked: bool,
) -> Vec<DropdownEntry<Message>> {
    use crate::app::TabContextAction as A;
    let mut v: Vec<DropdownEntry<Message>> = Vec::with_capacity(5);

    v.push(dd_msg(
        None,
        &format!("Close {title}"),
        "",
        Message::ContextMenu(ContextMenuMsg::TabAction(A::Close(tab_idx))),
    ));
    if total_tabs > 1 {
        v.push(dd_msg(
            None,
            "Close All Other Documents",
            "",
            Message::ContextMenu(ContextMenuMsg::TabAction(A::CloseAllOthers(tab_idx))),
        ));
    } else {
        v.push(dd_disabled(None, "Close All Other Documents", None));
    }
    v.push(dd_msg(
        None,
        "Close All Documents",
        "",
        Message::ContextMenu(ContextMenuMsg::TabAction(A::CloseAll)),
    ));
    v.push(DropdownEntry::Separator);
    v.push(if already_undocked {
        dd_disabled(None, "Open In New Window", None)
    } else {
        dd_msg(
            None,
            "Open In New Window",
            "",
            Message::ContextMenu(ContextMenuMsg::TabAction(A::Undock(tab_idx))),
        )
    });

    v
}
