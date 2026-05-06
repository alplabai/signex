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
use crate::library::editor::symbol::state::{
    SymActiveBarMenu, SymbolSelectionFilter,
};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg, SymbolToolMsg};

/// Build the SchLib bar items + render via the unified widget.
pub fn view<'a>(
    editor: &'a SymbolEditorState,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    let path = editor.path.clone();
    let active_tool = editor.tool;
    let selection_filter = editor.selection_filter;

    // 1) Eight chevron-trigger buttons (Filter / Snap / Place /
    // Select / Align / Pin / Text / Shapes) at the FRONT.
    let mut items: Vec<ActiveBarItem<LibraryMessage>> =
        dropdown_trigger_items(editor, theme_id);
    items.push(ActiveBarItem::Separator);

    // 2) Tool slots — Select + Place Pin. Other shape tools live in
    // the Shapes dropdown.
    items.push(ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(ic::icon_select(theme_id)),
        tooltip: "Select".into(),
        enabled: true,
        selected: active_tool == SymbolTool::Select,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::Select),
        }),
        ..ActiveBarButton::default()
    }));
    items.push(ActiveBarItem::Button(ActiveBarButton {
        // No dedicated pin SVG yet — use the arrow glyph.
        icon: ActiveBarIcon::Glyph("\u{2192}"),
        tooltip: "Place Pin".into(),
        enabled: true,
        selected: active_tool == SymbolTool::AddPin,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::AddPin),
        }),
        ..ActiveBarButton::default()
    }));

    let close_msg = LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::SymbolCloseActiveBarMenu,
    };
    let path_for_entries = path.clone();

    signex_widgets::active_bar::view_with_overlay::<LibraryMessage, SymActiveBarMenu>(
        items,
        editor.active_bar_menu,
        close_msg,
        move |menu| {
            crate::library::editor::symbol::active_bar_dropdowns::entries(
                menu,
                selection_filter,
                active_tool,
                path_for_entries.clone(),
                theme_id,
            )
        },
        |menu| match menu {
            SymActiveBarMenu::Filter => None,
            SymActiveBarMenu::Snap => Some(240.0),
            SymActiveBarMenu::Place => Some(240.0),
            SymActiveBarMenu::Select => Some(220.0),
            SymActiveBarMenu::Align => Some(320.0),
            SymActiveBarMenu::Pin => Some(220.0),
            SymActiveBarMenu::Text => Some(180.0),
            SymActiveBarMenu::Shapes => Some(220.0),
        },
        tokens,
    )
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
                left: Option<PrimitiveEditorMsg>|
         -> ActiveBarItem<LibraryMessage> {
        let on_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: left.unwrap_or(PrimitiveEditorMsg::SymbolToggleActiveBarMenu(menu)),
        });
        let on_right_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::SymbolToggleActiveBarMenu(menu),
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
            Some(PrimitiveEditorMsg::SymbolActiveBarStub("Move")),
        ),
        dual(
            "Select (right-click for selection-mode menu)",
            ActiveBarIcon::Svg(ic::icon_select(tid)),
            SymActiveBarMenu::Select,
            Some(PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::Select)),
        ),
        dual(
            "Align / Distribute (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_align(tid)),
            SymActiveBarMenu::Align,
            Some(PrimitiveEditorMsg::SymbolActiveBarStub("Align To Grid")),
        ),
        dual(
            "Pin (left-click places a pin, right-click for variants)",
            ActiveBarIcon::Glyph("\u{2192}"),
            SymActiveBarMenu::Pin,
            Some(PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::AddPin)),
        ),
        dual(
            "Text (left-click places String, right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_text(tid)),
            SymActiveBarMenu::Text,
            Some(PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::PlaceText)),
        ),
        dual(
            "Shapes (right-click for shape menu)",
            ActiveBarIcon::Svg(ic::icon_shapes(tid)),
            SymActiveBarMenu::Shapes,
            None,
        ),
    ]
}
