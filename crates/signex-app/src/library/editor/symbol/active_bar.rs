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

// v0.26-H — layout constants mirror `signex_widgets::active_bar`'s
// private constants (BTN_SIZE / SEP_W / BAR_PADDING / ROW_SPACING).
// Drift = misaligned dropdowns; the widget's constants are private
// so we mirror them here.
const BTN_SIZE: f32 = 26.0;
const SEP_W: f32 = 1.0;
const BAR_PADDING: f32 = 2.0;
const ROW_SPACING: f32 = 2.0;
const STEP_BTN: f32 = BTN_SIZE + ROW_SPACING;

/// Build the SchLib bar items only — caller mounts via
/// `signex_widgets::active_bar::view(items, tokens)` so the chain is
/// identical to the schematic.
pub fn bar_items(
    editor: &SymbolEditorState,
    theme_id: ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active_tool = editor.tool;

    let mut items: Vec<ActiveBarItem<LibraryMessage>> =
        dropdown_trigger_items(editor, theme_id);
    items.push(ActiveBarItem::Separator);

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
    items
}

/// v0.26-H — horizontal offset (in px) of a dropdown trigger
/// button's LEFT edge, measured from the bar's own left edge.
/// Mirror of [`crate::library::editor::footprint::unified_active_bar
/// ::dropdown_x_offset`] for the symbol editor's 8-dropdown layout.
pub fn dropdown_x_offset(menu: SymActiveBarMenu) -> f32 {
    let idx: f32 = match menu {
        SymActiveBarMenu::Filter => 0.0,
        SymActiveBarMenu::Snap => 1.0,
        SymActiveBarMenu::Place => 2.0,
        SymActiveBarMenu::Select => 3.0,
        SymActiveBarMenu::Align => 4.0,
        SymActiveBarMenu::Pin => 5.0,
        SymActiveBarMenu::Text => 6.0,
        SymActiveBarMenu::Shapes => 7.0,
    };
    BAR_PADDING + idx * STEP_BTN
}

/// v0.26-H — total bar width in px. Computed from item count so
/// dropdown positioning math knows the bar's centre-aligned left edge.
pub fn bar_width(editor: &SymbolEditorState, theme_id: ThemeId) -> f32 {
    let items = bar_items(editor, theme_id);
    let mut w = 2.0 * BAR_PADDING;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            w += ROW_SPACING;
        }
        match item {
            ActiveBarItem::Button(_) => w += BTN_SIZE,
            ActiveBarItem::Separator => w += SEP_W,
            ActiveBarItem::Custom(_) => w += 60.0,
        }
    }
    w
}

/// Build the dropdown overlay (panel + click-outside backstop) for
/// the currently-open menu. `None` when no menu open.
///
/// v0.26-H — Translate-positioned so the panel lands directly below
/// the trigger button instead of centred in the viewport.
///
/// `top_padding_px`: see [`crate::library::editor::footprint::unified_active_bar::dropdown_overlay`].
/// `window_width`: current window pixel width — for bar centre math.
pub fn dropdown_overlay<'a>(
    editor: &'a SymbolEditorState,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
    top_padding_px: u16,
    window_width: f32,
) -> Option<iced::Element<'a, LibraryMessage>> {
    use iced::widget::{Stack, container, mouse_area, Space};
    use iced::Length;

    let menu = editor.active_bar_menu?;
    let entries = crate::library::editor::symbol::active_bar_dropdowns::entries(
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

    let bar_w = bar_width(editor, theme_id);
    let bar_left = ((window_width - bar_w) / 2.0).max(0.0);
    let panel_w = width_hint.unwrap_or(220.0);
    let raw_x = bar_left + dropdown_x_offset(menu);
    let edge_margin: f32 = 4.0;
    let abs_x = if raw_x + panel_w + edge_margin > window_width {
        (window_width - panel_w - edge_margin).max(0.0)
    } else {
        raw_x
    };

    let panel_anchor =
        crate::app::view::translate::Translate::new(panel, (abs_x, top_padding_px as f32));

    let backstop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEditorMsg::SymbolCloseActiveBarMenu,
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
