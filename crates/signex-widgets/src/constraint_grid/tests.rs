use std::collections::{BTreeMap, HashMap};

use signex_i18n::I18n;
use signex_types::coord::Unit;

use super::*;
use crate::{Label, LengthFormat};

fn label(s: &str) -> Label<'static> {
    crate::raw(s.to_owned())
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

fn mm() -> LengthFormat<'static> {
    LengthFormat::new(I18n::en_us_ref(), Unit::Mm)
}

/// The caller's side of the contract: rules that inherit down the same
/// hierarchy as the grid's rows, with a default per column. The grid only
/// reads them; a test applies each reported change, as a caller would.
#[derive(Default)]
struct Rules {
    parents: HashMap<RowId, Option<RowId>>,
    defaults: HashMap<ColumnId, CellValue>,
    own: BTreeMap<CellRef, CellValue>,
}

impl Rules {
    fn set(&mut self, cell: CellRef, value: CellValue) {
        self.own.insert(cell, value);
    }

    fn apply(&mut self, change: Option<GridChange>) {
        match change {
            Some(GridChange::ValueSet { cell, value }) => {
                self.own.insert(cell, value);
            }
            Some(GridChange::ValueCleared { cell }) => {
                self.own.remove(&cell);
            }
            None => {}
        }
    }
}

impl CellValues for Rules {
    fn resolve(&self, cell: CellRef) -> Option<Resolved> {
        let default = *self.defaults.get(&cell.column)?;
        let mut row = Some(cell.row).filter(|r| self.parents.contains_key(r))?;
        for _ in 0..=self.parents.len() {
            if let Some(value) = self.own.get(&CellRef::new(row, cell.column)) {
                let source = if row == cell.row {
                    ValueSource::Own
                } else {
                    ValueSource::Inherited(row)
                };
                return Some(Resolved {
                    value: *value,
                    source,
                });
            }
            match self.parents.get(&row).copied().flatten() {
                Some(parent) => row = parent,
                None => break,
            }
        }
        Some(Resolved {
            value: default,
            source: ValueSource::Default,
        })
    }

    fn inherited(&self, cell: CellRef) -> Option<Resolved> {
        match *self.parents.get(&cell.row)? {
            None => Some(Resolved {
                value: *self.defaults.get(&cell.column)?,
                source: ValueSource::Default,
            }),
            Some(parent) => {
                let resolved = self.resolve(CellRef::new(parent, cell.column))?;
                let source = match resolved.source {
                    ValueSource::Own => ValueSource::Inherited(parent),
                    other => other,
                };
                Some(Resolved { source, ..resolved })
            }
        }
    }
}

/// All > PWR > VIN_12V, All > USB > USB_D (pair).
fn sample() -> (ConstraintGrid, Rules) {
    let mut grid = ConstraintGrid::new(vec![
        ColumnGroup {
            id: PHYSICAL,
            title: label("Physical"),
            columns: vec![
                GridColumn {
                    id: WIDTH,
                    title: label("Track width"),
                    kind: ColumnKind::Length,
                },
                GridColumn {
                    id: STYLE,
                    title: label("Via style"),
                    kind: ColumnKind::Choice(vec![label("Through"), label("Blind")]),
                },
                GridColumn {
                    id: TENTED,
                    title: label("Tented vias"),
                    kind: ColumnKind::Flag,
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
            }],
        },
    ]);
    let mut rules = Rules::default();
    rules.defaults.extend([
        (WIDTH, CellValue::Length(0.2)),
        (STYLE, CellValue::Choice(0)),
        (TENTED, CellValue::Flag(false)),
        (GAP, CellValue::Length(0.15)),
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
        rules.parents.insert(id, parent);
    }
    (grid, rules)
}

#[test]
fn a_row_needs_its_parent_first_and_a_fresh_id() {
    let (mut grid, _) = sample();
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
    let (_, rules) = sample();
    let r = rules.resolve(CellRef::new(VIN, WIDTH)).unwrap();
    assert_eq!(r.value, CellValue::Length(0.2));
    assert_eq!(r.source, ValueSource::Default);
    assert_eq!(rules.own_value(CellRef::new(VIN, WIDTH)), None);
}

#[test]
fn a_value_inherits_from_the_nearest_ancestor() {
    let (_, mut rules) = sample();
    rules.set(CellRef::new(ALL, WIDTH), CellValue::Length(0.15));
    rules.set(CellRef::new(PWR, WIDTH), CellValue::Length(0.5));
    let net = rules.resolve(CellRef::new(VIN, WIDTH)).unwrap();
    assert_eq!(net.value, CellValue::Length(0.5));
    assert_eq!(net.source, ValueSource::Inherited(PWR));
    let class = rules.resolve(CellRef::new(PWR, WIDTH)).unwrap();
    assert!(class.is_own());
    assert_eq!(
        rules.own_value(CellRef::new(PWR, WIDTH)),
        Some(CellValue::Length(0.5))
    );
    let pair = rules.resolve(CellRef::new(USB_DP, WIDTH)).unwrap();
    assert_eq!(pair.source, ValueSource::Inherited(ALL));
}

#[test]
fn inherited_ignores_the_rows_own_value() {
    let (_, mut rules) = sample();
    rules.set(CellRef::new(PWR, WIDTH), CellValue::Length(0.5));
    rules.set(CellRef::new(VIN, WIDTH), CellValue::Length(1.0));
    let under = rules.inherited(CellRef::new(VIN, WIDTH)).unwrap();
    assert_eq!(under.value, CellValue::Length(0.5));
    assert_eq!(under.source, ValueSource::Inherited(PWR));
    let root = rules.inherited(CellRef::new(ALL, WIDTH)).unwrap();
    assert_eq!(root.source, ValueSource::Default);
}

#[test]
fn the_grid_refuses_a_wrong_type_or_range() {
    let (mut grid, rules) = sample();
    let width = grid.column(WIDTH).unwrap();
    assert!(!width.accepts(CellValue::Flag(true)));
    assert!(!width.accepts(CellValue::Length(-0.1)));
    assert!(!width.accepts(CellValue::Length(f64::NAN)));
    assert!(width.accepts(CellValue::Length(0.1)));
    let choice = CellRef::new(PWR, STYLE);
    assert_eq!(
        grid.update(GridMessage::ChoicePicked(choice, 2), &rules, mm()),
        None
    );
    let wrong_type = CellRef::new(PWR, WIDTH);
    assert_eq!(
        grid.update(GridMessage::FlagToggled(wrong_type, true), &rules, mm()),
        None
    );
    let unknown = CellRef::new(RowId(77), STYLE);
    assert_eq!(
        grid.update(GridMessage::ChoicePicked(unknown, 0), &rules, mm()),
        None
    );
}

#[test]
fn visible_rows_walk_depth_first_and_skip_collapsed_children() {
    let (mut grid, rules) = sample();
    let ids: Vec<_> = grid.visible_rows().iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![ALL, PWR, VIN, USB, USB_DP]);
    let depths: Vec<_> = grid.visible_rows().iter().map(|r| r.depth).collect();
    assert_eq!(depths, vec![0, 1, 2, 1, 2]);

    assert_eq!(
        grid.update(GridMessage::RowToggled(PWR), &rules, mm()),
        None
    );
    let rows = grid.visible_rows();
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![ALL, PWR, USB, USB_DP]);
    let pwr = rows.iter().find(|r| r.id == PWR).unwrap();
    assert!(pwr.has_children && !pwr.is_open);
    assert!(!rows.iter().find(|r| r.id == USB_DP).unwrap().has_children);

    grid.update(GridMessage::RowToggled(PWR), &rules, mm());
    assert_eq!(grid.visible_rows().len(), 5);
}

#[test]
fn a_length_edit_commits_on_submit_and_reports_the_change() {
    let (mut grid, mut rules) = sample();
    let cell = CellRef::new(PWR, WIDTH);
    grid.update(GridMessage::CellPressed(cell), &rules, mm());
    assert_eq!(grid.selected(), Some(cell));
    assert_eq!(grid.edit().map(|e| e.text.as_str()), Some(""));
    grid.update(GridMessage::EditTextChanged("0.45".into()), &rules, mm());
    let change = grid.update(GridMessage::EditSubmitted, &rules, mm());
    assert_eq!(
        change,
        Some(GridChange::ValueSet {
            cell,
            value: CellValue::Length(0.45)
        })
    );
    assert!(grid.edit().is_none());
    assert!(
        !rules.resolve(cell).unwrap().is_own(),
        "the grid stores nothing; the caller applies the change"
    );
    rules.apply(change);
    assert!(rules.resolve(cell).unwrap().is_own());
}

#[test]
fn an_unparsable_or_negative_length_keeps_the_editor_open_and_marked() {
    let (mut grid, rules) = sample();
    let cell = CellRef::new(PWR, WIDTH);
    grid.update(GridMessage::CellPressed(cell), &rules, mm());
    for typed in ["wide", "-0.1", "inf"] {
        grid.update(GridMessage::EditTextChanged(typed.into()), &rules, mm());
        assert_eq!(
            grid.update(GridMessage::EditSubmitted, &rules, mm()),
            None,
            "{typed}"
        );
        assert!(grid.edit().is_some_and(|e| e.is_invalid), "{typed}");
    }
    grid.update(GridMessage::EditTextChanged("0.3".into()), &rules, mm());
    assert!(grid.edit().is_some_and(|e| !e.is_invalid));
}

#[test]
fn an_empty_submit_and_a_reset_drop_the_own_value() {
    let (mut grid, mut rules) = sample();
    let cell = CellRef::new(PWR, WIDTH);
    rules.set(cell, CellValue::Length(0.5));
    grid.update(GridMessage::CellPressed(cell), &rules, mm());
    assert_eq!(grid.edit().map(|e| e.text.as_str()), Some("0.500"));
    grid.update(GridMessage::EditTextChanged(String::new()), &rules, mm());
    let change = grid.update(GridMessage::EditSubmitted, &rules, mm());
    assert_eq!(change, Some(GridChange::ValueCleared { cell }));
    rules.apply(change);
    assert_eq!(rules.own_value(cell), None);

    rules.set(cell, CellValue::Length(0.5));
    let change = grid.update(GridMessage::CellReset(cell), &rules, mm());
    assert_eq!(change, Some(GridChange::ValueCleared { cell }));
    rules.apply(change);
    assert_eq!(
        grid.update(GridMessage::CellReset(cell), &rules, mm()),
        None
    );
}

#[test]
fn choice_and_flag_cells_write_typed_values() {
    let (mut grid, mut rules) = sample();
    let style = CellRef::new(USB, STYLE);
    grid.update(GridMessage::CellPressed(style), &rules, mm());
    assert!(grid.edit().is_some());
    assert_eq!(
        grid.update(GridMessage::ChoicePicked(style, 1), &rules, mm()),
        Some(GridChange::ValueSet {
            cell: style,
            value: CellValue::Choice(1)
        })
    );
    assert!(grid.edit().is_none());
    assert_eq!(
        grid.update(GridMessage::ChoicePicked(style, 9), &rules, mm()),
        None
    );

    let tented = CellRef::new(USB, TENTED);
    grid.update(GridMessage::CellPressed(tented), &rules, mm());
    assert!(grid.edit().is_none(), "a flag toggles in place");
    let change = grid.update(GridMessage::FlagToggled(tented, true), &rules, mm());
    assert_eq!(
        change,
        Some(GridChange::ValueSet {
            cell: tented,
            value: CellValue::Flag(true)
        })
    );
    rules.apply(change);
    assert_eq!(
        grid.update(GridMessage::FlagToggled(tented, true), &rules, mm()),
        None,
        "no change, no event"
    );
}

#[test]
fn groups_collapse_and_reopen() {
    let (mut grid, rules) = sample();
    grid.update(GridMessage::GroupToggled(PAIRS), &rules, mm());
    assert!(grid.is_group_collapsed(PAIRS));
    grid.update(GridMessage::GroupToggled(PAIRS), &rules, mm());
    assert!(!grid.is_group_collapsed(PAIRS));
}

#[test]
fn find_lands_on_a_column_opens_its_group_and_steps_on_enter() {
    let (mut grid, rules) = sample();
    let fold = |s: &str| I18n::en_us_ref().fold_for_search(s);
    grid.update(GridMessage::GroupToggled(PAIRS), &rules, mm());
    grid.update(GridMessage::ColumnQueryChanged("GAP".into()), &rules, mm());
    assert_eq!(grid.found_column(), Some(GAP));
    assert!(!grid.is_group_collapsed(PAIRS));

    grid.update(GridMessage::ColumnQueryChanged("v".into()), &rules, mm());
    assert_eq!(grid.matching_columns("v", fold), vec![STYLE, TENTED]);
    assert_eq!(grid.found_column(), Some(STYLE));
    grid.update(GridMessage::ColumnQuerySubmitted, &rules, mm());
    assert_eq!(grid.found_column(), Some(TENTED));
    grid.update(GridMessage::ColumnQuerySubmitted, &rules, mm());
    assert_eq!(grid.found_column(), Some(STYLE), "wraps around");

    grid.update(GridMessage::ColumnQueryChanged("  ".into()), &rules, mm());
    assert_eq!(grid.found_column(), None);
}

#[test]
fn find_uses_the_locale_search_fold() {
    let i18n = I18n::en_us_ref();
    let (grid, _) = sample();
    let fold = |s: &str| i18n.fold_for_search(s);
    assert_eq!(grid.matching_columns("TRACK", fold), vec![WIDTH]);
}

#[test]
fn typed_lengths_follow_the_display_unit_and_round_trip() {
    let (mut grid, mut rules) = sample();
    let cell = CellRef::new(PWR, WIDTH);
    let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
    rules.set(cell, CellValue::Length(0.254));
    grid.update(GridMessage::CellPressed(cell), &rules, mil);
    assert_eq!(grid.edit().map(|e| e.text.as_str()), Some("10.00"));
    grid.update(GridMessage::EditTextChanged("20".into()), &rules, mil);
    match grid.update(GridMessage::EditSubmitted, &rules, mil) {
        Some(GridChange::ValueSet {
            value: CellValue::Length(set),
            ..
        }) => assert!((set - 0.508).abs() < 1e-12),
        other => panic!("expected a length, got {other:?}"),
    }

    // Opening a value and submitting it unchanged reports nothing.
    for value in [0.1, 0.127, 2.54, 6.4] {
        rules.set(cell, CellValue::Length(value));
        grid.update(GridMessage::CellPressed(cell), &rules, mm());
        assert_eq!(
            grid.update(GridMessage::EditSubmitted, &rules, mm()),
            None,
            "{value}"
        );
    }
}

#[test]
fn grid_width_counts_collapsed_groups_narrow() {
    let (mut grid, rules) = sample();
    let m = GridMetrics::for_text(12.0);
    let open = grid_width(&grid, m);
    assert_eq!(open, m.name_width + 4.0 * m.cell_width);
    grid.update(GridMessage::GroupToggled(PHYSICAL), &rules, mm());
    assert_eq!(
        grid_width(&grid, m),
        m.name_width + m.collapsed_width + m.cell_width
    );
}

#[test]
fn the_view_builds_for_every_cell_state() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let (mut grid, mut rules) = sample();
    rules.set(CellRef::new(PWR, WIDTH), CellValue::Length(0.5));
    let words = GridText {
        object_header: label("Object"),
        set_here: label("Set on this row"),
        inherited_from: label("Inherited from"),
        built_in: label("Built-in default"),
        reset: label("Reset"),
        find_placeholder: label("Find"),
    };
    let style = GridStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
    let _ = view(&grid, &rules, mm(), &words, style);
    grid.update(
        GridMessage::CellPressed(CellRef::new(VIN, WIDTH)),
        &rules,
        mm(),
    );
    let _ = view(&grid, &rules, mm(), &words, style);
    grid.update(
        GridMessage::CellPressed(CellRef::new(VIN, STYLE)),
        &rules,
        mm(),
    );
    grid.update(GridMessage::GroupToggled(PAIRS), &rules, mm());
    let _ = view(&grid, &rules, mm(), &words, style);
}

/// The editor opens with a rounded value (0.1016 mm opens as 0.102, 0.1 mm
/// in mil as 3.94); submitting it untouched must not write that back.
#[test]
fn an_untouched_editor_reports_no_change() {
    let (mut grid, mut rules) = sample();
    let cell = CellRef::new(PWR, WIDTH);
    rules.set(cell, CellValue::Length(0.1016));
    grid.update(GridMessage::CellPressed(cell), &rules, mm());
    assert_eq!(grid.edit().map(|e| e.text.as_str()), Some("0.102"));
    assert_eq!(grid.update(GridMessage::EditSubmitted, &rules, mm()), None);
    assert!(grid.edit().is_none());

    rules.set(cell, CellValue::Length(0.1));
    let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
    grid.update(GridMessage::CellPressed(cell), &rules, mil);
    assert_eq!(grid.update(GridMessage::EditSubmitted, &rules, mil), None);
}
