//! Active Bar geometry tests: the slot table, the hit rects and the
//! dropdown anchors against what the widget draws, for Graphite (30 px
//! buttons, 3 px padding, 1 px gap).

use super::layout::{BarSlot, SCHEMATIC_BAR_SLOTS, bar_slots, slot_rects};
use super::*;
use crate::app::{DrawMode, Tool};
use signex_widgets::active_bar::{ActiveBarItem, Metrics, slot_offsets_with};

use signex_types::theme::ThemeTokens;

/// Both Graphite themes.
fn themes() -> [(ThemeTokens, ThemeId); 2] {
    let graphite = |tid| (signex_types::theme::theme_tokens(tid), tid);
    [
        graphite(ThemeId::GraphiteLight),
        graphite(ThemeId::GraphiteDark),
    ]
}

fn drawn_items(tool: Tool, tokens: &ThemeTokens, tid: ThemeId) -> Vec<ActiveBarItem<ActiveBarMsg>> {
    bar_items(
        tool,
        DrawMode::Ortho90,
        &std::collections::HashMap::new(),
        tokens,
        tid,
        false,
        false,
    )
}

fn metrics(tid: ThemeId) -> Metrics {
    Metrics::of(&signex_types::theme::theme_tokens(tid))
}

/// The view's item list and the slot table must describe the same bar.
#[test]
fn bar_items_follow_the_slot_table() {
    for (tokens, tid) in themes() {
        let slots: Vec<BarSlot> = drawn_items(Tool::Select, &tokens, tid)
            .iter()
            .map(|item| match item {
                ActiveBarItem::Separator => BarSlot::Separator,
                ActiveBarItem::Button(b) => match b.on_right_press {
                    Some(ActiveBarMsg::ToggleMenu(menu)) => BarSlot::Trigger(menu),
                    _ => panic!("bar button without a dropdown trigger"),
                },
                ActiveBarItem::Custom { .. } => panic!("unexpected custom slot"),
            })
            .collect();
        assert_eq!(slots, SCHEMATIC_BAR_SLOTS, "{tid:?}");
    }
}

/// Hit rects are the rects the renderer lays out (same row maths),
/// also while the draw-mode pill widens the bar.
#[test]
fn hit_rects_equal_drawn_rects() {
    for (tokens, tid) in themes() {
        let m = Metrics::of(&tokens);
        for tool in [Tool::Select, Tool::Wire, Tool::Bus] {
            let items = drawn_items(tool, &tokens, tid);
            let (drawn, total) = slot_offsets_with(&items, &m);
            assert_eq!(total, bar_width_for(tool, &m), "{tid:?} {tool:?} width");
            let rects = slot_rects(tool, &m);
            assert_eq!(rects.len(), items.len(), "{tool:?} slot count");
            for (i, (left, width)) in rects.iter().copied().enumerate() {
                assert_eq!(left, drawn[i], "slot {i} left edge");
                assert_eq!(width, items[i].width_with(&m), "slot {i} width");
                let BarSlot::Trigger(menu) = bar_slots(tool)[i] else {
                    assert_eq!(
                        menu_at(tool, left + width / 2.0, &m),
                        None,
                        "non-trigger {i}"
                    );
                    continue;
                };
                assert_eq!(menu_at(tool, left, &m), Some(menu));
                assert_eq!(menu_at(tool, left + width - 0.01, &m), Some(menu));
                assert_ne!(menu_at(tool, left + width, &m), Some(menu));
                assert_ne!(menu_at(tool, left - 0.01, &m), Some(menu));
                assert_eq!(dropdown_x_offset(menu, &m), left);
            }
        }
    }
}

/// Graphite metrics are the mock's 30 px buttons, 3 px padding, 1 px gap.
#[test]
fn graphite_metrics_are_thirty_three_one() {
    let m = metrics(ThemeId::GraphiteLight);
    assert_eq!((m.btn, m.padding, m.spacing), (30.0, 3.0, 1.0));
    assert_eq!(metrics(ThemeId::GraphiteDark), m);
}

/// #1509: with the pill shown the dropdown anchor (window-space left
/// edge of the trigger) lands on the drawn button, not 23.5 px off.
#[test]
fn dropdown_anchor_matches_drawn_button_with_pill() {
    for (tokens, tid) in themes() {
        let m = Metrics::of(&tokens);
        for window_width in [1400.0_f32, 1117.0] {
            for (menu, centre) in drawn_trigger_centres_with(Tool::Wire, window_width, &tokens, tid)
            {
                let anchor =
                    dropdown_x_offset(menu, &m) + bar_left_in_window(Tool::Wire, window_width, &m);
                assert_eq!(
                    anchor + m.btn / 2.0,
                    centre,
                    "{tid:?} {menu:?} at {window_width}"
                );
            }
        }
    }
}

/// The right-click zone is the Graphite bar's 10 px top margin, its 36 px
/// height and the slack below it.
#[test]
fn graphite_active_bar_zone_height_is_unchanged() {
    let m = Metrics::GRAPHITE;
    assert_eq!(
        bar_zone_height_for(&m),
        10.0 + 36.0 + layout::BAR_ZONE_SLACK_PX
    );
    assert_eq!(bar_zone_height_for(&m), 58.0);
}

/// Graphite floats the bar 10 px down.
#[test]
fn graphite_bar_top_margin_is_ten() {
    let graphite = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
    assert_eq!(layout::bar_top_margin(&graphite), 10.0);
}
