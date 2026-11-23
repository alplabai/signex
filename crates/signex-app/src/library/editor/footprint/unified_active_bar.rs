//! The footprint editor's floating active bar.
//!
//! In Sketch mode it is the shared sketch bar (#2518). Otherwise three
//! groups split by a divider: selection (Filter, Snap, Move, Select, Align
//! and the Pads-mode Select tool), placement (the Pads-mode place tools,
//! 3D Body, Text) and sketch (Shapes). A button with a dropdown carries the
//! corner indicator.
//!
//! The public surface is `bar_items()` + `dropdown_overlay()`, so the
//! mounting code calls `signex_widgets::active_bar::view(items, tokens)`
//! exactly as the schematic active bar does; a different `Element::map`
//! nesting shifted the bar by 2 px.

use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::FootprintDocument;
use crate::icons as ic;
use crate::library::editor::footprint::state::{EditorMode, FpActiveBarMenu};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Build the bar items only — caller mounts via
/// `signex_widgets::active_bar::view(items, tokens)` so the chain is
/// identical to the schematic.
pub fn bar_items(
    editor: &FootprintDocument,
    theme_id: ThemeId,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    // Sketch mode shows the one sketch bar on both hosts (#2518): a board
    // sketch is always in sketch mode, and a footprint in sketch mode gets
    // the same tools, ending in its own Snap ▾.
    if editor.state.mode == EditorMode::Sketch {
        return crate::library::editor::footprint::sketch_mode::active_bar::items(
            editor, theme_id, tokens, i18n,
        );
    }

    use crate::library::editor::footprint::pads_active_bar;
    let mut items: Vec<ActiveBarItem<LibraryMessage>> = Vec::new();
    let (mut selection, mut placement, mut sketch) = (Vec::new(), Vec::new(), Vec::new());
    for (group, item) in dropdown_trigger_items(editor, theme_id) {
        match group {
            TriggerGroup::Selection => selection.push(item),
            TriggerGroup::Placement => placement.push(item),
            TriggerGroup::Sketch => sketch.push(item),
        }
    }
    let is_pads = editor.state.mode == EditorMode::Normal;

    // Selection: Filter, Snap, Move, Select, Align, then the Pads-mode
    // Select tool.
    items.extend(selection);
    if is_pads {
        items.extend(pads_active_bar::selection_items(editor, theme_id));
    }
    items.push(ActiveBarItem::Separator);

    // Placement: the Pads-mode place tools, then 3D Body and Text.
    if is_pads {
        items.extend(pads_active_bar::placement_items(editor, theme_id));
    }
    items.extend(placement);
    items.push(ActiveBarItem::Separator);

    // Sketch: Shapes (the sketch tools are the sketch bar above).
    items.extend(sketch);
    items
}

/// The slots that scroll in a window too narrow for the bar: the sketch
/// bar's constraint strip; no other bar has a run.
pub fn scrolling_slots(editor: &FootprintDocument) -> std::ops::Range<usize> {
    if editor.state.mode == EditorMode::Sketch {
        crate::library::editor::footprint::sketch_mode::active_bar::CONSTRAINT_SLOTS
    } else {
        0..0
    }
}

/// The widest the bar may draw in a canvas `canvas_width` wide.
pub fn bar_max_width(canvas_width: f32) -> f32 {
    canvas_width - 2.0 * signex_widgets::active_bar::Metrics::GRAPHITE.padding
}

/// Gap kept between a dropdown panel and the canvas's right edge.
const PANEL_EDGE_MARGIN: f32 = 4.0;

/// The horizontal run of the canvas between the docks, in window
/// coordinates: the bar fits and centres in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarSpan {
    pub x: f32,
    pub width: f32,
}

impl BarSpan {
    /// Window x of a centred bar `bar_width` wide.
    fn bar_left(self, bar_width: f32) -> f32 {
        self.x + ((self.width - bar_width) / 2.0).max(0.0)
    }

    /// Window x of a dropdown panel under a trigger at `trigger_x`, moved
    /// left when it would pass the canvas's right edge.
    fn panel_x(self, trigger_x: f32, panel_width: f32) -> f32 {
        let right = self.x + self.width;
        if trigger_x + panel_width + PANEL_EDGE_MARGIN > right {
            (right - panel_width - PANEL_EDGE_MARGIN).max(self.x)
        } else {
            trigger_x
        }
    }
}

/// The bar as drawn in a [`BarSpan`], in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedBar {
    pub left: f32,
    pub width: f32,
    /// Window x of the menu's trigger; `None` when the bar has none.
    pub trigger_x: Option<f32>,
}

/// Where the bar and `menu`'s trigger land in `span`, with the slots of
/// `run` scrolling when the bar does not fit.
pub fn place_bar(
    items: &[ActiveBarItem<LibraryMessage>],
    menu: FpActiveBarMenu,
    run: std::ops::Range<usize>,
    span: BarSpan,
) -> PlacedBar {
    let (offset, width) = menu_trigger_geometry_within(items, menu, run, bar_max_width(span.width));
    let left = span.bar_left(width);
    PlacedBar {
        left,
        width,
        trigger_x: offset.map(|offset| left + offset),
    }
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
///
/// Measured as drawn inside `max_width`, with the slots of `run` scrolling
/// when the bar does not fit.
fn menu_trigger_geometry_within(
    items: &[ActiveBarItem<LibraryMessage>],
    menu: FpActiveBarMenu,
    run: std::ops::Range<usize>,
    max_width: f32,
) -> (Option<f32>, f32) {
    let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
    let (offsets, total) =
        signex_widgets::active_bar::slot_offsets_within(items, run, max_width, &metrics);
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

/// The geometry of the whole bar, unconstrained by a window.
#[cfg(test)]
fn menu_trigger_geometry(
    items: &[ActiveBarItem<LibraryMessage>],
    menu: FpActiveBarMenu,
) -> (Option<f32>, f32) {
    menu_trigger_geometry_within(items, menu, 0..0, f32::INFINITY)
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
/// `span` is the canvas between the docks, where the bar is centred.
pub fn dropdown_overlay<'a>(
    editor: &'a FootprintDocument,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
    footprint_filter_presets: &[crate::active_bar::FootprintFilterPreset],
    top_padding_px: u16,
    span: BarSpan,
    i18n: &signex_i18n::I18n,
) -> Option<iced::Element<'a, LibraryMessage>> {
    use iced::Length;
    use iced::Widget as _;
    use iced::widget::{Space, container, mouse_area, stack};

    let menu = editor.state.active_bar_menu?;

    let entries = crate::library::editor::footprint::active_bar_dropdowns::entries(
        menu,
        &editor.state,
        editor.path.clone(),
        theme_id,
        footprint_filter_presets,
        i18n,
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

    // v0.26-H — the panel lands under its own trigger button, measured
    // from the items this same bar just built, and stays on the canvas
    // so a near-rightmost button still keeps it fully visible.
    let placed = place_bar(
        &bar_items(editor, theme_id, tokens, i18n),
        menu,
        scrolling_slots(editor),
        span,
    );
    // No trigger on the current bar means nothing to anchor to — a
    // stale menu key from a mode switch. Draw nothing rather than
    // park a panel over an unrelated button.
    let trigger_x = placed.trigger_x?;
    let abs_x = span.panel_x(trigger_x, width_hint.unwrap_or(220.0));

    let panel_anchor = signex_widgets::Translate::new(panel, (abs_x, top_padding_px as f32));

    let backstop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CloseActiveBarMenu),
    });

    Some(stack![backstop, panel_anchor].boxed())
}

/// The bar group a dropdown trigger sits in (mockup row 140).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerGroup {
    Selection,
    Placement,
    Sketch,
}

/// Build the 8 dropdown trigger buttons matching the schematic's
/// pattern: left-click fires the default action (or toggles the
/// menu when there's no obvious default — Filter / Snap), right-click
/// opens the dropdown. Chevron indicator advertises the right-click
/// secondary action. Each carries the group it is drawn in.
fn dropdown_trigger_items(
    editor: &FootprintDocument,
    tid: ThemeId,
) -> Vec<(TriggerGroup, ActiveBarItem<LibraryMessage>)> {
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

    use TriggerGroup::{Placement, Selection, Sketch};
    vec![
        (
            Selection,
            dual(
                "Selection Filter (left or right click for menu)",
                ActiveBarIcon::Svg(ic::icon_filter(tid)),
                FpActiveBarMenu::Filter,
                None,
            ),
        ),
        (
            Selection,
            dual(
                "Snap Options (left or right click for menu)",
                ActiveBarIcon::Svg(ic::icon_dd_align_grid(tid)),
                FpActiveBarMenu::Snap,
                None,
            ),
        ),
        (
            Selection,
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
        ),
        (
            Selection,
            dual(
                "Select (right-click for selection-mode menu)",
                ActiveBarIcon::Svg(ic::icon_select(tid)),
                FpActiveBarMenu::Select,
                Some(FootprintEditorMsg::SetPadsTool(
                    crate::library::editor::footprint::state::PadsTool::Select,
                )),
            ),
        ),
        (
            Selection,
            dual(
                "Align / Distribute (right-click for menu)",
                ActiveBarIcon::Svg(ic::icon_align(tid)),
                FpActiveBarMenu::Align,
                Some(FootprintEditorMsg::ActiveBarAlignSelectionToGrid),
            ),
        ),
        (
            Placement,
            dual(
                "3D Body (left or right click for menu)",
                ActiveBarIcon::Svg(ic::icon_dd_graphic(tid)),
                FpActiveBarMenu::Body3d,
                None,
            ),
        ),
        (
            Placement,
            dual(
                "Text (left-click places String, right-click for menu)",
                ActiveBarIcon::Svg(ic::icon_text(tid)),
                FpActiveBarMenu::Text,
                Some(FootprintEditorMsg::SetPadsTool(
                    crate::library::editor::footprint::state::PadsTool::PlaceString,
                )),
            ),
        ),
        (
            Sketch,
            dual(
                "Shapes (right-click for sketch-mode shape menu)",
                ActiveBarIcon::Svg(ic::icon_shapes(tid)),
                FpActiveBarMenu::Shapes,
                None,
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_widgets::active_bar::Metrics;

    fn editor_in(mode: crate::library::editor::footprint::state::EditorMode) -> FootprintDocument {
        use signex_library::{Footprint, FootprintFile};
        let file = FootprintFile::from_footprint(Footprint::empty("t"));
        let mut editor =
            crate::app::FootprintDocument::new(std::path::PathBuf::from("t.snxfpt"), file);
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
        use signex_types::theme::ThemeId;

        // The Graphite metrics the padding / button / gap pins describe.
        let tid = ThemeId::DEFAULT;
        let tokens = signex_types::theme::theme_tokens(tid);
        let m = Metrics::GRAPHITE;

        let i18n = signex_i18n::I18n::en_us();
        let sketch = editor_in(EditorMode::Sketch);
        let items = bar_items(&sketch, tid, &tokens, &i18n);
        let at = |m| menu_trigger_geometry(&items, m).0;

        // #2518: the sketch bar is Select | Create ▾ Modify ▾ | ... |
        // Snap ▾. Create sits after Select and the separator.
        let create = at(FpActiveBarMenu::SketchCreate).expect("Create trigger in Sketch mode");
        assert_eq!(create, m.padding + m.btn + m.sep_w + 2.0 * m.spacing);
        let modify = at(FpActiveBarMenu::SketchModify).expect("Modify trigger in Sketch mode");
        let snap = at(FpActiveBarMenu::Snap).expect("Snap trigger in Sketch mode");
        assert!(create < modify && modify < snap, "Snap ends the bar");
        // The pads-mode menus are not on the sketch bar.
        assert_eq!(at(FpActiveBarMenu::Filter), None);
        assert_eq!(at(FpActiveBarMenu::Shapes), None);

        // Pads mode has no sketch group triggers — and must say so
        // rather than hand back a stale offset.
        let pads = editor_in(EditorMode::Normal);
        let pads_items = bar_items(&pads, tid, &tokens, &i18n);
        assert_eq!(
            menu_trigger_geometry(&pads_items, FpActiveBarMenu::SketchCreate).0,
            None
        );
        // The first pads-mode trigger sits flush against the bar padding,
        // the next one button-plus-spacing further.
        assert_eq!(
            menu_trigger_geometry(&pads_items, FpActiveBarMenu::Filter).0,
            Some(m.padding)
        );
        assert_eq!(
            menu_trigger_geometry(&pads_items, FpActiveBarMenu::Snap).0,
            Some(m.padding + m.btn + m.spacing)
        );
    }

    /// #2518: a board sketch and a footprint in sketch mode show the same
    /// bar, slot for slot.
    #[test]
    fn both_hosts_show_the_same_sketch_bar() {
        use crate::library::editor::footprint::state::EditorMode;
        use signex_types::theme::ThemeId;

        let tid = ThemeId::DEFAULT;
        let tokens = signex_types::theme::theme_tokens(tid);
        let i18n = signex_i18n::I18n::en_us();
        let footprint = editor_in(EditorMode::Sketch);
        let mut board = editor_in(EditorMode::Sketch);
        let pcb = signex_types::project::SetupProfile::Standard
            .pcb(2)
            .new_board();
        board.state.board_backdrop = Some(std::sync::Arc::new(
            crate::library::editor::footprint::BoardBackdrop::from_board(&pcb),
        ));
        let shape = |items: &[ActiveBarItem<LibraryMessage>]| -> Vec<String> {
            items
                .iter()
                .map(|item| match item {
                    ActiveBarItem::Button(b) => b.tooltip.clone(),
                    ActiveBarItem::Separator => "|".into(),
                    ActiveBarItem::Custom { .. } => "custom".into(),
                })
                .collect()
        };
        let footprint_bar = shape(&bar_items(&footprint, tid, &tokens, &i18n));
        assert_eq!(
            footprint_bar,
            shape(&bar_items(&board, tid, &tokens, &i18n))
        );
        assert_eq!(
            footprint_bar,
            [
                "Select",
                "|",
                "Create: Line, Rectangle, Rounded Rectangle, Circle, Arc, Tangent Arc, Edge Arc, Point",
                "Modify: Fillet, Trim, Mirror, Offset, patterns, Make Pad from Profile",
                "|",
                "Fix point in place (needs 1 Point)",
                "Coincident (needs 2 Points)",
                "Distance between Points (needs 2 Points + dim input)",
                "Horizontal (needs 1 Line)",
                "Vertical (needs 1 Line)",
                "Parallel (needs 2 Lines)",
                "Perpendicular (needs 2 Lines)",
                "Equal length (needs 2 Lines)",
                "Point on line (needs 1 Point + 1 Line)",
                "Midpoint (needs 1 Point + 1 Line)",
                "Tangent (needs 1 Line + 1 Arc)",
                "Tangent (needs 2 Arcs)",
                "Angle between Lines (needs 2 Lines + dim input, degrees)",
                "Equal radius (needs 2 Circles/Arcs)",
                "Point on arc (needs 1 Point + 1 Arc)",
                "Distance Point↔Line (needs 1 Point + 1 Line + dim input)",
                "Distance Point↔Circle (needs 1 Point + 1 Circle/Arc + dim input)",
                "Symmetric about line (needs 2 Points + 1 Line in selection)",
                "Symmetric about point (needs 3 Points: 2 + a centre)",
                "|",
                "custom",
                "|",
                "custom",
                "|",
                "Snap options (left or right click for menu)",
            ]
        );
    }

    /// The measured width has to match what the bar actually draws, or
    /// the centre-aligned bar's left edge is wrong and every panel
    /// shifts by half the error. Guards the Custom slot in particular:
    /// its width is declared, not measured.
    #[test]
    fn bar_width_counts_every_slot_including_the_custom_one() {
        use crate::library::editor::footprint::sketch_mode::active_bar::{DIM_INPUT_W, LINETYPE_W};
        use crate::library::editor::footprint::state::EditorMode;
        use signex_types::theme::ThemeId;

        // The Graphite metrics the padding / button / gap pins describe.
        let tid = ThemeId::DEFAULT;
        let tokens = signex_types::theme::theme_tokens(tid);
        let m = Metrics::GRAPHITE;
        let editor = editor_in(EditorMode::Sketch);
        let items = bar_items(&editor, tid, &tokens, &signex_i18n::I18n::en_us());

        let customs = items
            .iter()
            .filter(|i| matches!(i, ActiveBarItem::Custom { .. }))
            .count();
        assert_eq!(
            customs, 2,
            "sketch bar should carry the dimension input and the linetype segments"
        );

        let expected: f32 = 2.0 * m.padding
            + items.iter().map(|i| i.width(&m)).sum::<f32>()
            + m.spacing * (items.len() - 1) as f32;
        let (_, measured) = menu_trigger_geometry(&items, FpActiveBarMenu::Snap);
        assert!(
            (measured - expected).abs() < 0.01,
            "{measured} vs {expected}"
        );
        // And the Custom slot is contributing its declared width, not a
        // button's — the bug that shifted every panel ~13 px right. Both
        // sides are constants, so this is a compile-time guard: were the
        // two widths ever made equal the assertion above would still pass
        // while measuring a button, and the test would stop proving
        // anything. Checked in a `const` block so that regression breaks
        // the build rather than waiting for a test run.
        const { assert!(DIM_INPUT_W > Metrics::GRAPHITE.btn) };
        const { assert!(LINETYPE_W > Metrics::GRAPHITE.btn) };
    }

    /// #2843: in an 800 px window the full sketch bar does not fit. The
    /// strip scrolls, so Snap ▾ past it stays inside the window, and its
    /// dropdown anchors to where it is drawn.
    #[test]
    fn a_narrow_window_keeps_the_triggers_past_the_strip_in_reach() {
        use crate::library::editor::footprint::state::EditorMode;
        use signex_types::theme::ThemeId;

        const NARROW: f32 = 800.0;
        let tid = ThemeId::DEFAULT;
        let tokens = signex_types::theme::theme_tokens(tid);
        let m = Metrics::GRAPHITE;
        let editor = editor_in(EditorMode::Sketch);
        let items = bar_items(&editor, tid, &tokens, &signex_i18n::I18n::en_us());
        let (_, full) = menu_trigger_geometry(&items, FpActiveBarMenu::Snap);
        assert!(full > NARROW, "the full bar is {full} px wide");

        let run = scrolling_slots(&editor);
        let max = bar_max_width(NARROW);
        let (snap, fitted) =
            menu_trigger_geometry_within(&items, FpActiveBarMenu::Snap, run.clone(), max);
        let snap = snap.expect("Snap trigger");
        assert!(fitted <= max, "{fitted} fits {max}");
        assert!(snap + m.btn <= fitted, "Snap at {snap} is inside {fitted}");
        let (create, _) =
            menu_trigger_geometry_within(&items, FpActiveBarMenu::SketchCreate, run, max);
        assert_eq!(
            create,
            menu_trigger_geometry(&items, FpActiveBarMenu::SketchCreate).0
        );
    }

    /// #2843: a dropdown stays on the canvas, not merely in the window.
    #[test]
    fn a_dropdown_panel_stays_between_the_docks() {
        let span = BarSpan {
            x: 245.0,
            width: 816.0,
        };
        let right = span.x + span.width;
        assert_eq!(span.panel_x(300.0, 260.0), 300.0, "room: under its trigger");
        assert_eq!(
            span.panel_x(right - 30.0, 260.0),
            right - 260.0 - PANEL_EDGE_MARGIN
        );
        assert_eq!(span.panel_x(300.0, 2000.0), span.x, "wider than the canvas");
    }

    /// #2843: a press on the lowest row of a strip button reaches the
    /// button; the rail sits below it, and the bar keeps its height.
    #[test]
    fn a_press_on_the_bottom_row_of_a_scrolling_button_reaches_it() {
        use signex_widgets::active_bar::{slot_offsets_within, view_within};

        const FIRST_IN_RUN: u8 = 10;
        const RUN: std::ops::Range<usize> = 2..12;
        fn bar() -> Vec<ActiveBarItem<u8>> {
            let button = |message| {
                ActiveBarItem::Button(ActiveBarButton {
                    on_press: Some(message),
                    ..ActiveBarButton::default()
                })
            };
            let mut items = vec![button(0), ActiveBarItem::Separator];
            items.extend((0..10).map(|i| button(FIRST_IN_RUN + i)));
            items.extend([ActiveBarItem::Separator, button(99)]);
            items
        }

        let tokens = signex_types::theme::theme_tokens(ThemeId::DEFAULT);
        let m = Metrics::GRAPHITE;
        let (_, full) = signex_widgets::active_bar::slot_offsets(&bar(), &m);
        let max = full - 100.0;
        let window = iced::Size::new(full, 2.0 * m.bar_height());
        let (offsets, _) = slot_offsets_within(&bar(), RUN, max, &m);
        let x = offsets[RUN.start] + m.btn / 2.0;
        let click = |y: f32| {
            crate::layout_probe::messages_on_click_at(
                view_within(bar(), RUN, max, &tokens),
                window,
                None,
                iced::Point::new(x, y),
            )
        };

        let button_bottom = m.padding + m.btn;
        assert_eq!(click(button_bottom - 0.5), [FIRST_IN_RUN]);
        assert!(
            click(button_bottom + m.padding / 2.0).is_empty(),
            "the rail"
        );

        let scrolling = view_within(bar(), RUN, max, &tokens);
        let fitting = view_within(bar(), RUN, full, &tokens);
        assert_eq!(
            crate::layout_probe::root_size(scrolling, window).height,
            crate::layout_probe::root_size(fitting, window).height,
        );
    }

    /// Only the sketch bar has a scrolling run.
    #[test]
    fn the_pads_bar_has_no_scrolling_run() {
        use crate::library::editor::footprint::state::EditorMode;
        assert!(scrolling_slots(&editor_in(EditorMode::Normal)).is_empty());
    }

    /// Outside Sketch mode each trigger sits in the group its tag names:
    /// selection, then placement after the first divider, then Shapes.
    #[test]
    fn each_trigger_sits_in_its_tagged_group() {
        use crate::library::editor::footprint::state::EditorMode;
        use signex_types::theme::ThemeId;

        let tid = ThemeId::DEFAULT;
        let tokens = signex_types::theme::theme_tokens(tid);
        let i18n = signex_i18n::I18n::en_us();
        let pads = editor_in(EditorMode::Normal);
        let items = bar_items(&pads, tid, &tokens, &i18n);
        let mut groups = vec![Vec::new()];
        for item in &items {
            match item {
                ActiveBarItem::Separator => groups.push(Vec::new()),
                ActiveBarItem::Button(b) => groups.last_mut().unwrap().push(b.tooltip.clone()),
                ActiveBarItem::Custom { .. } => {}
            }
        }
        let group_of = |prefix: &str| {
            groups
                .iter()
                .position(|g| g.iter().any(|t| t.starts_with(prefix)))
                .unwrap_or_else(|| panic!("{prefix} is on the bar: {groups:?}"))
        };
        for selection in [
            "Selection Filter",
            "Snap Options",
            "Move",
            "Select (",
            "Align",
        ] {
            assert_eq!(group_of(selection), 0, "{selection}");
        }
        assert_eq!(group_of("3D Body"), 1);
        assert_eq!(group_of("Text ("), 1);
        assert_eq!(group_of("Shapes"), 2);
        assert_eq!(groups.len(), 3);
    }
}
