//! The document tab menu's icon column (mockup row 5, #2990): every row has a
//! glyph, live or greyed, and every label starts at the same x.

use super::*;

use iced::Size;

use crate::app::{ContextMenuMsg, TabInfo, TabKind};
use crate::canvas::overlay_camera_tests::graphite_app_showing;
use crate::layout_probe::lay_out_texts;

/// The rows of the tab menu, without the separators.
const ROWS: [&str; 7] = [
    "Close power.snxsch",
    "Close All Other Documents",
    "Close Tabs to the Right",
    "Close All Documents",
    "Open In New Window",
    "Copy Path",
    "Reveal in Projects",
];

/// Every row carries a glyph whether it is live (the middle of three tabs) or
/// greyed (a sole tab greys Close All Other Documents and Close Tabs to the
/// Right).
#[test]
fn every_tab_menu_row_has_a_glyph_live_or_greyed() {
    let live = tab_menu(&tab_facts("power.snxsch", 1, 3));
    let sole = tab_menu(&tab_facts("power.snxsch", 0, 1));
    assert!(disabled_of(&sole, "Close All Other Documents"));
    assert!(disabled_of(&sole, "Close Tabs to the Right"));
    for rows in [&live, &sole] {
        assert_eq!(labels(rows), ROWS);
        for label in ROWS {
            assert!(item_of(rows, label).icon.is_some(), "{label} has a glyph");
        }
    }
}

/// The three bulk closes each have a glyph of their own, not the Close row's.
#[test]
fn the_bulk_close_rows_have_their_own_glyphs() {
    let rows = tab_menu(&tab_facts("power.snxsch", 1, 3));
    let glyph = |label: &str| item_of(&rows, label).icon.clone();
    let close = glyph("Close power.snxsch");
    let bulk = [
        glyph("Close All Other Documents"),
        glyph("Close Tabs to the Right"),
        glyph("Close All Documents"),
    ];
    for (i, one) in bulk.iter().enumerate() {
        assert_ne!(one, &close, "bulk row {i} reuses the Close glyph");
        for other in &bulk[i + 1..] {
            assert_ne!(one, other, "two bulk rows share a glyph");
        }
    }
}

/// The menu as the window shows it, opened through `update` on tab 0 of
/// `tabs` tabs: every label starts at the same x.
fn label_lefts(tabs: usize) -> Vec<(String, f32)> {
    let (mut app, _main) = graphite_app_showing(|_| {});
    for n in 1..tabs {
        app.document_state.tabs.push(TabInfo {
            title: format!("sheet-{n}"),
            path: std::path::PathBuf::from(format!("sheet-{n}.snxsch")),
            project_id: None,
            kind: TabKind::Schematic,
        });
    }
    app.interaction_state.last_mouse_pos = (300.0, 60.0);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowTab(0)));
    let ctx = app
        .interaction_state
        .tab_context_menu
        .clone()
        .expect("the tab menu opened");
    let texts = lay_out_texts(app.view_tab_context_menu(&ctx), Size::new(1400.0, 900.0));
    let title = format!("Close {}", app.document_state.tabs[0].title);
    let rows: Vec<String> = std::iter::once(title)
        .chain(ROWS[1..].iter().map(|row| (*row).to_string()))
        .collect();
    let lefts: Vec<(String, f32)> = texts
        .into_iter()
        .filter(|text| rows.contains(&text.text))
        .map(|text| (text.text, text.bounds.x))
        .collect();
    assert_eq!(lefts.len(), rows.len(), "every row laid out: {lefts:?}");
    lefts
}

#[test]
fn every_tab_menu_label_starts_at_the_same_x() {
    for tabs in [1, 3] {
        let lefts = label_lefts(tabs);
        let first = lefts[0].1;
        for (label, x) in &lefts {
            assert!(
                (x - first).abs() < 0.5,
                "{tabs} tabs: {label} starts at {x}, the Close row at {first}"
            );
        }
    }
}
