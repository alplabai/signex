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

use iced::Element;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::FootprintEditorState;
use crate::icons as ic;
use crate::library::editor::footprint::state::{EditorMode, FpActiveBarMenu};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};

/// Build the unified bar items and render the bar + open-dropdown
/// overlay (when one is open) — single-call API via
/// `signex_widgets::active_bar::view_with_overlay`. The widget
/// handles the bar centring, dropdown panel anchoring, and the
/// click-outside backstop layer.
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

    // Capture context for the closures used by the unified widget.
    let path_for_entries = editor.path.clone();
    let presets_for_entries = custom_filter_presets.to_vec();
    let state_snapshot_path = editor.path.clone();
    // Unsafe-free workaround: clone the snapshot needed by entries()
    // closure. FootprintEditorState is large but we only read
    // selection_filter / snap_options / pads_tool / snapping_mode /
    // snap_subtab / active_bar_menu through it. Cloning the inner
    // state struct keeps the closure 'static-friendly.
    let state_clone = editor.state.clone();
    let close_msg = LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEditorMsg::FootprintCloseActiveBarMenu,
    };
    let _ = state_snapshot_path;

    signex_widgets::active_bar::view_with_overlay::<LibraryMessage, FpActiveBarMenu>(
        items,
        editor.state.active_bar_menu,
        close_msg,
        move |menu| {
            crate::library::editor::footprint::active_bar_dropdowns::entries(
                menu,
                &state_clone,
                path_for_entries.clone(),
                theme_id,
                &presets_for_entries,
            )
        },
        |menu| match menu {
            FpActiveBarMenu::Filter => None,
            FpActiveBarMenu::Snap => Some(260.0),
            FpActiveBarMenu::Place => Some(240.0),
            FpActiveBarMenu::Select => Some(220.0),
            FpActiveBarMenu::Align => Some(320.0),
            FpActiveBarMenu::Body3d => Some(200.0),
            FpActiveBarMenu::Text => Some(180.0),
            FpActiveBarMenu::Shapes => Some(220.0),
        },
        tokens,
    )
}

/// Build the 8 dropdown trigger buttons matching the schematic's
/// pattern: left-click fires the default action (or toggles the
/// menu when there's no obvious default — Filter / Snap), right-click
/// opens the dropdown. Chevron indicator advertises the right-click
/// secondary action.
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

    // Build a button with BOTH left-action and right-click dropdown.
    // Mirrors the schematic active bar's `btn` helper. When `left` is
    // None the left-click also toggles the menu — used for surfaces
    // (Filter / Snap) that have no obvious "default action".
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
        // Filter / Snap — no clear default action; both left and right
        // toggle the dropdown. Same as the schematic's Filter button.
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
        // Place / Move — left-click runs the last-used Place command
        // (defaults to Move Selection); right-click opens the menu.
        dual(
            "Place / Move (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_move(tid)),
            FpActiveBarMenu::Place,
            Some(PrimitiveEditorMsg::FootprintActiveBarStub("Move")),
        ),
        // Selection — left-click switches to Select tool; right-click
        // opens the Selection-mode menu (Inside Area / Lasso / etc.).
        dual(
            "Select (right-click for selection-mode menu)",
            ActiveBarIcon::Svg(ic::icon_select(tid)),
            FpActiveBarMenu::Select,
            Some(PrimitiveEditorMsg::FootprintSetPadsTool(
                crate::library::editor::footprint::state::PadsTool::Select,
            )),
        ),
        // Align — left-click runs Align To Grid; right-click opens the
        // Align/Distribute menu.
        dual(
            "Align / Distribute (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_align(tid)),
            FpActiveBarMenu::Align,
            Some(PrimitiveEditorMsg::FootprintActiveBarAlignSelectionToGrid),
        ),
        // 3D Body / Text / Shapes — left-click opens the menu since
        // there's no single "default" placement action.
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
