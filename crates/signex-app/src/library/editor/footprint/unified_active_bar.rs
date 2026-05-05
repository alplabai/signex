//! v0.18.14 — Altium-style unified active bar for the footprint
//! editor. Replaces the per-mode `pads_active_bar::view` /
//! `sketch_mode::active_bar::view` mounting in `standalone.rs`.
//!
//! v0.13 — Eight Altium dropdown menus (Filter / Snap / Place /
//! Select / Align / 3D Body / Text / Shapes) live at the FRONT of the
//! bar; their bodies come from `active_bar_dropdowns::entries`. The
//! bar's tool slot follows the active mode (`EditorMode::Sketch` →
//! sketch tools; `EditorMode::Normal` → pads tools; `EditorMode::View3d`
//! → no tools). The right-side per-kind Selection Filter pill row is
//! gone — the Filter dropdown supersedes it.

use iced::widget::{Stack, container};
use iced::{Element, Length};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::FootprintEditorState;
use crate::icons as ic;
use crate::library::editor::footprint::state::{EditorMode, FpActiveBarMenu};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};

/// Build the unified bar items and render the bar + open-dropdown
/// overlay (when one is open). Both layers carry `LibraryMessage` so
/// callers can mount them in a Stack alongside the canvas without a
/// message-type bridge.
pub fn view<'a>(
    editor: &'a FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
    custom_filter_presets: &[crate::active_bar::CustomFilterPreset],
) -> Element<'a, LibraryMessage> {
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

    let bar = signex_widgets::active_bar::view(items, tokens);

    // Bar centred at the top of the canvas.
    let bar_layer = container(bar)
        .padding([6, 10])
        .center_x(Length::Fill)
        .align_y(iced::alignment::Vertical::Top);

    // 3) Dropdown overlay (when open) — backstop layer captures
    // click-outside; the panel itself hosts the items.
    if let Some(menu) = editor.state.active_bar_menu {
        let entries = crate::library::editor::footprint::active_bar_dropdowns::entries(
            menu,
            &editor.state,
            editor.path.clone(),
            theme_id,
            custom_filter_presets,
        );
        // Per-menu width hint: list-style menus get a fixed width
        // (~longest label + icon column + padding) so hover paints a
        // full row; chip-grid menus (Filter) auto-size from the wrap
        // layout.
        let width_hint = match menu {
            FpActiveBarMenu::Filter => None, // chip wrap
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
            .padding([46, 10])
            .center_x(Length::Fill)
            .align_y(iced::alignment::Vertical::Top);

        // Backstop: full-area transparent button that closes the menu
        // when clicked outside the panel. Mounted UNDER the panel so
        // panel clicks don't fall through.
        let backstop = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: editor.path.clone(),
            msg: PrimitiveEditorMsg::FootprintCloseActiveBarMenu,
        });

        return Stack::new()
            .push(backstop)
            .push(bar_layer)
            .push(panel_anchor)
            .into();
    }

    bar_layer.into()
}

/// Build the 8 dropdown trigger buttons (Filter / Snap / Place /
/// Select / Align / 3D Body / Text / Shapes). Each click toggles the
/// matching `FpActiveBarMenu` on `state.active_bar_menu` via
/// `FootprintToggleActiveBarMenu`.
///
/// Icons reuse the schematic active bar's existing SVG set (themed,
/// accent-tinted) so the visual rhythm matches across editors. 3D
/// Body uses `icon_dd_graphic` until a dedicated 3D icon lands in
/// `assets/icons/`.
fn dropdown_trigger_items(
    editor: &FootprintEditorState,
    tid: ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active = editor.state.active_bar_menu;
    let trigger = |label: &str, icon: ActiveBarIcon, menu: FpActiveBarMenu| -> ActiveBarItem<LibraryMessage> {
        ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip: label.to_string(),
            enabled: true,
            selected: active == Some(menu),
            on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEditorMsg::FootprintToggleActiveBarMenu(menu),
            }),
            // Chevron indicator at the bottom-right of the button —
            // matches the schematic active bar's `dropdown_indicator`.
            dropdown_indicator: Some(ActiveBarIcon::Svg(ic::icon_chevron_45(tid))),
            ..ActiveBarButton::default()
        })
    };
    vec![
        trigger(
            "Selection Filter",
            ActiveBarIcon::Svg(ic::icon_filter(tid)),
            FpActiveBarMenu::Filter,
        ),
        trigger(
            "Snap Options",
            // No dedicated snap icon yet — borrow the align-grid glyph
            // since it visually communicates "snap to grid".
            ActiveBarIcon::Svg(ic::icon_dd_align_grid(tid)),
            FpActiveBarMenu::Snap,
        ),
        trigger(
            "Place / Move",
            ActiveBarIcon::Svg(ic::icon_move(tid)),
            FpActiveBarMenu::Place,
        ),
        trigger(
            "Selection",
            ActiveBarIcon::Svg(ic::icon_select(tid)),
            FpActiveBarMenu::Select,
        ),
        trigger(
            "Align / Distribute",
            ActiveBarIcon::Svg(ic::icon_align(tid)),
            FpActiveBarMenu::Align,
        ),
        trigger(
            "3D Body",
            ActiveBarIcon::Svg(ic::icon_dd_graphic(tid)),
            FpActiveBarMenu::Body3d,
        ),
        trigger(
            "Text",
            ActiveBarIcon::Svg(ic::icon_text(tid)),
            FpActiveBarMenu::Text,
        ),
        trigger(
            "Shapes",
            ActiveBarIcon::Svg(ic::icon_shapes(tid)),
            FpActiveBarMenu::Shapes,
        ),
    ]
}
