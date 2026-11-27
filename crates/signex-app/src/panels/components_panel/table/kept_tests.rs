//! The old list's pinned behaviour, kept in the table (Caner, 2026-11-27):
//! Recently placed (now a category), the Previews switch (now the
//! thumbnail column and taller rows), the thumbnails it asks for, and the
//! row's press / hover Place / right-click split (#2535, #2631).

use iced::Size;
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, theme_tokens};

use super::super::look;
use super::super::recent::MAX_RECENT;
use super::super::tests::thumb_look;
use super::state::CategoryKey;
use super::table_block;
use super::test_fixture::{drawn_parts, key_of, parts, show};
use crate::layout_probe::{find, lay_out_texts, messages_for_events};
use crate::library::messages::{ComponentsTableMsg as M, LibraryMessage};
use crate::library::state::{ComponentsPanelRowKey, ComponentsRowStyle, LibraryState};

const DOCK: Size = Size::new(900.0, 700.0);

fn texts(state: &LibraryState) -> Vec<crate::layout_probe::LaidOutText> {
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    lay_out_texts(
        table_block(state, thumb_look(), &tokens, &I18n::en_us()),
        DOCK,
    )
}

/// Recently placed lists the parts placed this session, most recent
/// first, each once, at most `MAX_RECENT` (the log's own order and cap);
/// the picker counts them.
#[test]
fn recently_placed_lists_the_log_most_recent_first_and_capped() {
    let mut state = parts();
    for pn in ["C3", "C1", "U2", "C3"] {
        let key = key_of(&state, pn);
        state.components_panel.recent.record(key);
    }
    show(&mut state, "@recent");
    assert_eq!(drawn_parts(&state), ["C3", "U2", "C1"]);
    assert!(
        state
            .components_panel
            .table
            .cache
            .categories
            .iter()
            .any(|e| e.key == CategoryKey::Recent && e.label.contains("Recently placed")),
    );
    for pn in ["C1", "C2", "C4", "C5", "R1", "U1"] {
        let key = key_of(&state, pn);
        state.components_panel.recent.record(key);
    }
    state.sync_view_caches();
    let drawn = drawn_parts(&state);
    assert_eq!(drawn.len(), MAX_RECENT);
    assert_eq!(drawn[0], "U1");
}

/// A recent part whose row is gone (library closed, row deleted) is not
/// listed.
#[test]
fn a_recent_part_whose_row_is_gone_is_not_listed() {
    let mut state = parts();
    state.components_panel.recent.record(ComponentsPanelRowKey {
        library_path: "gone.snxlib".into(),
        table: "t".into(),
        row_id: signex_library::RowId(uuid::Uuid::from_u128(7)),
    });
    show(&mut state, "@recent");
    assert!(drawn_parts(&state).is_empty());
}

/// A header click in Recently placed sorts it like any category; turning
/// the sort off gives the log's order back.
#[test]
fn recently_placed_keeps_the_log_order_until_sorted() {
    let mut state = parts();
    for pn in ["C1", "U2"] {
        let key = key_of(&state, pn);
        state.components_panel.recent.record(key);
    }
    show(&mut state, "@recent");
    assert_eq!(drawn_parts(&state), ["U2", "C1"]);
    let pn = signex_library::templates::ColumnKey::PartNumber;
    let _ = state
        .components_panel
        .table
        .apply(M::HeaderPressed(pn.clone()));
    state.sync_view_caches();
    assert_eq!(drawn_parts(&state), ["C1", "U2"]);
    let _ = state
        .components_panel
        .table
        .apply(M::HeaderPressed(pn.clone()));
    let _ = state.components_panel.table.apply(M::HeaderPressed(pn));
    state.sync_view_caches();
    assert_eq!(drawn_parts(&state), ["U2", "C1"]);
}

/// Compact is the default; Previews rows are taller and start with the
/// thumbnail column, so the part number moves right by its width.
#[test]
fn compact_rows_are_the_default_and_previews_rows_are_taller() {
    assert_eq!(ComponentsRowStyle::default(), ComponentsRowStyle::Compact);
    assert!(!ComponentsRowStyle::default().shows_previews());
    assert_eq!(
        ComponentsRowStyle::from_previews(true),
        ComponentsRowStyle::Previews
    );
    const { assert!(look::PREVIEW_ROW_HEIGHT > look::ROW_HEIGHT) };

    let mut state = parts();
    let compact = texts(&state);
    let (a, b) = (find(&compact, "C1").bounds, find(&compact, "C2").bounds);
    assert!((b.y - a.y - look::ROW_HEIGHT).abs() < 0.5, "{}", b.y - a.y);

    state.components_panel.row_style = ComponentsRowStyle::Previews;
    let previews = texts(&state);
    let (c, d) = (find(&previews, "C1").bounds, find(&previews, "C2").bounds);
    assert!(
        (d.y - c.y - look::PREVIEW_ROW_HEIGHT).abs() < 0.5,
        "{}",
        d.y - c.y
    );
    assert!(
        c.x >= a.x + look::THUMB_WIDTH - 0.5,
        "the thumbnail column comes first"
    );
}

/// The thumbnail read is asked for the rows in view only with Previews on;
/// the dragged row's card asks for its own either way.
#[test]
fn thumbnails_are_asked_for_the_drawn_rows_only_with_previews() {
    let mut state = parts();
    assert!(crate::panels::components_panel::wanted_thumbnails(&state).is_empty());
    state.components_panel.row_style = ComponentsRowStyle::Previews;
    let wanted = crate::panels::components_panel::wanted_thumbnails(&state);
    assert_eq!(wanted.len(), 9, "every row of All parts is in view");
    let c1 = state.open_libraries[0].tables["t"]
        .iter()
        .find(|r| r.internal_pn.as_str() == "C1")
        .map(|r| r.symbol_ref)
        .unwrap();
    assert!(wanted.contains(&c1));
}

/// Only the hovered row offers the inline Place.
#[test]
fn a_hovered_row_offers_place() {
    let mut state = parts();
    assert!(texts(&state).iter().all(|t| t.text != "Place"));
    state.components_panel.hovered = Some(key_of(&state, "C2"));
    let hovered = texts(&state);
    assert_eq!(hovered.iter().filter(|t| t.text == "Place").count(), 1);
    let place = find(&hovered, "Place").bounds;
    let c2 = find(&hovered, "C2").bounds;
    assert!(
        (place.center_y() - c2.center_y()).abs() < 1.0,
        "in C2's row"
    );
}

/// The messages a mouse `button` press (and release) on the text `label`
/// of the hovered row C1 publishes, hover and scroll left out.
fn row_press(button: iced::mouse::Button, label: &str) -> Vec<LibraryMessage> {
    use iced::mouse::Event;
    let mut state = parts();
    state.components_panel.hovered = Some(key_of(&state, "C1"));
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let build = || table_block(&state, thumb_look(), &tokens, &i18n);
    let at = find(&lay_out_texts(build(), DOCK), label).bounds.center();
    let mut published = messages_for_events(
        build(),
        DOCK,
        at,
        &[Event::ButtonPressed(button), Event::ButtonReleased(button)].map(iced::Event::Mouse),
    );
    published.retain(|m| {
        !matches!(
            m,
            LibraryMessage::ComponentsPanelRowEntered(_)
                | LibraryMessage::ComponentsPanelRowExited(_)
                | LibraryMessage::ComponentsTable(M::Scrolled { .. })
        )
    });
    published
}

/// A press on a row's label selects it, a press on its hover Place places
/// it, a right press opens its menu: each publishes that one message only
/// (#2631).
#[test]
fn a_row_publishes_press_place_and_right_click_apart() {
    use iced::mouse::Button;
    let pressed = row_press(Button::Left, "C1");
    assert!(
        matches!(
            pressed.as_slice(),
            [LibraryMessage::ComponentsPanelRowPressed(key)] if key.table == "t"
        ),
        "{pressed:?}"
    );
    let placed = row_press(Button::Left, "Place");
    assert!(
        matches!(
            placed.as_slice(),
            [LibraryMessage::ComponentsPanelPlace { table, .. }] if table == "t"
        ),
        "{placed:?}"
    );
    let right = row_press(Button::Right, "C1");
    assert!(
        matches!(
            right.as_slice(),
            [LibraryMessage::ComponentsPanelRowRightClicked(_)]
        ),
        "{right:?}"
    );
}
