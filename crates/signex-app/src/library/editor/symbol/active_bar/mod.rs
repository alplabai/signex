//! The symbol editor's floating active bar over the `.snxsym` canvas,
//! grouped like the footprint editor's bar.
//!
//! Two groups split by a divider: selection (Filter, Snap, Move, Select,
//! Align, then the Select tool) and placement (Pin, Shapes, Text, then the
//! Place Pin tool). The eight dropdown triggers carry the corner
//! indicator; their menus come from [`dropdowns::entries`]. Pure-graphics
//! tools live in the Shapes dropdown to keep the bar slim.
//!
//! Built on `signex_widgets::active_bar::view`; the dropdown overlay and
//! click-outside backstop are mounted by the symbol editor view.

use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::SymbolDocument;
use crate::icons as ic;
use crate::library::editor::symbol::canvas::SymbolTool;
use crate::library::editor::symbol::state::{SymActiveBarMenu, SymbolFilterKind};
use crate::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg, SymbolToolMsg};

mod dropdowns;

/// Build the SchLib bar items only — caller mounts via
/// `signex_widgets::active_bar::view(items, tokens)` so the chain is
/// identical to the schematic.
pub fn bar_items(
    editor: &SymbolDocument,
    theme_id: ThemeId,
    i18n: &I18n,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active_tool = editor.tool;
    let tool = |icon: ActiveBarIcon, tooltip: &str, pick: SymbolToolMsg, armed: SymbolTool| {
        ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip: tooltip.to_string(),
            enabled: true,
            selected: active_tool == armed,
            on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Symbol(SymbolEditorMsg::SetTool(pick)),
            }),
            ..ActiveBarButton::default()
        })
    };

    let mut triggers = dropdown_trigger_items(editor, theme_id, i18n);
    let placement = triggers.split_off(SELECTION_TRIGGERS);

    let mut items = triggers;
    items.push(tool(
        ActiveBarIcon::Svg(ic::icon_select(theme_id)),
        "Select",
        SymbolToolMsg::Select,
        SymbolTool::Select,
    ));
    items.push(ActiveBarItem::Separator);
    items.extend(placement);
    items.push(tool(
        ActiveBarIcon::Glyph("\u{2192}"),
        "Place Pin",
        SymbolToolMsg::AddPin,
        SymbolTool::AddPin,
    ));
    items
}

/// The triggers of the selection group (Filter, Snap, Move, Select,
/// Align); the rest are the placement group.
const SELECTION_TRIGGERS: usize = 5;

/// The selection filter's categories, in the order of its dropdown chips.
const FILTER_KINDS: [SymbolFilterKind; 7] = [
    SymbolFilterKind::Pins,
    SymbolFilterKind::Drawings,
    SymbolFilterKind::Texts,
    SymbolFilterKind::Designators,
    SymbolFilterKind::Values,
    SymbolFilterKind::Parameters,
    SymbolFilterKind::Other,
];

/// Tooltip of the selection-filter trigger: the categories it filters.
fn filter_tooltip(i18n: &I18n) -> String {
    let kinds: Vec<String> = FILTER_KINDS
        .iter()
        .map(|kind| {
            t!(i18n, ids::EDITORS_SYMBOL_BAR_FILTER_KIND, "kind" => kind.label()).into_string()
        })
        .collect();
    t!(
        i18n,
        ids::EDITORS_SYMBOL_BAR_FILTER_TOOLTIP,
        "kinds" => kinds.join(", ")
    )
    .into_string()
}

/// Build the dropdown overlay (panel + click-outside backstop) for
/// the currently-open menu. `None` when no menu open.
///
/// `top_padding_px`: see [`crate::library::editor::footprint::unified_active_bar::dropdown_overlay`].
pub fn dropdown_overlay<'a>(
    editor: &'a SymbolDocument,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
    top_padding_px: u16,
    i18n: &I18n,
) -> Option<iced::Element<'a, LibraryMessage>> {
    use iced::Length;
    use iced::Widget as _;
    use iced::widget::{Space, Stack, container, mouse_area};

    let menu = editor.active_bar_menu?;
    let entries = dropdowns::entries(
        menu,
        editor.selection_filter,
        editor.tool,
        editor.path.clone(),
        theme_id,
        i18n,
    );
    let width_hint = match menu {
        SymActiveBarMenu::Filter => None,
        SymActiveBarMenu::Snap => Some(240.0),
        SymActiveBarMenu::Place => Some(240.0),
        SymActiveBarMenu::Select => Some(220.0),
        SymActiveBarMenu::Align => Some(320.0),
        SymActiveBarMenu::Pin => Some(220.0),
        SymActiveBarMenu::Text => Some(180.0),
        SymActiveBarMenu::Shapes => Some(220.0),
    };
    let panel = signex_widgets::active_bar_dropdown::view(entries, tokens, width_hint);
    let panel_anchor = container(panel)
        .padding([top_padding_px, 10])
        .center_x(Length::Fill)
        .align_y(iced::alignment::Vertical::Top);
    let backstop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::CloseActiveBarMenu),
    });

    Some(
        Stack::new()
            .push(backstop.boxed())
            .push(panel_anchor.boxed())
            .boxed(),
    )
}

/// Dropdown trigger items for the SchLib bar. Same dual-action
/// pattern as the schematic / footprint bars: left-click runs the
/// default action (or toggles the menu when there's no obvious
/// default), right-click opens the dropdown.
fn dropdown_trigger_items(
    editor: &SymbolDocument,
    tid: ThemeId,
    i18n: &I18n,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active = editor.active_bar_menu;

    let dual = |label: &str,
                icon: ActiveBarIcon,
                menu: SymActiveBarMenu,
                left: Option<SymbolEditorMsg>|
     -> ActiveBarItem<LibraryMessage> {
        let on_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(left.unwrap_or(SymbolEditorMsg::ToggleActiveBarMenu(menu))),
        });
        let on_right_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::ToggleActiveBarMenu(menu)),
        });
        ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip: label.to_string(),
            enabled: true,
            selected: active == Some(menu),
            on_press,
            on_right_press,
            dropdown_indicator: Some(ActiveBarIcon::Svg(ic::icon_chevron_45(tid))),
        })
    };

    vec![
        dual(
            &filter_tooltip(i18n),
            ActiveBarIcon::Svg(ic::icon_filter(tid)),
            SymActiveBarMenu::Filter,
            None,
        ),
        dual(
            "Snap Options (left or right click for menu)",
            ActiveBarIcon::Svg(ic::icon_dd_align_grid(tid)),
            SymActiveBarMenu::Snap,
            None,
        ),
        dual(
            "Place / Move (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_move(tid)),
            SymActiveBarMenu::Place,
            // #426 — a symbol has no separate move tool: pin/graphic
            // movement IS drag-under-Select (see `dropdowns.rs`'s
            // `place_entries` comment). Left-click therefore arms
            // Select directly instead of routing through
            // `ActiveBarStub`, mirroring the footprint editor's
            // identical Move trigger.
            Some(SymbolEditorMsg::SetTool(SymbolToolMsg::Select)),
        ),
        dual(
            "Select (right-click for selection-mode menu)",
            ActiveBarIcon::Svg(ic::icon_select(tid)),
            SymActiveBarMenu::Select,
            Some(SymbolEditorMsg::SetTool(SymbolToolMsg::Select)),
        ),
        dual(
            "Align / Distribute (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_align(tid)),
            SymActiveBarMenu::Align,
            // #426 — real implementation: snaps the current selection
            // onto the symbol canvas's snap grid.
            Some(SymbolEditorMsg::AlignSelectedToGrid),
        ),
        dual(
            "Pin (left-click places a pin, right-click for variants)",
            ActiveBarIcon::Glyph("\u{2192}"),
            SymActiveBarMenu::Pin,
            Some(SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin)),
        ),
        dual(
            "Shapes (right-click for shape menu)",
            ActiveBarIcon::Svg(ic::icon_shapes(tid)),
            SymActiveBarMenu::Shapes,
            None,
        ),
        dual(
            "Text (left-click places String, right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_text(tid)),
            SymActiveBarMenu::Text,
            Some(SymbolEditorMsg::SetTool(SymbolToolMsg::PlaceText)),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{Symbol, SymbolFile};
    use std::path::PathBuf;

    fn new_editor() -> SymbolDocument {
        SymbolDocument::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        )
    }

    /// Unwrap a trigger button's left-click message down to the
    /// `SymbolEditorMsg` it carries — panics on anything but a
    /// `Button` with a `PrimitiveEditorEvent(Symbol(_))` `on_press`.
    fn left_click_symbol_msg(item: &ActiveBarItem<LibraryMessage>) -> &SymbolEditorMsg {
        let ActiveBarItem::Button(button) = item else {
            panic!("expected a Button item");
        };
        match button
            .on_press
            .as_ref()
            .expect("trigger button must have a left-click action")
        {
            LibraryMessage::PrimitiveEditorEvent {
                msg: PrimitiveEdit::Symbol(sym_msg),
                ..
            } => sym_msg,
            _ => panic!("expected a Symbol primitive-editor event"),
        }
    }

    /// #426 — the Place/Move trigger's left-click used to emit a dead
    /// `ActiveBarStub("Move")`. It now arms the Select tool directly,
    /// same as the Select trigger next to it: a symbol has no separate
    /// move tool, so Move IS drag-under-Select.
    #[test]
    fn move_trigger_left_click_arms_select_tool() {
        let editor = new_editor();
        let items = dropdown_trigger_items(&editor, ThemeId::GraphiteLight, &I18n::en_us());
        let place = &items[2]; // Filter, Snap, Place, Select, Align, Pin, Shapes, Text
        assert!(matches!(
            left_click_symbol_msg(place),
            SymbolEditorMsg::SetTool(SymbolToolMsg::Select)
        ));
    }

    /// #426 — the Align trigger's left-click used to emit a dead
    /// `ActiveBarStub("Align To Grid")`. It now dispatches the real
    /// `AlignSelectedToGrid` snap.
    #[test]
    fn align_trigger_left_click_snaps_selection_to_grid() {
        let editor = new_editor();
        let items = dropdown_trigger_items(&editor, ThemeId::GraphiteLight, &I18n::en_us());
        let align = &items[4];
        assert!(matches!(
            left_click_symbol_msg(align),
            SymbolEditorMsg::AlignSelectedToGrid
        ));
    }

    /// Mockup row 141: the bar has two groups split by one divider, selection
    /// (five triggers and the Select tool) then placement (Pin, Shapes,
    /// Text and the Place Pin tool); every trigger carries the dropdown
    /// indicator and no tool button does.
    #[test]
    fn the_bar_has_a_selection_and_a_placement_group() {
        let editor = new_editor();
        let items = bar_items(&editor, ThemeId::GraphiteLight, &I18n::en_us());
        let divider = items
            .iter()
            .position(|i| matches!(i, ActiveBarItem::Separator))
            .expect("one divider");
        assert_eq!(divider, SELECTION_TRIGGERS + 1);
        assert_eq!(
            items
                .iter()
                .filter(|i| matches!(i, ActiveBarItem::Separator))
                .count(),
            1
        );
        let tooltip = |i: &ActiveBarItem<LibraryMessage>| match i {
            ActiveBarItem::Button(b) => b.tooltip.clone(),
            _ => String::new(),
        };
        assert_eq!(tooltip(&items[divider - 1]), "Select");
        assert!(tooltip(&items[divider + 1]).starts_with("Pin"));
        assert!(tooltip(&items[divider + 2]).starts_with("Shapes"));
        assert!(tooltip(&items[divider + 3]).starts_with("Text"));
        assert_eq!(tooltip(items.last().expect("items")), "Place Pin");
        for (index, item) in items.iter().enumerate() {
            let ActiveBarItem::Button(b) = item else {
                continue;
            };
            let is_tool = index == divider - 1 || index == items.len() - 1;
            assert_eq!(b.dropdown_indicator.is_none(), is_tool, "{}", b.tooltip);
        }
    }

    /// The filter trigger's tooltip names the categories the filter has,
    /// each from the catalog.
    #[test]
    fn the_filter_tooltip_lists_the_filter_categories() {
        let i18n = I18n::en_us();
        let tooltip = filter_tooltip(&i18n).replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(
            tooltip,
            "Selection Filter: Pins, Drawings, Texts, Designators, Values, Parameters, Other \
             (left or right click for menu)"
        );
        for kind in FILTER_KINDS {
            let name = t!(i18n, ids::EDITORS_SYMBOL_BAR_FILTER_KIND, "kind" => kind.label());
            assert_eq!(name.into_string(), kind.label(), "en-US names the category");
        }
    }
}
