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
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

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

/// Where `menu`'s trigger button sits: its left edge in px from the
/// bar's own left edge, plus the bar's total width. `None` when the
/// current bar has no trigger for that menu (the two sketch group
/// menus only exist in `EditorMode::Sketch`).
///
/// Both numbers are **derived from the items the bar just built**, not
/// from a table of slot indices. That matters: the previous version
/// hand-maintained a `Filter => 0, Snap => 1, …` index map plus its own
/// copies of the widget's pixel constants, so every button added,
/// removed, or reordered was a chance to silently misplace every panel.
///
/// The trigger is located by its *message*, not its position — every
/// menu trigger publishes `ToggleActiveBarMenu(menu)` on right-press,
/// while the left-press action varies (Place / Select / Text arm a tool
/// instead). Nothing else on the bar sends that message.
fn menu_trigger_geometry(
    items: &[ActiveBarItem<LibraryMessage>],
    menu: FpActiveBarMenu,
) -> (Option<f32>, f32) {
    let (offsets, total) = signex_widgets::active_bar::slot_offsets(items);
    let opens = |item: &ActiveBarItem<LibraryMessage>| -> bool {
        let ActiveBarItem::Button(b) = item else {
            return false;
        };
        matches!(
            &b.on_right_press,
            Some(LibraryMessage::PrimitiveEditorEvent {
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleActiveBarMenu(m)),
                ..
            }) if *m == menu
        )
    };
    (items.iter().position(opens).map(|i| offsets[i]), total)
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
    footprint_filter_presets: &[crate::active_bar::FootprintFilterPreset],
    top_padding_px: u16,
    window_width: f32,
) -> Option<iced::Element<'a, LibraryMessage>> {
    use iced::Length;
    use iced::widget::{Space, Stack, container, mouse_area};

    let menu = editor.state.active_bar_menu?;

    let entries = crate::library::editor::footprint::active_bar_dropdowns::entries(
        menu,
        &editor.state,
        editor.path.clone(),
        theme_id,
        footprint_filter_presets,
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
        // Wide enough for "Rounded Rectangle" / "Rectangular Pattern" /
        // "Make Pad from Profile" plus the leading icon.
        FpActiveBarMenu::SketchCreate => Some(230.0),
        FpActiveBarMenu::SketchModify => Some(250.0),
    };
    let panel = signex_widgets::active_bar_dropdown::view(entries, tokens, width_hint);

    // v0.26-H — the panel lands under its own trigger button:
    //   bar_left + <that button's offset within the bar>
    // where bar_left = (window_width - bar_width) / 2, since the
    // mounting site centre-aligns the bar. Both come from measuring
    // the items this same bar just built. Clamp the panel's right edge
    // inside the viewport so a near-rightmost button still keeps it
    // fully visible.
    let (trigger_x, bar_w) = menu_trigger_geometry(&bar_items(editor, theme_id, tokens), menu);
    // No trigger on the current bar means nothing to anchor to — a
    // stale menu key from a mode switch. Draw nothing rather than
    // park a panel over an unrelated button.
    let trigger_x = trigger_x?;
    let bar_left = ((window_width - bar_w) / 2.0).max(0.0);
    let panel_w = width_hint.unwrap_or(220.0);
    let raw_x = bar_left + trigger_x;
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
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CloseActiveBarMenu),
    });

    Some(Stack::new().push(backstop).push(panel_anchor).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_widgets::active_bar::{BAR_PADDING, BTN_SIZE, ROW_SPACING};

    fn editor_in(
        mode: crate::library::editor::footprint::state::EditorMode,
    ) -> FootprintEditorState {
        use signex_library::{Footprint, FootprintFile};
        let file = FootprintFile::from_footprint(Footprint::empty("t"));
        let mut editor =
            crate::app::FootprintEditorState::new(std::path::PathBuf::from("t.snxfpt"), file);
        editor.state.mode = mode;
        editor
    }

    /// Panel anchors are located by scanning the built bar for the
    /// button that opens each menu. Confirm the scan actually finds
    /// them, in the right order, and that a menu whose trigger isn't on
    /// the current bar reports `None` instead of a bogus offset.
    #[test]
    fn menu_triggers_are_located_by_message_not_by_index() {
        use crate::library::editor::footprint::state::EditorMode;
        use signex_types::theme::{ThemeId, theme_tokens};

        let tid = ThemeId::Signex;
        let tokens = theme_tokens(tid);

        let sketch = editor_in(EditorMode::Sketch);
        let items = bar_items(&sketch, tid, &tokens);
        let at = |m| menu_trigger_geometry(&items, m).0;

        // The first shared trigger sits flush against the bar padding.
        assert_eq!(at(FpActiveBarMenu::Filter), Some(BAR_PADDING));
        // Later shared triggers advance one button-plus-spacing each.
        assert_eq!(
            at(FpActiveBarMenu::Snap),
            Some(BAR_PADDING + BTN_SIZE + ROW_SPACING)
        );
        // Both sketch groups are present, Create left of Modify, and
        // both right of every shared trigger.
        let (create, modify) = (
            at(FpActiveBarMenu::SketchCreate).expect("Create trigger missing in Sketch mode"),
            at(FpActiveBarMenu::SketchModify).expect("Modify trigger missing in Sketch mode"),
        );
        assert!(create < modify);
        assert!(at(FpActiveBarMenu::Shapes).is_some_and(|shapes| shapes < create));

        // Pads mode has no sketch group triggers — and must say so
        // rather than hand back a stale offset.
        let pads = editor_in(EditorMode::Normal);
        let pads_items = bar_items(&pads, tid, &tokens);
        assert_eq!(
            menu_trigger_geometry(&pads_items, FpActiveBarMenu::SketchCreate).0,
            None
        );
        assert!(
            menu_trigger_geometry(&pads_items, FpActiveBarMenu::Filter)
                .0
                .is_some()
        );
    }

    /// The measured width has to match what the bar actually draws, or
    /// the centre-aligned bar's left edge is wrong and every panel
    /// shifts by half the error. Guards the Custom slot in particular:
    /// its width is declared, not measured.
    #[test]
    fn bar_width_counts_every_slot_including_the_custom_one() {
        use crate::library::editor::footprint::sketch_mode::active_bar::DIM_INPUT_W;
        use crate::library::editor::footprint::state::EditorMode;
        use signex_types::theme::{ThemeId, theme_tokens};

        let tid = ThemeId::Signex;
        let tokens = theme_tokens(tid);
        let editor = editor_in(EditorMode::Sketch);
        let items = bar_items(&editor, tid, &tokens);

        let customs = items
            .iter()
            .filter(|i| matches!(i, ActiveBarItem::Custom { .. }))
            .count();
        assert_eq!(customs, 1, "sketch bar should carry the dimension input");

        let expected: f32 = 2.0 * BAR_PADDING
            + items.iter().map(|i| i.width()).sum::<f32>()
            + ROW_SPACING * (items.len() - 1) as f32;
        let (_, measured) = menu_trigger_geometry(&items, FpActiveBarMenu::Filter);
        assert!(
            (measured - expected).abs() < 0.01,
            "{measured} vs {expected}"
        );
        // And the Custom slot is contributing its declared width, not a
        // button's — the bug that shifted every panel ~13 px right.
        assert!(DIM_INPUT_W > BTN_SIZE);
    }
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
                left: Option<FootprintEditorMsg>|
     -> ActiveBarItem<LibraryMessage> {
        let on_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(
                left.unwrap_or(FootprintEditorMsg::ToggleActiveBarMenu(menu)),
            ),
        });
        let on_right_press = Some(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleActiveBarMenu(menu)),
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
            "Move (right-click for move / transform menu)",
            ActiveBarIcon::Svg(ic::icon_move(tid)),
            FpActiveBarMenu::Place,
            // A footprint has no separate move tool: pad movement IS
            // drag-under-Select (see `active_bar_dropdowns.rs`'s
            // `place_entries` — every row in this menu arms Select).
            // Left-click therefore arms Select directly instead of
            // routing through `ActiveBarStub`, which only logged
            // "coming soon" and never opened the menu either.
            Some(FootprintEditorMsg::SetPadsTool(
                crate::library::editor::footprint::state::PadsTool::Select,
            )),
        ),
        dual(
            "Select (right-click for selection-mode menu)",
            ActiveBarIcon::Svg(ic::icon_select(tid)),
            FpActiveBarMenu::Select,
            Some(FootprintEditorMsg::SetPadsTool(
                crate::library::editor::footprint::state::PadsTool::Select,
            )),
        ),
        dual(
            "Align / Distribute (right-click for menu)",
            ActiveBarIcon::Svg(ic::icon_align(tid)),
            FpActiveBarMenu::Align,
            Some(FootprintEditorMsg::ActiveBarAlignSelectionToGrid),
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
            Some(FootprintEditorMsg::SetPadsTool(
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
