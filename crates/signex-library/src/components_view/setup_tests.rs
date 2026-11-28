use signex_types::quantity::RangeEnd;
use uuid::Uuid;

use super::test_rows::{INTERFACE_IC, interface_ic, interface_rows, m, row, text};
use super::*;
use crate::templates::{
    CategoryView, ColumnKey, ParameterTemplate, RangeSlot, SortKey, TemplateRegistry,
};

fn col(name: &str) -> ColumnKey {
    ColumnKey::param(name)
}

fn ic_setup() -> (ViewSetup, CategoryView, Vec<ColumnKey>) {
    let template = interface_ic();
    let offered = offered_columns(Some(&template), &interface_rows());
    let view = category_view(Some(&template));
    (ViewSetup::from_view(&view, &offered), view, offered)
}

#[test]
fn column_keys_are_written_as_strings() {
    let keys = [
        ColumnKey::PartNumber,
        ColumnKey::Description,
        ColumnKey::Manufacturer,
        ColumnKey::LibraryRef,
        ColumnKey::Footprint,
        ColumnKey::Stock,
        ColumnKey::Price,
        col("value"),
    ];
    let json = serde_json::to_string(&keys).unwrap();
    assert_eq!(
        json,
        r#"["row:part_number","row:description","row:manufacturer","row:library_ref","row:footprint","row:stock","row:price","value"]"#
    );
    let back: Vec<ColumnKey> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, keys);
    // A row field a newer build added survives as written.
    let unknown: ColumnKey = serde_json::from_str(r#""row:lifecycle""#).unwrap();
    assert_eq!(unknown, col("row:lifecycle"));
    assert_eq!(
        serde_json::to_string(&unknown).unwrap(),
        r#""row:lifecycle""#
    );
}

/// mockup row 52: a category's configuration, edited with the
/// class template, sets its default columns, sort and grouping.
#[test]
fn the_template_view_parses_and_round_trips() {
    let t = interface_ic();
    assert_eq!(
        t.ranges[1],
        RangeSlot {
            name: "temperature".into(),
            min: "temp_min".into(),
            max: "temp_max".into(),
            sort_by: RangeEnd::Max,
        }
    );
    let view = t.view.clone().unwrap();
    assert_eq!(view.sort, [SortKey::descending(col("rate"))]);
    assert_eq!(view.group, [SortKey::ascending(col("protocol"))]);
    assert_eq!(view.columns[0], ColumnKey::PartNumber);
    let written = toml::to_string_pretty(&t).unwrap();
    assert_eq!(ParameterTemplate::parse(&written).unwrap(), t);
    assert!(
        !written.contains("sort_by = \"min\""),
        "the default end is not written"
    );
    assert!(INTERFACE_IC.contains("descending = true"));
}

/// A template without the new keys writes exactly what it wrote before.
#[test]
fn a_template_without_a_view_writes_no_new_keys() {
    let t = ParameterTemplate::parse("class = \"x\"\n").unwrap();
    assert!(t.ranges.is_empty() && t.view.is_none());
    let written = toml::to_string_pretty(&t).unwrap();
    assert!(
        !written.contains("view") && !written.contains("ranges"),
        "{written}"
    );
}

#[test]
fn parameter_columns_collapse_a_pair_into_one_column() {
    let t = interface_ic();
    assert_eq!(
        t.parameter_columns(),
        [
            col("protocol"),
            col("rate"),
            col("supply"),
            col("package"),
            col("temperature"),
            col("tolerance")
        ]
    );
    let mut orphan = ParameterTemplate::default();
    orphan.ranges.push(RangeSlot {
        name: "span".into(),
        min: "a".into(),
        max: "b".into(),
        sort_by: RangeEnd::Min,
    });
    assert_eq!(orphan.parameter_columns(), [col("span")]);
}

#[test]
fn the_fallback_view_shows_every_parameter_sorted_by_part_number() {
    let mut t = interface_ic();
    t.view = None;
    let view = t.category_view();
    let mut want = vec![ColumnKey::PartNumber];
    want.extend(t.parameter_columns());
    want.extend([ColumnKey::Stock, ColumnKey::Price]);
    assert_eq!(view.columns, want);
    assert_eq!(view.sort, [SortKey::ascending(ColumnKey::PartNumber)]);
    assert!(view.group.is_empty());
    assert_eq!(category_view(None), CategoryView::all_parts());
}

/// The bundled capacitor and resistor categories open as mockup row 52
/// draws them; the op-amp pairs its supply.
#[test]
fn builtin_templates_carry_the_mockup_views() {
    let r = TemplateRegistry::new_with_builtins();
    let cap = r.resolve(Uuid::nil(), "capacitor").unwrap().category_view();
    assert_eq!(cap.sort, [SortKey::ascending(col("voltage_max"))]);
    assert_eq!(
        cap.group,
        [
            SortKey::ascending(col("package")),
            SortKey::ascending(col("value"))
        ]
    );
    let res = r.resolve(Uuid::nil(), "resistor").unwrap().category_view();
    assert_eq!(res.sort, [SortKey::ascending(col("tolerance"))]);
    assert_eq!(res.columns.len(), 8);
    let opamp = r.resolve(Uuid::nil(), "opamp").unwrap();
    assert!(opamp.parameter_columns().contains(&col("supply")));
    for t in r.global_templates() {
        let offered = offered_columns(Some(t), []);
        for column in &t.category_view().columns {
            assert!(offered.contains(column), "{} offers {column}", t.class);
        }
    }
}

#[test]
fn offered_columns_add_free_parameters_and_the_row_fields() {
    let template = interface_ic();
    let mut rows = interface_rows();
    rows.push(row(
        "X",
        &[("ecad_note", text("x")), ("designator_prefix", text("U"))],
    ));
    let offered = offered_columns(Some(&template), &rows);
    let mut want = vec![ColumnKey::PartNumber];
    want.extend(template.parameter_columns());
    want.push(col("ecad_note"));
    want.extend(ColumnKey::ROW_FIELDS.into_iter().skip(1));
    assert_eq!(offered, want);
    assert_eq!(offered_columns(None, &rows), ColumnKey::ROW_FIELDS);
}

#[test]
fn a_new_setup_is_the_category_default() {
    let (setup, view, offered) = ic_setup();
    assert!(setup.is_category_default(&view, &offered));
    assert_eq!(setup.visible_columns(), view.columns);
    assert_eq!(setup.pinned, ["protocol", "rate"]);
    // Offered columns the view does not show are hidden after the shown ones.
    let choices = setup.column_choices();
    assert_eq!(choices.len(), offered.len());
    assert!(
        choices[..view.columns.len()]
            .iter()
            .all(|(_, shown)| *shown)
    );
    assert!(
        choices[view.columns.len()..]
            .iter()
            .all(|(_, shown)| !*shown)
    );
}

/// mockup row 52: changes are the user's; Reset to
/// category default returns to the configuration.
#[test]
fn any_change_leaves_the_category_default() {
    let (base, view, offered) = ic_setup();
    let changes: [&dyn Fn(&mut ViewSetup); 5] = [
        &|s| s.hide_column(&col("package")),
        &|s| s.show_column(&ColumnKey::Description),
        &|s| s.set_width(&col("rate"), 120.0),
        &|s| s.click_sort(&col("package"), true),
        &|s| s.toggle_group(&col("package")),
    ];
    for change in changes {
        let mut s = base.clone();
        change(&mut s);
        assert!(!s.is_category_default(&view, &offered), "{s:?}");
    }
    let mut pinned = base.clone();
    pinned.toggle_pin("package");
    assert!(
        pinned.is_category_default(&view, &offered),
        "pins are not part of it"
    );
    let reset = ViewSetup::from_view(&view, &offered);
    assert_eq!(reset, base);
}

#[test]
fn part_number_stays_first_and_visible() {
    let (mut s, _, _) = ic_setup();
    s.hide_column(&ColumnKey::PartNumber);
    s.move_column_to(&ColumnKey::PartNumber, &col("rate"), true);
    s.move_column_to(&col("rate"), &ColumnKey::PartNumber, false);
    assert_eq!(s.visible_columns()[0], ColumnKey::PartNumber);
    assert_eq!(s.visible_columns()[1], col("rate"));
}

#[test]
fn columns_move_by_drag_and_among_their_peers() {
    let (mut s, _, _) = ic_setup();
    s.move_column_to(&ColumnKey::Stock, &col("protocol"), false);
    assert_eq!(
        &s.visible_columns()[..3],
        [ColumnKey::PartNumber, ColumnKey::Stock, col("protocol")]
    );
    let (mut s, _, _) = ic_setup();
    s.move_column_by(&col("rate"), -1);
    assert_eq!(&s.visible_columns()[1..3], [col("rate"), col("protocol")]);
    // The first parameter cannot move up past part number.
    let before = s.clone();
    s.move_column_by(&col("rate"), -1);
    assert_eq!(s, before);
    // A row field swaps with the next row field, skipping parameters; a
    // hidden row field swaps with the next hidden one (#3236: Stock, the
    // only visible row field, has no visible peer to swap with).
    let before = s.clone();
    s.move_column_by(&ColumnKey::Stock, 1);
    assert_eq!(s, before);
    let at = |s: &ViewSetup, c: &ColumnKey| s.order.iter().position(|x| x == c).unwrap();
    let (library_ref, description) = (
        at(&s, &ColumnKey::LibraryRef),
        at(&s, &ColumnKey::Description),
    );
    s.move_column_by(&ColumnKey::LibraryRef, 1);
    assert_eq!(
        (
            at(&s, &ColumnKey::LibraryRef),
            at(&s, &ColumnKey::Description)
        ),
        (description, library_ref)
    );
    assert_eq!(at(&s, &col("tolerance")), at(&before, &col("tolerance")));
}

/// #3236: Up and Down in the Columns picker move a visible column among
/// the visible ones; a hidden column between them is skipped, so every
/// press changes the visible order.
#[test]
fn moving_a_column_skips_hidden_columns_between() {
    let mut s = ViewSetup {
        order: vec![
            ColumnKey::PartNumber,
            col("value"),
            col("tolerance"),
            col("power"),
        ],
        hidden: [col("tolerance")].into_iter().collect(),
        ..ViewSetup::default()
    };
    s.move_column_by(&col("power"), -1);
    assert_eq!(
        s.visible_columns(),
        [ColumnKey::PartNumber, col("power"), col("value")]
    );
    assert_eq!(s.column_choices().last(), Some(&(col("tolerance"), false)));
    s.move_column_by(&col("power"), 1);
    assert_eq!(
        s.visible_columns(),
        [ColumnKey::PartNumber, col("value"), col("power")]
    );
}

/// Header clicks as mockup row 52 draws them: a click sorts by that
/// column alone, again descending, again off; Shift+click adds nested
/// keys the same way.
#[test]
fn header_clicks_cycle_the_sort() {
    let mut s = ViewSetup::default();
    let a = col("rate");
    let b = col("package");
    s.click_sort(&a, false);
    assert_eq!(s.sort, [SortKey::ascending(a.clone())]);
    s.click_sort(&a, false);
    assert_eq!(s.sort, [SortKey::descending(a.clone())]);
    s.click_sort(&a, false);
    assert!(s.sort.is_empty());
    s.click_sort(&a, false);
    s.click_sort(&b, true);
    assert_eq!(
        s.sort,
        [SortKey::ascending(a.clone()), SortKey::ascending(b.clone())]
    );
    s.click_sort(&b, true);
    assert_eq!(s.sort[1], SortKey::descending(b.clone()));
    s.click_sort(&b, true);
    assert_eq!(s.sort, [SortKey::ascending(a.clone())]);
    s.click_sort(&b, true);
    s.click_sort(&a, false);
    assert_eq!(
        s.sort,
        [SortKey::ascending(a.clone())],
        "a plain click replaces a nested sort"
    );
    s.sort_only(&b, true);
    assert_eq!(s.sort, [SortKey::descending(b.clone())]);
    s.toggle_in_sort(&a);
    s.toggle_in_sort(&b);
    assert_eq!(s.sort, [SortKey::ascending(a)]);
}

#[test]
fn the_group_bar_inserts_moves_flips_and_removes_levels() {
    let mut s = ViewSetup::default();
    let (p, k, v) = (col("protocol"), col("package"), col("value"));
    s.insert_group(&p, 0);
    s.insert_group(&k, 9);
    s.insert_group(&v, 1);
    assert_eq!(
        s.group.iter().map(|g| g.column.clone()).collect::<Vec<_>>(),
        [p.clone(), v.clone(), k.clone()]
    );
    s.flip_group(0);
    s.insert_group(&p, 3);
    assert_eq!(
        s.group.last(),
        Some(&SortKey::descending(p.clone())),
        "a moved level keeps its direction"
    );
    s.remove_group(0);
    s.remove_group(7);
    assert_eq!(s.group.len(), 2);
    s.toggle_group(&k);
    assert_eq!(s.group, [SortKey::descending(p)]);
}

/// Saved setups live in the user's preferences: they survive JSON, and a
/// setup saved before the template changed is fitted to it.
#[test]
fn a_saved_setup_round_trips_and_fits_a_changed_template() {
    let (mut s, _, offered) = ic_setup();
    s.set_width(&col("rate"), 96.0);
    s.hide_column(&col("package"));
    s.toggle_group(&col("supply"));
    let json = serde_json::to_string(&s).unwrap();
    let back: ViewSetup = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.clone().fitted(&offered), back);
    let mut smaller = interface_ic();
    smaller.ranges.retain(|r| r.name != "supply");
    let offered = offered_columns(Some(&smaller), &interface_rows());
    let fitted = back.fitted(&offered);
    assert!(!fitted.order.contains(&col("supply")));
    assert_eq!(
        fitted.group,
        [SortKey::ascending(col("protocol"))],
        "a group on a removed column goes"
    );
    assert!(fitted.order.contains(&col("supply_min")));
    assert!(
        fitted.hidden.contains(&col("supply_min")),
        "a new column arrives hidden"
    );
    assert_eq!(fitted.widths.get(&col("rate")), Some(&96.0));
}

#[test]
fn a_bad_width_clears_and_setups_load_with_defaults() {
    let mut s = ViewSetup::default();
    s.set_width(&col("rate"), 80.0);
    s.set_width(&col("rate"), f32::NAN);
    assert!(s.widths.is_empty());
    let minimal: ViewSetup = serde_json::from_str(r#"{"order":["row:part_number"]}"#).unwrap();
    assert!(minimal.sort.is_empty() && minimal.hidden.is_empty() && minimal.pinned.is_empty());
}

/// Details lists every parameter with its value, pinned ones first.
#[test]
fn details_list_pinned_parameters_first() {
    let template = interface_ic();
    let mut r = interface_rows().remove(1);
    r.parameters.insert("ecad_note".into(), text("x"));
    let pinned = vec!["temperature".to_string(), "nonsense".to_string()];
    let entries = detail_parameters(&r, Some(&template), &pinned);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "temperature",
            "protocol",
            "rate",
            "supply",
            "package",
            "tolerance",
            "ecad_note"
        ]
    );
    assert!(entries[0].pinned && !entries[1].pinned);
    assert!(
        entries[5].value.is_missing(),
        "an empty slot is listed as not set"
    );
    let bare = detail_parameters(
        &row("Y", &[("b", m(1.0, "V")), ("a", text("z"))]),
        None,
        &[],
    );
    let names: Vec<&str> = bare.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
}

/// #3236: an edit made while a column is not offered (its library is
/// unmounted) keeps that column's place, hidden flag, width and the saved
/// sort; the setup as shown is exactly the edit.
#[test]
fn an_edit_keeps_the_columns_not_offered_now() {
    let (mut saved, _, offered) = ic_setup();
    let esr = col("esr");
    let at = saved.order.iter().position(|c| *c == col("rate")).unwrap() + 1;
    saved.order.insert(at, esr.clone());
    saved.widths.insert(esr.clone(), 80.0);
    saved.sort.insert(0, SortKey::ascending(esr.clone()));
    let shown = saved.clone().fitted(&offered);
    assert!(!shown.order.contains(&esr));

    let mut edited = shown.clone();
    edited.set_width(&col("rate"), 90.0);
    let kept = edited.clone().keeping_unoffered(&saved, &shown, &offered);
    assert_eq!(kept.order[at - 1..=at], [col("rate"), esr.clone()]);
    assert_eq!(kept.widths.get(&esr), Some(&80.0));
    assert_eq!(kept.widths.get(&col("rate")), Some(&90.0));
    assert_eq!(kept.sort, saved.sort);
    assert_eq!(kept.clone().fitted(&offered), edited);

    // A sort the user changes is theirs: the hidden key does not return.
    let mut sorted = shown.clone();
    sorted.click_sort(&col("package"), false);
    let kept = sorted.clone().keeping_unoffered(&saved, &shown, &offered);
    assert_eq!(kept.sort, sorted.sort);
    assert!(kept.order.contains(&esr));
}
