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

/// Build the dropdown overlay (panel + click-outside backstop) for
/// the currently-open menu. Returns `None` when no menu is open.
/// Caller pushes the result as a separate layer above the bar.
///
/// `top_padding_px` is the y-offset (from the overlay's top edge,
/// which sits at window y=0) where the dropdown panel should land —
/// callers compute it as `y_offset + 4 + bar_height + small_gap` so
/// the panel touches the bar's bottom edge regardless of whether the
/// tab strip is showing. Hard-coded values drift when the menu bar
/// or tab-strip heights shift, so the caller owns the formula.
pub fn dropdown_overlay<'a>(
    editor: &'a FootprintEditorState,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
    custom_filter_presets: &[crate::active_bar::CustomFilterPreset],
    top_padding_px: u16,
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
