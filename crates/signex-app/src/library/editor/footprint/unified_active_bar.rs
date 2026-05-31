//! v0.18.14 — Altium-style unified active bar for the footprint
//! editor. Replaces the per-mode `pads_active_bar::view` /
//! `sketch_mode::active_bar::view` mounting in `standalone.rs`.
//!
//! v0.13 — Public surface split into `bar_items()` + `dropdown_overlay()`
//! so the layer-site mounting code at `view_main_for` calls
//! `signex_widgets::active_bar::view(items, tokens).map(...)` directly,
//! BYTE-FOR-BYTE matching the schematic active bar's chain. This
//! prevents `Element::map` ordering drift (Map-wraps-container vs
//! container-wraps-Map) that introduced a 2 px layout-pass shift.

use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::FootprintEditorState;
use crate::icons as ic;
use crate::library::editor::footprint::state::{EditorMode, FpActiveBarMenu};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};

// v0.26-H — layout constants for per-button dropdown positioning.
// MUST stay in sync with `signex_widgets::active_bar` (BTN_SIZE,
// SEP_W, BAR_PADDING, ROW_SPACING). Drift here = misaligned
// dropdowns; the widget's constants are private so we mirror them.
const BTN_SIZE: f32 = 26.0;
const SEP_W: f32 = 1.0;
const BAR_PADDING: f32 = 2.0;
const ROW_SPACING: f32 = 2.0;
/// Per-step horizontal advance: a button + the spacing after it.
const STEP_BTN: f32 = BTN_SIZE + ROW_SPACING;
/// Per-step horizontal advance for a separator: width + spacing.
const STEP_SEP: f32 = SEP_W + ROW_SPACING;

/// Build the bar items only — caller mounts via
/// `signex_widgets::active_bar::view(items, tokens)` so the chain is
/// identical to the schematic.
pub fn bar_items(
    editor: &FootprintEditorState,
    theme_id: ThemeId,
    tokens: &ThemeTokens,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let mut items: Vec<ActiveBarItem<LibraryMessage>> = Vec::new();

    // 1) Eight active-bar dropdown trigger buttons at the FRONT.
    items.extend(dropdown_trigger_items(editor, theme_id));
    items.push(ActiveBarItem::Separator);

    // 2) Mode-keyed tool buttons (pads / sketch / 3D-view).
    let mode_items: Vec<ActiveBarItem<LibraryMessage>> = match editor.state.mode {
        EditorMode::Sketch => crate::library::editor::footprint::sketch_mode::active_bar::items(
            editor, theme_id, tokens,
        ),
        EditorMode::Normal => {
            crate::library::editor::footprint::pads_active_bar::items(editor, theme_id, tokens)
        }
        EditorMode::View3d => Vec::new(),
    };
    items.extend(mode_items);
    items
}

/// v0.26-H — horizontal offset (in px) of a dropdown trigger
/// button's LEFT edge, measured from the bar's own left edge.
/// Use [`bar_width`] to convert to a window-absolute coordinate:
///   `abs_x = bar_left_x + dropdown_x_offset(menu)`
/// where `bar_left_x = (window_width - bar_width) / 2.0`.
///
/// Order matches [`dropdown_trigger_items`]: Filter, Snap, Place,
/// Select, Align, Body3d, Text, Shapes. The bar starts with these
/// 8 buttons before any separator, so each step is one
/// [`STEP_BTN`].
pub fn dropdown_x_offset(menu: FpActiveBarMenu) -> f32 {
    let idx: f32 = match menu {
        FpActiveBarMenu::Filter => 0.0,
        FpActiveBarMenu::Snap => 1.0,
        FpActiveBarMenu::Place => 2.0,
        FpActiveBarMenu::Select => 3.0,
        FpActiveBarMenu::Align => 4.0,
        FpActiveBarMenu::Body3d => 5.0,
        FpActiveBarMenu::Text => 6.0,
        FpActiveBarMenu::Shapes => 7.0,
    };
    BAR_PADDING + idx * STEP_BTN
}

/// v0.26-H — total bar width in px. Mirrors the [`bar_items`] item
/// list so dropdown positioning math knows where the bar's left
/// edge sits when iced auto-sizes + centre-aligns the bar.
pub fn bar_width(
    editor: &FootprintEditorState,
    theme_id: ThemeId,
    tokens: &ThemeTokens,
) -> f32 {
    let items = bar_items(editor, theme_id, tokens);
    let mut w = 2.0 * BAR_PADDING;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            w += ROW_SPACING;
        }
        match item {
            ActiveBarItem::Button(_) => w += BTN_SIZE,
            ActiveBarItem::Separator => w += SEP_W,
            // Sketch-mode bar uses one Custom item for the dimension
            // input (58 px wide). Approximation: 60 px. Off-by-a-few
            // px is fine — the dropdown trigger buttons live in the
            // first 8 slots BEFORE any Custom item, so the offset
            // computation itself stays exact; only the bar's centre
            // alignment drifts by half the estimate error.
            ActiveBarItem::Custom(_) => w += 60.0,
        }
    }
    w
}

/// Build the dropdown overlay (panel + click-outside backstop) for
/// the currently-open menu. Returns `None` when no menu is open.
/// Caller pushes the result as a separate layer above the bar.
///
/// v0.26-H — dropdown is now Translate-positioned so it lands
/// directly under the trigger button instead of being centered
/// in the viewport.
///
/// `top_padding_px` is the y-offset (from the overlay's top edge,
/// which sits at window y=0) where the dropdown panel should land —
/// callers compute it as `y_offset + 4 + bar_height + small_gap` so
/// the panel touches the bar's bottom edge regardless of whether the
/// tab strip is showing.
///
/// `window_width` is the current window's pixel width — needed to
/// compute the bar's left edge when iced centre-aligns it.
pub fn dropdown_overlay<'a>(
    editor: &'a FootprintEditorState,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
    custom_filter_presets: &[crate::active_bar::CustomFilterPreset],
    top_padding_px: u16,
    window_width: f32,
) -> Option<iced::Element<'a, LibraryMessage>> {
    use iced::widget::{Stack, container, mouse_area, Space};
    use iced::Length;

    let menu = editor.state.active_bar_menu?;

    let entries = crate::library::editor::footprint::active_bar_dropdowns::entries(
        menu,
        &editor.state,
        editor.path.clone(),
        theme_id,
        custom_filter_presets,
    );
    let width_hint = match menu {
        FpActiveBarMenu::Filter => None,
        FpActiveBarMenu::Snap => Some(260.0),
        FpActiveBarMenu::Place => Some(240.0),
        FpActiveBarMenu::Select => Some(220.0),
        FpActiveBarMenu::Align => Some(320.0),
        FpActiveBarMenu::Body3d => Some(200.0),
        FpActiveBarMenu::Text => Some(180.0),
        FpActiveBarMenu::Shapes => Some(220.0),
    };
    let panel = signex_widgets::active_bar_dropdown::view(entries, tokens, width_hint);

    // v0.26-H — compute the dropdown's absolute window x as
    //   bar_left + dropdown_x_offset(menu)
    // where bar_left = (window_width - bar_width) / 2 (since the
    // mounting site centre-aligns the bar). Clamp the dropdown's
    // right edge inside the viewport so a near-rightmost button
    // still keeps its panel fully visible.
    let bar_w = bar_width(editor, theme_id, tokens);
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
        msg: PrimitiveEditorMsg::FootprintCloseActiveBarMenu,
    });

    Some(Stack::new().push(backstop).push(panel_anchor).into())
}

/// Build the 8 dropdown trigger buttons matching the schematic's
/// pattern: left-click fires the default action (or toggles the
/// menu when there's no obvious default — Filter / Snap), right-click
/// opens the dropdown. Chevron indicator advertises the right-click
/// secondary action.
fn dropdown_trigger_items(
    editor: &FootprintEditorState,
    tid: ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active = editor.state.active_bar_menu;

    let dual = |label: &str,
                icon: ActiveBarIcon,
                menu: FpActiveBarMenu,
                left: Option<PrimitiveEditorMsg>|
         -> ActiveBarItem<LibraryMessage> {
        let on_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: left.unwrap_or(PrimitiveEditorMsg::FootprintToggleActiveBarMenu(menu)),
        });
        let on_right_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::FootprintToggleActiveBarMenu(menu),
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
            FpActiveBarMenu::Filter,
            None,
        ),
        dual(
            "Snap Options (left or right click for menu)",
            ActiveBarIcon::Svg(ic::icon_dd_align_grid(tid)),
            FpActiveBarMenu::Snap,
            None,
        ),
        dual(
            "Place / Move (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_move(tid)),
            FpActiveBarMenu::Place,
            Some(PrimitiveEditorMsg::FootprintActiveBarStub("Move")),
        ),
        dual(
            "Select (right-click for selection-mode menu)",
            ActiveBarIcon::Svg(ic::icon_select(tid)),
            FpActiveBarMenu::Select,
            Some(PrimitiveEditorMsg::FootprintSetPadsTool(
                crate::library::editor::footprint::state::PadsTool::Select,
            )),
        ),
        dual(
            "Align / Distribute (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_align(tid)),
            FpActiveBarMenu::Align,
            Some(PrimitiveEditorMsg::FootprintActiveBarAlignSelectionToGrid),
        ),
        dual(
            "3D Body (left or right click for menu)",
            ActiveBarIcon::Svg(ic::icon_dd_graphic(tid)),
            FpActiveBarMenu::Body3d,
            None,
        ),
        dual(
            "Text (left-click places String, right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_text(tid)),
            FpActiveBarMenu::Text,
            Some(PrimitiveEditorMsg::FootprintSetPadsTool(
                crate::library::editor::footprint::state::PadsTool::PlaceString,
            )),
        ),
        dual(
            "Shapes (right-click for sketch-mode shape menu)",
            ActiveBarIcon::Svg(ic::icon_shapes(tid)),
            FpActiveBarMenu::Shapes,
            None,
        ),
    ]
}
