//! The table's views build and lay out in Graphite Light and Dark, show
//! what the model says, and a part row keeps its gestures (#2631).

use iced::Size;
use signex_i18n::I18n;
use signex_library::templates::ColumnKey;
use signex_types::theme::{ThemeId, theme_tokens};

use super::super::tests::thumb_look;
use super::test_fixture::{key_of, parts, show};
use super::{table_block, toolbar};
use crate::layout_probe::{find, lay_out_texts, messages_for_events};
use crate::library::messages::{ComponentsTableMsg as M, LibraryMessage, RangeFilterMsg};
use crate::library::state::LibraryState;

const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];
const DOCK: Size = Size::new(900.0, 700.0);

fn texts_of(state: &LibraryState, theme: ThemeId) -> Vec<String> {
    let tokens = theme_tokens(theme);
    let i18n = I18n::en_us();
    let mut out: Vec<String> =
        lay_out_texts(toolbar(&state.components_panel, &tokens, &i18n), DOCK)
            .into_iter()
            .map(|t| t.text)
            .collect();
    out.extend(
        lay_out_texts(table_block(state, thumb_look(), &tokens, &i18n), DOCK)
            .into_iter()
            .map(|t| t.text),
    );
    // Catalog arguments come wrapped in isolation marks.
    out.into_iter()
        .map(|t| t.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn apply(state: &mut LibraryState, msg: M) {
    let _ = state.components_panel.table.apply(msg);
    state.sync_view_caches();
}

/// The grouped capacitor view: the tool line, the group bar's chips, the
/// headers with the sort mark, a group line per package with "Not set"
/// last, and the status line, in both themes.
#[test]
fn the_grouped_view_builds_in_both_graphite_themes() {
    let mut state = parts();
    show(&mut state, "capacitor");
    for theme in THEMES {
        let texts = texts_of(&state, theme);
        let has = |s: &str| texts.iter().any(|t| t == s);
        for expected in [
            "Filter",
            "Columns",
            "+ Group",
            "Expand all",
            "Package",
            "Voltage_max",
            "↑",
            "0402",
            "0603",
            "Not set",
            "Results 5",
            "Sort: Voltage_max ↑",
        ] {
            assert!(has(expected), "{theme:?}: no {expected:?} in {texts:#?}");
        }
    }
}

/// Every popover builds in both themes: Columns, Filter with a min-to-max
/// range row, a header menu and the "+ Group" menu.
#[test]
fn every_popover_builds_in_both_graphite_themes() {
    let mut state = parts();
    show(&mut state, "opamp");
    apply(
        &mut state,
        M::RangeFilter(RangeFilterMsg::ColumnPicked {
            key: "supply".into(),
            unit: signex_types::quantity::Unit::Volt,
        }),
    );
    for (open, expected) in [
        (M::ColumnsToggled, "Reset to category default"),
        (M::FilterToggled, "In stock only"),
        (
            M::HeaderRightClicked(ColumnKey::param("supply")),
            "Group by this",
        ),
        (M::GroupMenuToggled, "Group by"),
    ] {
        apply(&mut state, M::PopoverDismissed);
        apply(&mut state, open);
        for theme in THEMES {
            let texts = texts_of(&state, theme);
            assert!(
                texts.iter().any(|t| t == expected),
                "{theme:?}: no {expected:?} in {texts:#?}"
            );
        }
    }
    apply(&mut state, M::ColumnsToggled);
    let texts = texts_of(&state, ThemeId::GraphiteDark);
    assert!(texts.iter().any(|t| t == "Using category default"));
}

/// A filter that leaves nothing says so and offers Clear filters and
/// Search all parts.
#[test]
fn an_empty_table_offers_its_way_back() {
    let mut state = parts();
    show(&mut state, "capacitor");
    apply(&mut state, M::InStockToggled(true));
    let texts = texts_of(&state, ThemeId::GraphiteLight);
    for expected in ["Clear filters", "Search all parts", "Filter 1"] {
        assert!(texts.iter().any(|t| t == expected), "no {expected:?}");
    }
}

/// A press on a part's cell selects it, a right press opens its menu and a
/// double-click places it (mockup row 52); each publishes only that.
#[test]
fn a_part_row_publishes_press_right_click_and_double_click_place() {
    use iced::mouse::{Button, Event};
    let state = parts();
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let i18n = I18n::en_us();
    let build = || table_block(&state, thumb_look(), &tokens, &i18n);
    let texts = lay_out_texts(build(), DOCK);
    let at = find(&texts, "C1").bounds.center();
    let events = |buttons: &[(Button, bool)]| -> Vec<LibraryMessage> {
        let events: Vec<iced::Event> = buttons
            .iter()
            .map(|(b, down)| {
                iced::Event::Mouse(if *down {
                    Event::ButtonPressed(*b)
                } else {
                    Event::ButtonReleased(*b)
                })
            })
            .collect();
        let mut published = messages_for_events(build(), DOCK, at, &events);
        published.retain(|m| {
            !matches!(
                m,
                LibraryMessage::ComponentsPanelRowEntered(_)
                    | LibraryMessage::ComponentsPanelRowExited(_)
                    | LibraryMessage::ComponentsTable(M::Scrolled { .. })
            )
        });
        published
    };
    let c1 = key_of(&state, "C1");
    let pressed = events(&[(Button::Left, true), (Button::Left, false)]);
    assert!(
        matches!(pressed.as_slice(), [LibraryMessage::ComponentsPanelRowPressed(k)] if *k == c1),
        "{pressed:?}"
    );
    let right = events(&[(Button::Right, true), (Button::Right, false)]);
    assert!(
        matches!(right.as_slice(), [LibraryMessage::ComponentsPanelRowRightClicked(k)] if *k == c1),
        "{right:?}"
    );
    let double = events(&[
        (Button::Left, true),
        (Button::Left, false),
        (Button::Left, true),
        (Button::Left, false),
    ]);
    assert!(
        double.iter().any(|m| matches!(
            m,
            LibraryMessage::ComponentsPanelPlace { row_id, .. } if *row_id == c1.row_id
        )),
        "{double:?}"
    );
}

/// A part's cells: a value that is not set draws "—", a min-to-max pair
/// "3 to 3.6 V", a half-set pair "≥ 2.7 V".
#[test]
fn cells_show_not_set_ranges_and_half_set_pairs() {
    let mut state = parts();
    show(&mut state, "opamp");
    let texts = texts_of(&state, ThemeId::GraphiteLight);
    for expected in ["3 to 3.6 V", "4.5 to 5.5 V", "≥ 2.7 V", "\u{2014}"] {
        assert!(
            texts.iter().any(|t| t == expected),
            "no {expected:?} in {texts:#?}"
        );
    }
}

/// The selected part's row is the selected one; its key is the one a
/// press published.
#[test]
fn the_selected_part_keeps_its_key_across_a_sort() {
    let mut state = parts();
    let key = key_of(&state, "U2");
    state.components_panel.selected = Some(key.clone());
    apply(&mut state, M::HeaderPressed(ColumnKey::PartNumber));
    let table = &state.components_panel.table;
    assert!(table.cache.base.iter().any(|r| r.key == key));
    assert_eq!(table.cache.details.key.as_ref(), Some(&key));
}
