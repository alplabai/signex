use signex_i18n::{I18n, UserText};

use super::*;
use crate::Label;

fn label(s: &str) -> Label<'static> {
    UserText::new(s).into()
}

const WIDTH: ColumnId = ColumnId(1);
const STYLE: ColumnId = ColumnId(2);
const TENTED: ColumnId = ColumnId(3);
const GAP: ColumnId = ColumnId(4);
const PHYSICAL: GroupId = GroupId(10);
const PAIRS: GroupId = GroupId(11);
const ALL: RowId = RowId(0);
const PWR: RowId = RowId(1);
const VIN: RowId = RowId(2);
const USB: RowId = RowId(3);
const USB_DP: RowId = RowId(4);

fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// All > PWR > VIN_12V, All > USB > USB_D (pair).
fn sample() -> ConstraintGrid {
    let mut grid = ConstraintGrid::new(vec![
        ColumnGroup {
            id: PHYSICAL,
            title: label("Physical"),
            columns: vec![
                GridColumn {
                    id: WIDTH,
                    title: label("Track width"),
                    kind: ColumnKind::Length,
                    default: CellValue::Length(0.2),
                },
                GridColumn {
                    id: STYLE,
                    title: label("Via style"),
                    kind: ColumnKind::Choice(vec![label("Through"), label("Blind")]),
                    default: CellValue::Choice(0),
                },
                GridColumn {
                    id: TENTED,
                    title: label("Tented vias"),
                    kind: ColumnKind::Flag,
                    default: CellValue::Flag(false),
                },
            ],
        },
        ColumnGroup {
            id: PAIRS,
            title: label("Differential pairs"),
            columns: vec![GridColumn {
                id: GAP,
                title: label("Pair gap"),
                kind: ColumnKind::Length,
                default: CellValue::Length(0.15),
            }],
        },
    ]);
    for (id, parent, kind, name) in [
        (ALL, None, RowKind::All, "All"),
        (PWR, Some(ALL), RowKind::Class, "PWR"),
        (VIN, Some(PWR), RowKind::Net, "VIN_12V"),
        (USB, Some(ALL), RowKind::Class, "USB"),
        (USB_DP, Some(USB), RowKind::Pair, "USB_D"),
    ] {
        grid.push_row(GridRow {
            id,
            parent,
            kind,
            name: name.into(),
        })
        .expect("parents come first");
    }
    grid
}

#[test]
fn a_row_needs_its_parent_first_and_a_fresh_id() {
    let mut grid = sample();
    let orphan = GridRow {
        id: RowId(99),
        parent: Some(RowId(98)),
        kind: RowKind::Net,
        name: "X".into(),
    };
    assert_eq!(
        grid.push_row(orphan),
        Err(GridError::UnknownParent(RowId(98)))
    );
    let twin = GridRow {
        id: PWR,
        parent: None,
        kind: RowKind::Class,
        name: "PWR".into(),
    };
    assert_eq!(grid.push_row(twin), Err(GridError::DuplicateRow(PWR)));
}

#[test]
fn a_cell_without_values_resolves_to_the_column_default() {
    let grid = sample();
    let r = grid.resolve(CellRef::new(VIN, WIDTH)).unwrap();
    assert_eq!(r.value, CellValue::Length(0.2));
    assert_eq!(r.source, ValueSource::Default);
}

#[test]
fn a_value_inherits_from_the_nearest_ancestor() {
    let mut grid = sample();
    grid.set_value(CellRef::new(ALL, WIDTH), CellValue::Length(0.15))
        .unwrap();
    grid.set_value(CellRef::new(PWR, WIDTH), CellValue::Length(0.5))
        .unwrap();
    let net = grid.resolve(CellRef::new(VIN, WIDTH)).unwrap();
    assert_eq!(net.value, CellValue::Length(0.5));
    assert_eq!(net.source, ValueSource::Inherited(PWR));
    let class = grid.resolve(CellRef::new(PWR, WIDTH)).unwrap();
    assert!(class.is_own());
    let pair = grid.resolve(CellRef::new(USB_DP, WIDTH)).unwrap();
    assert_eq!(pair.source, ValueSource::Inherited(ALL));
}

#[test]
fn inherited_ignores_the_rows_own_value() {
    let mut grid = sample();
    grid.set_value(CellRef::new(PWR, WIDTH), CellValue::Length(0.5))
        .unwrap();
    grid.set_value(CellRef::new(VIN, WIDTH), CellValue::Length(1.0))
        .unwrap();
    let under = grid.inherited(CellRef::new(VIN, WIDTH)).unwrap();
    assert_eq!(under.value, CellValue::Length(0.5));
    assert_eq!(under.source, ValueSource::Inherited(PWR));
    let root = grid.inherited(CellRef::new(ALL, WIDTH)).unwrap();
    assert_eq!(root.source, ValueSource::Default);
}

#[test]
fn set_value_refuses_a_wrong_type_or_range() {
    let mut grid = sample();
    let cell = CellRef::new(PWR, WIDTH);
    assert_eq!(
        grid.set_value(cell, CellValue::Flag(true)),
        Err(GridError::RejectedValue(cell))
    );
    assert_eq!(
        grid.set_value(cell, CellValue::Length(-0.1)),
        Err(GridError::RejectedValue(cell))
    );
    assert_eq!(
        grid.set_value(cell, CellValue::Length(f64::NAN)),
        Err(GridError::RejectedValue(cell))
    );
    let choice = CellRef::new(PWR, STYLE);
    assert_eq!(
        grid.set_value(choice, CellValue::Choice(2)),
        Err(GridError::RejectedValue(choice))
    );
    let unknown = CellRef::new(RowId(77), WIDTH);
    assert_eq!(
        grid.set_value(unknown, CellValue::Length(1.0)),
        Err(GridError::UnknownCell(unknown))
    );
}

#[test]
fn visible_rows_walk_depth_first_and_skip_collapsed_children() {
    let mut grid = sample();
    let ids: Vec<_> = grid.visible_rows().iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![ALL, PWR, VIN, USB, USB_DP]);
    let depths: Vec<_> = grid.visible_rows().iter().map(|r| r.depth).collect();
    assert_eq!(depths, vec![0, 1, 2, 1, 2]);

    assert_eq!(grid.update(GridMessage::RowToggled(PWR), lower), None);
    let rows = grid.visible_rows();
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![ALL, PWR, USB, USB_DP]);
    let pwr = rows.iter().find(|r| r.id == PWR).unwrap();
    assert!(pwr.has_children && !pwr.is_open);
    assert!(!rows.iter().find(|r| r.id == USB_DP).unwrap().has_children);

    grid.update(GridMessage::RowToggled(PWR), lower);
    assert_eq!(grid.visible_rows().len(), 5);
}

#[test]
fn a_length_edit_commits_on_submit_and_reports_the_change() {
    let mut grid = sample();
    let cell = CellRef::new(PWR, WIDTH);
    grid.update(GridMessage::CellPressed(cell), lower);
    assert_eq!(grid.selected(), Some(cell));
    assert_eq!(grid.edit().map(|e| e.text.as_str()), Some(""));
    grid.update(GridMessage::EditTextChanged("0,45".into()), lower);
    let change = grid.update(GridMessage::EditSubmitted, lower);
    assert_eq!(
        change,
        Some(GridChange::ValueSet {
            cell,
            value: CellValue::Length(0.45)
        })
    );
    assert!(grid.edit().is_none());
    assert!(grid.resolve(cell).unwrap().is_own());
}

#[test]
fn an_unparsable_length_keeps_the_editor_open_and_marked() {
    let mut grid = sample();
    let cell = CellRef::new(PWR, WIDTH);
    grid.update(GridMessage::CellPressed(cell), lower);
    grid.update(GridMessage::EditTextChanged("wide".into()), lower);
    assert_eq!(grid.update(GridMessage::EditSubmitted, lower), None);
    assert!(grid.edit().is_some_and(|e| e.is_invalid));
    assert_eq!(grid.own_value(cell), None);
    grid.update(GridMessage::EditTextChanged("0.3".into()), lower);
    assert!(grid.edit().is_some_and(|e| !e.is_invalid));
}

#[test]
fn an_empty_submit_and_a_reset_drop_the_own_value() {
    let mut grid = sample();
    let cell = CellRef::new(PWR, WIDTH);
    grid.set_value(cell, CellValue::Length(0.5)).unwrap();
    grid.update(GridMessage::CellPressed(cell), lower);
    assert_eq!(grid.edit().map(|e| e.text.as_str()), Some("0.5"));
    grid.update(GridMessage::EditTextChanged(String::new()), lower);
    assert_eq!(
        grid.update(GridMessage::EditSubmitted, lower),
        Some(GridChange::ValueCleared { cell })
    );
    assert_eq!(grid.own_value(cell), None);

    grid.set_value(cell, CellValue::Length(0.5)).unwrap();
    assert_eq!(
        grid.update(GridMessage::CellReset(cell), lower),
        Some(GridChange::ValueCleared { cell })
    );
    assert_eq!(grid.update(GridMessage::CellReset(cell), lower), None);
}

#[test]
fn choice_and_flag_cells_write_typed_values() {
    let mut grid = sample();
    let style = CellRef::new(USB, STYLE);
    grid.update(GridMessage::CellPressed(style), lower);
    assert!(grid.edit().is_some());
    assert_eq!(
        grid.update(GridMessage::ChoicePicked(style, 1), lower),
        Some(GridChange::ValueSet {
            cell: style,
            value: CellValue::Choice(1)
        })
    );
    assert!(grid.edit().is_none());
    assert_eq!(
        grid.update(GridMessage::ChoicePicked(style, 9), lower),
        None
    );

    let tented = CellRef::new(USB, TENTED);
    grid.update(GridMessage::CellPressed(tented), lower);
    assert!(grid.edit().is_none(), "a flag toggles in place");
    assert_eq!(
        grid.update(GridMessage::FlagToggled(tented, true), lower),
        Some(GridChange::ValueSet {
            cell: tented,
            value: CellValue::Flag(true)
        })
    );
    assert_eq!(
        grid.update(GridMessage::FlagToggled(tented, true), lower),
        None,
        "no change, no event"
    );
}

#[test]
fn groups_collapse_and_reopen() {
    let mut grid = sample();
    grid.update(GridMessage::GroupToggled(PAIRS), lower);
    assert!(grid.is_group_collapsed(PAIRS));
    grid.update(GridMessage::GroupToggled(PAIRS), lower);
    assert!(!grid.is_group_collapsed(PAIRS));
}

#[test]
fn find_lands_on_a_column_opens_its_group_and_steps_on_enter() {
    let mut grid = sample();
    grid.update(GridMessage::GroupToggled(PAIRS), lower);
    grid.update(GridMessage::ColumnQueryChanged("GAP".into()), lower);
    assert_eq!(grid.found_column(), Some(GAP));
    assert!(!grid.is_group_collapsed(PAIRS));

    grid.update(GridMessage::ColumnQueryChanged("v".into()), lower);
    assert_eq!(grid.matching_columns("v", lower), vec![STYLE, TENTED]);
    assert_eq!(grid.found_column(), Some(STYLE));
    grid.update(GridMessage::ColumnQuerySubmitted, lower);
    assert_eq!(grid.found_column(), Some(TENTED));
    grid.update(GridMessage::ColumnQuerySubmitted, lower);
    assert_eq!(grid.found_column(), Some(STYLE), "wraps around");

    grid.update(GridMessage::ColumnQueryChanged("  ".into()), lower);
    assert_eq!(grid.found_column(), None);
}

#[test]
fn find_uses_the_locale_search_fold() {
    let i18n = I18n::en_us_ref();
    let grid = sample();
    let fold = |s: &str| i18n.fold_for_search(s);
    assert_eq!(grid.matching_columns("TRACK", fold), vec![WIDTH]);
}

#[test]
fn lengths_parse_and_format_round_trip() {
    assert_eq!(parse_mm(" 0.25 "), Some(0.25));
    assert_eq!(parse_mm("0,25"), Some(0.25));
    assert_eq!(parse_mm("-1"), None);
    assert_eq!(parse_mm("inf"), None);
    assert_eq!(parse_mm("abc"), None);
    assert_eq!(format_mm(0.5), "0.5");
    assert_eq!(format_mm(10.0), "10");
    assert_eq!(format_mm(0.0), "0");
    assert_eq!(format_mm(0.123456), "0.1235");
    for mm in [0.1, 0.127, 2.54, 6.4] {
        assert_eq!(parse_mm(&format_mm(mm)), Some(mm));
    }
}

#[test]
fn grid_width_counts_collapsed_groups_narrow() {
    let mut grid = sample();
    let m = GridMetrics::for_text(12.0);
    let open = grid_width(&grid, m);
    assert_eq!(open, m.name_width + 4.0 * m.cell_width);
    grid.update(GridMessage::GroupToggled(PHYSICAL), lower);
    assert_eq!(
        grid_width(&grid, m),
        m.name_width + m.collapsed_width + m.cell_width
    );
}

#[test]
fn the_view_builds_for_every_cell_state() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let i18n = I18n::en_us_ref();
    let mut grid = sample();
    grid.set_value(CellRef::new(PWR, WIDTH), CellValue::Length(0.5))
        .unwrap();
    let words = GridText {
        object_header: label("Object"),
        set_here: label("Set on this row"),
        inherited_from: label("Inherited from"),
        built_in: label("Built-in default"),
        reset: label("Reset"),
        find_placeholder: label("Find"),
    };
    let style = GridStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
    let _ = view(&grid, i18n, &words, style);
    grid.update(GridMessage::CellPressed(CellRef::new(VIN, WIDTH)), lower);
    let _ = view(&grid, i18n, &words, style);
    grid.update(GridMessage::CellPressed(CellRef::new(VIN, STYLE)), lower);
    grid.update(GridMessage::GroupToggled(PAIRS), lower);
    let _ = view(&grid, i18n, &words, style);
}
