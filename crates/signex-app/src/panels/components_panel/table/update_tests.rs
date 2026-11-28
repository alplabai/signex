//! State-machine tests of the Components panel table: each interaction
//! from the message to the rows the table draws (mockup row 52).

use signex_library::RangeOn;
use signex_library::templates::{ColumnKey, SortKey};
use signex_types::quantity::{SiPrefix, Unit};

use super::TableLine;
use super::state::{CategoryKey, TablePopover};
use super::test_fixture::{drawn_parts, key_of, parts, show};
use super::update::TableEffects;
use crate::library::messages::{ComponentsTableMsg as M, HeaderAction, RangeBound, RangeFilterMsg};
use crate::library::state::LibraryState;

fn apply(state: &mut LibraryState, msg: M) -> TableEffects {
    let fx = state.components_panel.table.apply(msg);
    state.sync_view_caches();
    fx
}

fn p(name: &str) -> ColumnKey {
    ColumnKey::param(name)
}

fn group_lines(state: &LibraryState) -> Vec<(usize, usize, bool)> {
    state
        .components_panel
        .table
        .cache
        .lines
        .iter()
        .filter_map(|l| match l {
            TableLine::Group {
                depth, count, open, ..
            } => Some((*depth, *count, *open)),
            TableLine::Row { .. } => None,
        })
        .collect()
}

/// Decision 3: a category opens with its class template's view: its
/// columns, its sort and its grouping, every group closed.
#[test]
fn a_category_opens_with_its_template_view() {
    let mut state = parts();
    show(&mut state, "capacitor");
    let setup = &state.components_panel.table.cache.setup;
    assert_eq!(
        setup.visible_columns(),
        [
            ColumnKey::PartNumber,
            p("value"),
            p("voltage_max"),
            p("dielectric"),
            p("tolerance"),
            p("package"),
            ColumnKey::Stock,
            ColumnKey::Price,
        ]
    );
    assert_eq!(setup.sort, [SortKey::ascending(p("voltage_max"))]);
    assert_eq!(
        setup.group,
        [
            SortKey::ascending(p("package")),
            SortKey::ascending(p("value"))
        ]
    );
    // 0402, 0603, then the part without a package ("Not set", last).
    assert_eq!(
        group_lines(&state),
        [(0, 2, false), (0, 2, false), (0, 1, false)]
    );
    assert!(drawn_parts(&state).is_empty(), "every group starts closed");
    assert!(
        state.components_panel.table.setups.is_empty(),
        "nothing saved yet"
    );
}

/// All parts lists every part by part number, ungrouped.
#[test]
fn all_parts_lists_every_part_by_part_number() {
    let mut state = parts();
    show(&mut state, "*");
    assert_eq!(
        drawn_parts(&state),
        ["C1", "C2", "C3", "C4", "C5", "R1", "U1", "U2", "U3"]
    );
}

/// A header click sorts by that column alone: ascending, then
/// descending, then off. With Shift it adds a nested key. Each change is
/// saved for the category (decision 1).
#[test]
fn header_clicks_sort_and_shift_nests() {
    let mut state = parts();
    let fx = apply(&mut state, M::HeaderPressed(ColumnKey::Manufacturer));
    assert!(fx.save_prefs);
    let sort = |s: &LibraryState| s.components_panel.table.cache.setup.sort.clone();
    assert_eq!(sort(&state), [SortKey::ascending(ColumnKey::Manufacturer)]);
    apply(&mut state, M::HeaderPressed(ColumnKey::Manufacturer));
    assert_eq!(sort(&state), [SortKey::descending(ColumnKey::Manufacturer)]);
    state.components_panel.table.shift_held = true;
    apply(&mut state, M::HeaderPressed(ColumnKey::PartNumber));
    assert_eq!(
        sort(&state),
        [
            SortKey::descending(ColumnKey::Manufacturer),
            SortKey::ascending(ColumnKey::PartNumber)
        ]
    );
    state.components_panel.table.shift_held = false;
    apply(&mut state, M::HeaderPressed(ColumnKey::PartNumber));
    apply(&mut state, M::HeaderPressed(ColumnKey::PartNumber));
    apply(&mut state, M::HeaderPressed(ColumnKey::PartNumber));
    assert!(
        sort(&state).is_empty(),
        "the third click turns the sort off"
    );
    let saved = &state.components_panel.table.setups["*"];
    assert!(saved.sort.is_empty());
}

/// Sorting descending by part number reverses the rows; "not set" values
/// stay last either way.
#[test]
fn a_sort_orders_the_rows_and_not_set_stays_last() {
    let mut state = parts();
    show(&mut state, "capacitor");
    apply(&mut state, M::HeaderRightClicked(p("package")));
    apply(&mut state, M::HeaderMenuPicked(HeaderAction::ToggleGroup));
    apply(&mut state, M::HeaderRightClicked(p("value")));
    apply(&mut state, M::HeaderMenuPicked(HeaderAction::ToggleGroup));
    assert!(state.components_panel.table.cache.setup.group.is_empty());
    apply(&mut state, M::HeaderRightClicked(p("voltage_max")));
    apply(
        &mut state,
        M::HeaderMenuPicked(HeaderAction::SortDescending),
    );
    // C5 has no voltage: last in both directions.
    assert_eq!(drawn_parts(&state), ["C2", "C4", "C3", "C1", "C5"]);
    apply(&mut state, M::HeaderRightClicked(p("voltage_max")));
    apply(&mut state, M::HeaderMenuPicked(HeaderAction::SortAscending));
    assert_eq!(drawn_parts(&state), ["C1", "C3", "C2", "C4", "C5"]);
}

/// The header menu opens on a right-click and closes on a pick; Hide and
/// Columns… act on its column.
#[test]
fn the_header_menu_acts_on_its_column_and_closes() {
    let mut state = parts();
    show(&mut state, "capacitor");
    apply(&mut state, M::HeaderRightClicked(p("dielectric")));
    assert_eq!(
        state.components_panel.table.popover,
        Some(TablePopover::HeaderMenu(p("dielectric")))
    );
    apply(&mut state, M::HeaderMenuPicked(HeaderAction::Hide));
    assert_eq!(state.components_panel.table.popover, None);
    assert!(
        !state
            .components_panel
            .table
            .cache
            .setup
            .visible_columns()
            .contains(&p("dielectric"))
    );
    apply(&mut state, M::HeaderRightClicked(p("value")));
    apply(&mut state, M::HeaderMenuPicked(HeaderAction::OpenColumns));
    assert_eq!(
        state.components_panel.table.popover,
        Some(TablePopover::Columns)
    );
}

/// A drag on a header's handle resizes the column by the pointer's travel,
/// clamped, and the width is saved on release. A release lost outside the
/// table ends the resize at the next header press, which does not sort.
#[test]
fn a_handle_drag_resizes_and_saves_the_width() {
    let mut state = parts();
    show(&mut state, "capacitor");
    let start = state.components_panel.table.width_of(&p("value"));
    apply(&mut state, M::ResizeStarted(p("value")));
    apply(&mut state, M::ResizeMoved(200.0));
    apply(&mut state, M::ResizeMoved(230.0));
    assert_eq!(
        state.components_panel.table.width_of(&p("value")),
        start + 30.0
    );
    let fx = apply(&mut state, M::ResizeEnded);
    assert!(fx.save_prefs);
    assert_eq!(
        state
            .components_panel
            .table
            .cache
            .setup
            .widths
            .get(&p("value")),
        Some(&(start + 30.0))
    );

    apply(&mut state, M::ResizeStarted(p("package")));
    apply(&mut state, M::ResizeMoved(0.0));
    apply(&mut state, M::ResizeMoved(-1000.0));
    let sort_before = state.components_panel.table.cache.setup.sort.clone();
    apply(&mut state, M::HeaderPressed(p("package")));
    let table = &state.components_panel.table;
    assert!(table.resize.is_none());
    assert_eq!(
        table.cache.setup.sort, sort_before,
        "no sort from that press"
    );
    assert_eq!(
        table.cache.setup.widths.get(&p("package")),
        Some(&super::state::MIN_COLUMN_WIDTH)
    );
}

/// A group line opens and closes; Expand all opens every level and
/// Collapse all closes them again.
#[test]
fn groups_open_close_and_expand_all() {
    let mut state = parts();
    show(&mut state, "capacitor");
    let first = match &state.components_panel.table.cache.lines[0] {
        TableLine::Group { path, .. } => path.clone(),
        TableLine::Row { .. } => panic!("a group first"),
    };
    apply(&mut state, M::GroupToggled(first.clone()));
    // 0402 opens on its one value group (100 nF, both parts).
    assert_eq!(
        group_lines(&state),
        [(0, 2, true), (1, 2, false), (0, 2, false), (0, 1, false)]
    );
    apply(&mut state, M::ExpandAllPressed);
    assert_eq!(drawn_parts(&state), ["C1", "C2", "C4", "C3", "C5"]);
    let fx = apply(&mut state, M::CollapseAllPressed);
    assert!(fx.body_to_top);
    assert!(drawn_parts(&state).is_empty());
    apply(&mut state, M::GroupToggled(first.clone()));
    apply(&mut state, M::GroupToggled(first));
    assert!(group_lines(&state).iter().all(|(_, _, open)| !open));
}

/// "+ Group" adds an innermost level; a chip's arrow flips its direction
/// and its ✕ removes it.
#[test]
fn the_group_bar_adds_flips_and_removes_levels() {
    let mut state = parts();
    show(&mut state, "capacitor");
    apply(&mut state, M::GroupMenuToggled);
    assert_eq!(
        state.components_panel.table.popover,
        Some(TablePopover::GroupMenu)
    );
    apply(&mut state, M::GroupAdded(p("voltage_max")));
    let group = |s: &LibraryState| s.components_panel.table.cache.setup.group.clone();
    assert_eq!(group(&state).len(), 3);
    assert_eq!(group(&state)[2], SortKey::ascending(p("voltage_max")));
    assert_eq!(state.components_panel.table.popover, None);
    apply(&mut state, M::GroupFlipped(0));
    assert!(group(&state)[0].descending);
    // Descending by package: 0603 before 0402; Not set stays last.
    let lines = &state.components_panel.table.cache.lines;
    let TableLine::Group { value, .. } = &lines[0] else {
        panic!("a group first")
    };
    assert_eq!(
        super::cells::shown(value, &signex_i18n::I18n::en_us()).as_deref(),
        Some("0603")
    );
    apply(&mut state, M::GroupRemoved(0));
    apply(&mut state, M::GroupRemoved(0));
    assert_eq!(group(&state), [SortKey::ascending(p("voltage_max"))]);
}

/// The Columns picker shows, hides and moves columns; Reset to category
/// default drops the saved setup, and the category's view applies again.
#[test]
fn the_columns_picker_edits_and_reset_returns_to_the_default() {
    let mut state = parts();
    show(&mut state, "capacitor");
    apply(&mut state, M::ColumnsToggled);
    assert_eq!(
        state.components_panel.table.popover,
        Some(TablePopover::Columns)
    );
    apply(
        &mut state,
        M::ColumnShown {
            column: ColumnKey::Description,
            shown: true,
        },
    );
    apply(
        &mut state,
        M::ColumnMoved {
            column: p("voltage_max"),
            later: false,
        },
    );
    let visible = state.components_panel.table.cache.setup.visible_columns();
    assert!(visible.contains(&ColumnKey::Description));
    assert_eq!(visible[1], p("voltage_max"), "moved before value");
    let cache = &state.components_panel.table.cache;
    assert!(
        !cache
            .setup
            .is_category_default(&cache.default_view, &cache.offered)
    );
    let fx = apply(&mut state, M::ResetPressed);
    assert!(fx.save_prefs);
    let cache = &state.components_panel.table.cache;
    assert!(
        cache
            .setup
            .is_category_default(&cache.default_view, &cache.offered)
    );
    assert!(
        !state
            .components_panel
            .table
            .setups
            .contains_key("capacitor")
    );
    // Part number cannot be hidden.
    apply(
        &mut state,
        M::ColumnShown {
            column: ColumnKey::PartNumber,
            shown: false,
        },
    );
    assert_eq!(
        state.components_panel.table.cache.setup.visible_columns()[0],
        ColumnKey::PartNumber
    );
}

/// Each category keeps its own setup: a sort in Capacitor leaves All parts
/// as it was, and coming back finds it.
#[test]
fn setups_are_per_category() {
    let mut state = parts();
    show(&mut state, "capacitor");
    apply(&mut state, M::HeaderPressed(p("value")));
    show(&mut state, "*");
    assert_eq!(
        state.components_panel.table.cache.setup.sort,
        [SortKey::ascending(ColumnKey::PartNumber)]
    );
    show(&mut state, "capacitor");
    assert_eq!(
        state.components_panel.table.cache.setup.sort,
        [SortKey::ascending(p("value"))]
    );
}

fn add_range(state: &mut LibraryState, key: &str, unit: Unit) {
    apply(
        state,
        M::RangeFilter(RangeFilterMsg::ColumnPicked {
            key: key.into(),
            unit,
        }),
    );
}

fn type_low(state: &mut LibraryState, key: &str, text: &str) {
    apply(
        state,
        M::RangeFilter(RangeFilterMsg::NumberEdited {
            key: key.into(),
            bound: RangeBound::Low,
            text: text.into(),
        }),
    );
}

fn flat(state: &mut LibraryState) {
    for _ in 0..2 {
        apply(state, M::GroupRemoved(0));
    }
}

/// Decision 6: a new range filter starts on Any prefix, so `= 100` takes
/// 100 pF and 100 nF; picking a prefix compares base values.
#[test]
fn a_range_filter_starts_on_any_prefix_and_a_prefix_narrows_it() {
    let mut state = parts();
    show(&mut state, "capacitor");
    flat(&mut state);
    add_range(&mut state, "value", Unit::Farad);
    let draft = state
        .components_panel
        .table
        .filters()
        .unwrap()
        .range
        .get("value")
        .unwrap();
    assert!(draft.low.any_prefix && draft.high.any_prefix);
    apply(
        &mut state,
        M::RangeFilter(RangeFilterMsg::OperatorPicked {
            key: "value".into(),
            op: signex_library::RangeOp::Equals,
        }),
    );
    type_low(&mut state, "value", "100");
    let mut drawn = drawn_parts(&state);
    drawn.sort();
    assert_eq!(drawn, ["C1", "C2", "C4"]);
    apply(
        &mut state,
        M::BoundPrefixPicked {
            key: "value".into(),
            high: false,
            prefix: Some(SiPrefix::Nano),
        },
    );
    let mut drawn = drawn_parts(&state);
    drawn.sort();
    assert_eq!(drawn, ["C1", "C2"]);
    assert_eq!(
        state
            .components_panel
            .table
            .filters()
            .unwrap()
            .active_count(),
        1
    );
    apply(&mut state, M::FiltersCleared);
    assert_eq!(drawn_parts(&state).len(), 5);
}

/// Decision 8: a min-to-max filter starts on Covers, so 3.3 V finds the
/// part that runs at 3.3 V; a half-set pair has no range to judge and is
/// counted as excluded. Min tests the low end.
#[test]
fn a_min_to_max_filter_covers_and_counts_the_half_set_pair() {
    let mut state = parts();
    show(&mut state, "opamp");
    add_range(&mut state, "supply", Unit::Volt);
    let draft = state
        .components_panel
        .table
        .filters()
        .unwrap()
        .range
        .get("supply")
        .unwrap();
    assert_eq!(draft.on, RangeOn::Covers);
    type_low(&mut state, "supply", "3.3");
    assert_eq!(drawn_parts(&state), ["U1"]);
    assert_eq!(
        state.components_panel.table.cache.excluded.get("supply"),
        Some(&1)
    );
    apply(
        &mut state,
        M::RangeTargetPicked {
            key: "supply".into(),
            on: RangeOn::Min,
        },
    );
    // Min ≥ 3.3: U2's 4.5 V low end.
    assert_eq!(drawn_parts(&state), ["U2"]);
}

/// In stock only drops parts without a cached stock figure.
#[test]
fn in_stock_only_drops_parts_without_stock() {
    let mut state = parts();
    show(&mut state, "*");
    apply(&mut state, M::InStockToggled(true));
    assert!(drawn_parts(&state).is_empty());
    assert_eq!(state.components_panel.table.cache.base.len(), 9);
    apply(&mut state, M::InStockToggled(false));
    assert_eq!(drawn_parts(&state).len(), 9);
}

/// #3236: Stock reads every distributor, not the first: a part the first
/// distributor is out of and the second has 5000 of shows 5000 and stays
/// under In stock only, as the Change Part session's filter keeps it.
#[test]
fn stock_and_in_stock_only_read_every_distributor() {
    use std::collections::BTreeMap;
    let mut state = parts();
    let lib = &mut state.open_libraries[0];
    let rows = lib.tables.values_mut().next().expect("one table");
    let c1 = rows
        .iter_mut()
        .find(|r| r.internal_pn.as_str() == "C1")
        .expect("C1");
    c1.supply = vec![
        signex_library::DistributorListing::new("Mouser", "M-1"),
        signex_library::DistributorListing::new("DigiKey", "D-1"),
    ];
    let row_id = c1.row_id;
    let price = |sku: &str, stock| signex_library::CachedPricing {
        sku: sku.into(),
        currency: "USD".into(),
        stock: Some(stock),
        breaks: Vec::new(),
        fetched_at: chrono::Utc::now(),
    };
    lib.pricing.insert(
        row_id,
        BTreeMap::from([
            (
                "Mouser".to_string(),
                BTreeMap::from([("M-1".to_string(), price("M-1", 0))]),
            ),
            (
                "DigiKey".to_string(),
                BTreeMap::from([("D-1".to_string(), price("D-1", 5000))]),
            ),
        ]),
    );
    show(&mut state, "capacitor");
    let table = &state.components_panel.table;
    let c1 = table
        .cache
        .base
        .iter()
        .find(|r| r.row(&state.open_libraries).unwrap().internal_pn.as_str() == "C1")
        .expect("C1 listed");
    assert_eq!(c1.facts.stock, Some(5000));
    apply(&mut state, M::InStockToggled(true));
    apply(&mut state, M::ExpandAllPressed);
    assert_eq!(drawn_parts(&state), ["C1"]);
}

/// The popovers toggle and only one is open.
#[test]
fn one_popover_at_a_time() {
    let mut state = parts();
    apply(&mut state, M::FilterToggled);
    assert_eq!(
        state.components_panel.table.popover,
        Some(TablePopover::Filter)
    );
    apply(&mut state, M::ColumnsToggled);
    assert_eq!(
        state.components_panel.table.popover,
        Some(TablePopover::Columns)
    );
    apply(&mut state, M::ColumnsToggled);
    assert_eq!(state.components_panel.table.popover, None);
    apply(&mut state, M::FilterToggled);
    apply(&mut state, M::PopoverDismissed);
    assert_eq!(state.components_panel.table.popover, None);
}

/// A new category closes the popover and scrolls the body to its top; a
/// sideways scroll moves the header strip.
#[test]
fn scrolling_and_switching_category() {
    let mut state = parts();
    apply(&mut state, M::FilterToggled);
    let fx = apply(
        &mut state,
        M::CategoryPicked(CategoryKey::Class("opamp".into())),
    );
    assert!(fx.body_to_top);
    assert_eq!(state.components_panel.table.popover, None);
    let fx = apply(
        &mut state,
        M::Scrolled {
            x: 40.0,
            y: 10.0,
            viewport_h: 300.0,
        },
    );
    assert_eq!(fx.header_x, Some(40.0));
    let fx = apply(
        &mut state,
        M::Scrolled {
            x: 40.0,
            y: 30.0,
            viewport_h: 300.0,
        },
    );
    assert_eq!(fx.header_x, None);
}

/// Details lists the pinned parameters first (the template's pins), the
/// rest under Show all; a star pins for the part's category and is saved.
#[test]
fn details_pins_follow_the_parts_category_and_are_saved() {
    let mut state = parts();
    show(&mut state, "*");
    state.components_panel.selected = Some(key_of(&state, "C1"));
    state.sync_view_caches();
    let i18n = signex_i18n::I18n::en_us();
    let (lines, more) = super::detail_lines(&state.components_panel.table, &i18n);
    let names: Vec<_> = lines.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["value", "voltage_max", "dielectric", "package"]);
    assert_eq!(more, Some(6));
    let dielectric = lines.iter().find(|l| l.name == "dielectric").unwrap();
    assert!(!dielectric.is_set);
    assert_eq!(dielectric.value, "Not set");

    let fx = apply(&mut state, M::PinToggled("tolerance".into()));
    assert!(fx.save_prefs);
    assert!(
        state.components_panel.table.setups["capacitor"]
            .pinned
            .contains(&"tolerance".to_owned())
    );
    let (lines, _) = super::detail_lines(&state.components_panel.table, &i18n);
    assert_eq!(lines.last().map(|l| l.name.as_str()), Some("tolerance"));
    apply(&mut state, M::ShowAllParametersToggled);
    let (lines, _) = super::detail_lines(&state.components_panel.table, &i18n);
    assert_eq!(lines.len(), 6);
}

/// #3236: a release anywhere (over the Details pane, captured or not),
/// the pointer leaving the window or the window losing focus ends a
/// column resize, so a later move with no button held changes nothing.
#[test]
fn a_resize_ends_on_a_release_outside_the_table() {
    use iced::mouse::{Button, Event as Mouse};
    let released = iced::Event::Mouse(Mouse::ButtonReleased(Button::Left));
    assert!(super::ends_column_resize(&released));
    assert!(super::ends_column_resize(&iced::Event::Mouse(
        Mouse::CursorLeft
    )));
    assert!(super::ends_column_resize(&iced::Event::Window(
        iced::window::Event::Unfocused
    )));
    assert!(!super::ends_column_resize(&iced::Event::Mouse(
        Mouse::CursorMoved {
            position: iced::Point::new(10.0, 10.0)
        }
    )));
    assert!(!super::ends_column_resize(&iced::Event::Mouse(
        Mouse::ButtonReleased(Button::Right)
    )));

    let mut state = parts();
    show(&mut state, "capacitor");
    let start = state.components_panel.table.width_of(&p("value"));
    apply(&mut state, M::ResizeStarted(p("value")));
    apply(&mut state, M::ResizeMoved(200.0));
    apply(&mut state, M::ResizeMoved(220.0));
    // The window-level release the subscription turns into ResizeEnded.
    let fx = apply(&mut state, M::ResizeEnded);
    assert!(fx.save_prefs);
    assert!(state.components_panel.table.resize.is_none());
    apply(&mut state, M::ResizeMoved(400.0));
    assert_eq!(
        state.components_panel.table.width_of(&p("value")),
        start + 20.0
    );
}

/// #3236: a setup edit while a column's library is unmounted (the column
/// is not offered) keeps that column in the saved setup, with its width.
#[test]
fn a_setup_edit_keeps_the_columns_of_an_unmounted_library() {
    let mut state = parts();
    show(&mut state, "capacitor");
    let table = &mut state.components_panel.table;
    let mut saved = table.cache.setup.clone();
    saved.order.push(p("only_in_library_b"));
    saved.widths.insert(p("only_in_library_b"), 80.0);
    table.setups.insert("capacitor".into(), saved);
    state.sync_view_caches();
    assert!(
        !state
            .components_panel
            .table
            .cache
            .setup
            .order
            .contains(&p("only_in_library_b"))
    );
    let fx = apply(
        &mut state,
        M::ColumnShown {
            column: ColumnKey::Description,
            shown: true,
        },
    );
    assert!(fx.save_prefs);
    let saved = &state.components_panel.table.setups["capacitor"];
    assert!(
        saved.order.contains(&p("only_in_library_b")),
        "{:?}",
        saved.order
    );
    assert_eq!(saved.widths.get(&p("only_in_library_b")), Some(&80.0));
    assert!(!saved.hidden.contains(&ColumnKey::Description));
    assert!(
        state
            .components_panel
            .table
            .cache
            .setup
            .visible_columns()
            .contains(&ColumnKey::Description)
    );
}

/// #3236: a part whose class pins nothing (an op-amp) still has
/// parameters; Details offers them under Show all and does not say "No
/// parameters." above it.
#[test]
fn details_without_pins_offer_show_all_not_no_parameters() {
    let mut state = parts();
    show(&mut state, "*");
    state.components_panel.selected = Some(key_of(&state, "U1"));
    state.sync_view_caches();
    let i18n = signex_i18n::I18n::en_us();
    let (lines, more) = super::detail_lines(&state.components_panel.table, &i18n);
    assert!(lines.is_empty(), "the op-amp template pins nothing");
    let all = more.expect("the op-amp has parameters");
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let section = crate::panels::components_panel::details::parameters_section(
        &state.components_panel.table,
        &tokens,
        &i18n,
    );
    let texts: Vec<String> =
        crate::layout_probe::lay_out_texts(section, iced::Size::new(320.0, 400.0))
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
    let no_parameters = i18n.message_plain(
        signex_i18n::ids::PANELS_COMPONENTS_DETAILS_NO_PARAMETERS,
        &[],
    );
    assert!(!texts.contains(&no_parameters), "{texts:?}");
    let show_all = i18n.message_plain(
        signex_i18n::ids::PANELS_COMPONENTS_SHOW_ALL,
        &[("count", all.into())],
    );
    assert!(texts.contains(&show_all), "{texts:?}");

    // A part with no parameters at all still says so.
    state.components_panel.selected = None;
    state.sync_view_caches();
    let (lines, more) = super::detail_lines(&state.components_panel.table, &i18n);
    assert!(lines.is_empty() && more.is_none());
    let section = crate::panels::components_panel::details::parameters_section(
        &state.components_panel.table,
        &tokens,
        &i18n,
    );
    let texts = crate::layout_probe::lay_out_texts(section, iced::Size::new(320.0, 400.0));
    assert!(texts.iter().any(|t| t.text == no_parameters));
}

/// Caner, 2026-11-27: a min-to-max pair with one end set shows "≥ low" (or
/// "≤ high") in its cell and in Details, and still counts as not set.
#[test]
fn a_half_set_pair_shows_its_one_end() {
    let mut state = parts();
    show(&mut state, "opamp");
    let table = &state.components_panel.table;
    let i18n = signex_i18n::I18n::en_us();
    let text = |pn: &str| {
        let r = table
            .cache
            .base
            .iter()
            .find(|r| r.row(&state.open_libraries).unwrap().internal_pn.as_str() == pn)
            .unwrap();
        super::cells::cell_text(
            r.row(&state.open_libraries).unwrap(),
            &r.facts,
            table.cache.template.as_ref(),
            &p("supply"),
            &i18n,
        )
    };
    assert_eq!(text("U3").as_deref(), Some("≥ 2.7 V"));
    assert_eq!(text("U1").as_deref(), Some("3 to 3.6 V"));
    let high_only = super::test_fixture::opamp("U9", None, Some(5.5));
    let template = table.cache.template.as_ref();
    assert_eq!(
        super::cells::half_set(&high_only, template, "supply", &i18n).as_deref(),
        Some("≤ 5.5 V")
    );
    // Sorted by supply, U3 goes with the parts that have none: last.
    apply(&mut state, M::HeaderRightClicked(p("supply")));
    apply(&mut state, M::HeaderMenuPicked(HeaderAction::SortAscending));
    assert_eq!(drawn_parts(&state), ["U1", "U2", "U3"]);
}

/// Caner, 2026-11-27: the category shown last is saved with the setups and
/// the panel opens on it again; picking the category already shown saves
/// nothing.
#[test]
fn the_last_category_is_saved_and_reopened() {
    let mut state = parts();
    let fx = apply(
        &mut state,
        M::CategoryPicked(CategoryKey::Class("opamp".into())),
    );
    assert!(fx.save_prefs);
    let prefs = state.components_panel.table.prefs();
    assert_eq!(prefs.category.as_deref(), Some("opamp"));
    let reopened = super::ComponentsTable::from_prefs(prefs);
    assert_eq!(reopened.category, CategoryKey::Class("opamp".into()));
    let recent = super::ComponentsTable::from_prefs(crate::fonts::ComponentsViews {
        category: Some("@recent".into()),
        ..Default::default()
    });
    assert_eq!(recent.category, CategoryKey::Recent);
    let fx = apply(
        &mut state,
        M::CategoryPicked(CategoryKey::Class("opamp".into())),
    );
    assert!(!fx.save_prefs);
}
