//! The picker's search row and footer hint (mockup row 129).

use super::*;
use crate::layout_probe::{find, lay_out_inputs, lay_out_texts, messages_on_click};
use signex_types::theme::ThemeId;

const WINDOW: iced::Size = iced::Size::new(1200.0, 800.0);

fn picker_view<'a>(
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    view(
        picker,
        tokens,
        I18n::en_us_ref(),
        false,
        PreviewLook::for_tests(),
    )
}

/// "Add range filter…" sits beside the search on one row even when no
/// listed part has a measurement column; there it is disabled and a click
/// does nothing.
#[test]
fn add_range_filter_shares_the_search_row_without_a_measurement_column() {
    let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
    let picker = PickerState::default();
    assert!(picker.rows.filter_columns().is_empty());

    let texts = lay_out_texts(picker_view(&picker, &tokens), WINDOW);
    let filter = find(&texts, "Add range filter…").bounds;
    let inputs = lay_out_inputs(picker_view(&picker, &tokens), WINDOW);
    let search = find(&inputs, "Search internal PN / MPN / description…").bounds;
    assert!(
        filter.x >= search.x + search.width,
        "filter {filter:?} search {search:?}"
    );
    let overlap = filter.y < search.y + search.height && search.y < filter.y + filter.height;
    assert!(overlap, "filter {filter:?} search {search:?}");

    let clicked = messages_on_click(picker_view(&picker, &tokens), WINDOW, "Add range filter…");
    assert!(clicked.is_empty(), "{clicked:?}");
}

/// The footer names the arrows as well as Enter, in both list stages.
#[test]
fn the_footer_hint_names_the_arrows_and_enter() {
    let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
    let picker = PickerState::default();
    let texts = lay_out_texts(picker_view(&picker, &tokens), WINDOW);
    assert!(
        texts
            .iter()
            .any(|t| t.text == "↑↓ to move · Enter to place")
    );
    assert_eq!(
        I18n::en_us_ref().message_plain(ids::LIBRARY_PICKER_HINT_NEXT, &[]),
        "↑↓ to move · Enter for Next"
    );
}

/// The list offset puts the first row at the top, the last at the bottom
/// and a middle row in proportion; a short list does not scroll.
#[test]
fn the_list_offset_follows_the_rows_place() {
    let y = |index, count| list_offset_showing(index, count).y;
    for (index, count, want) in [
        (0, 0, 0.0),
        (0, 1, 0.0),
        (0, 5, 0.0),
        (2, 5, 0.5),
        (4, 5, 1.0),
        (9, 5, 1.0),
    ] {
        assert!(
            (y(index, count) - want).abs() < 1e-6,
            "row {index} of {count}"
        );
    }
}
