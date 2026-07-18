//! SchLib editor's Active Bar — the floating tool bar over the
//! `.snxsym` canvas, mirroring the schematic editor's Altium-style
//! Active Bar pattern but with SchLib-specific tools.
//!
//! v0.13 — Eight Altium dropdown menus (Filter / Snap / Place /
//! Select / Align / Pin / Text / Shapes) live at the FRONT of the
//! bar; their bodies come from `active_bar_dropdowns::entries`. The
//! Select / Place Pin tool slots stay at the end of the bar for
//! quick keyboard / single-click access. Pure-graphics tools (Line /
//! Arc / Circle / Rectangle) live in the Shapes dropdown to keep the
//! bar slim.
//!
//! Built on top of the unified
//! `signex_widgets::active_bar::view_with_overlay` so a single call
//! returns the bar + dropdown overlay + click-outside backstop —
//! identical pattern across schematic / footprint / SchLib /
//! upcoming PCB editors.

use std::path::PathBuf;

use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::SymbolEditorState;
use crate::icons as ic;
use crate::library::editor::symbol::canvas::SymbolTool;
use crate::library::editor::symbol::state::{SymActiveBarMenu, SymbolSelectionFilter};
use crate::library::messages::{LibraryMessage, PrimitiveEdit, SymbolEditorMsg, SymbolToolMsg};

mod dropdowns;

/// Build the SchLib bar items only — caller mounts via
/// `signex_widgets::active_bar::view(items, tokens)` so the chain is
/// identical to the schematic.
pub fn bar_items(
    editor: &SymbolEditorState,
    theme_id: ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active_tool = editor.tool;

    let mut items: Vec<ActiveBarItem<LibraryMessage>> = dropdown_trigger_items(editor, theme_id);
    items.push(ActiveBarItem::Separator);

    items.push(ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(ic::icon_select(theme_id)),
        tooltip: "Select".into(),
        enabled: true,
        selected: active_tool == SymbolTool::Select,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::SetTool(SymbolToolMsg::Select)),
        }),
        ..ActiveBarButton::default()
    }));
    items.push(ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{2192}"),
        tooltip: "Place Pin".into(),
        enabled: true,
        selected: active_tool == SymbolTool::AddPin,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Symbol(SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin)),
        }),
        ..ActiveBarButton::default()
    }));
    items
}

/// Build the dropdown overlay (panel + click-outside backstop) for
/// the currently-open menu. `None` when no menu open.
///
/// `top_padding_px`: see [`crate::library::editor::footprint::unified_active_bar::dropdown_overlay`].
pub fn dropdown_overlay<'a>(
    editor: &'a SymbolEditorState,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
    top_padding_px: u16,
) -> Option<iced::Element<'a, LibraryMessage>> {
    use iced::Length;
    use iced::widget::{Space, Stack, container, mouse_area};

    let menu = editor.active_bar_menu?;
    let entries = dropdowns::entries(
        menu,
        editor.selection_filter,
        editor.tool,
        editor.path.clone(),
        theme_id,
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

    Some(Stack::new().push(backstop).push(panel_anchor).into())
}

/// Dropdown trigger items for the SchLib bar. Same dual-action
/// pattern as the schematic / footprint bars: left-click runs the
/// default action (or toggles the menu when there's no obvious
/// default), right-click opens the dropdown.
fn dropdown_trigger_items(
    editor: &SymbolEditorState,
    tid: ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active = editor.active_bar_menu;
    let _ = SymbolSelectionFilter::default; // silences unused-import lint

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
            "Selection Filter (left or right click for menu)",
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
            Some(SymbolEditorMsg::ActiveBarStub("Move")),
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
            Some(SymbolEditorMsg::ActiveBarStub("Align To Grid")),
        ),
        dual(
            "Pin (left-click places a pin, right-click for variants)",
            ActiveBarIcon::Glyph("\u{2192}"),
            SymActiveBarMenu::Pin,
            Some(SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin)),
        ),
        dual(
            "Text (left-click places String, right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_text(tid)),
            SymActiveBarMenu::Text,
            Some(SymbolEditorMsg::SetTool(SymbolToolMsg::PlaceText)),
        ),
        dual(
            "Shapes (right-click for shape menu)",
            ActiveBarIcon::Svg(ic::icon_shapes(tid)),
            SymActiveBarMenu::Shapes,
            None,
        ),
    ]
}
