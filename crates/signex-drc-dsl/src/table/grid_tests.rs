//! #2530: the Rules constraint grid. The catalog covers every limit once,
//! a cell resolves by the check's own precedence (and agrees with
//! `signex_drc` on a board), and a cell edit is one span splice.

use std::collections::BTreeMap;

use signex_drc::{DrcRules, Source};
use signex_types::net_class::NetClass;
use signex_types::project::NamedRuleSet;

use super::catalog::{
    ColumnUnit, Domain, catalog, column_of, group_of, object_kinds, object_pairs,
};
use super::grid::{
    CellSource, GridCell, GridScope, reset_cell, resolve, scoped_rules, set_cell, written_scopes,
};
use super::sets::{applied_scopes, apply_set, new_set, writer_of};
use super::{LimitField, table};
use crate::compile;
use crate::fixture::BaseRule;

fn no_sets() -> BTreeMap<String, NamedRuleSet> {
    BTreeMap::new()
}

fn net(name: &str) -> GridScope {
    GridScope::Net(name.into())
}

fn class(name: &str) -> GridScope {
    GridScope::Class(name.into())
}

// ---- catalog ----------------------------------------------------------

#[test]
fn every_limit_is_one_column_and_every_id_is_unique() {
    for field in LimitField::ALL {
        let columns = catalog()
            .iter()
            .flat_map(|g| g.columns.iter())
            .filter(|c| c.field == Some(field))
            .count();
        assert_eq!(columns, 1, "{field:?} is shown by one column");
        assert_eq!(column_of(field).and_then(|c| c.field), Some(field));
        assert_eq!(column_of(field).map(|c| c.unit), Some(ColumnUnit::Mm));
    }
    let mut groups: Vec<&str> = catalog().iter().map(|g| g.id).collect();
    let mut columns: Vec<&str> = catalog()
        .iter()
        .flat_map(|g| g.columns.iter().map(|c| c.id))
        .collect();
    let (g, c) = (groups.len(), columns.len());
    groups.sort_unstable();
    groups.dedup();
    columns.sort_unstable();
    columns.dedup();
    assert_eq!(groups.len(), g, "group ids are unique");
    assert_eq!(columns.len(), c, "column ids are unique");
}

#[test]
fn each_limit_sits_in_its_domain_and_group() {
    use Domain as D;
    use LimitField as F;
    let cases = [
        (F::MinClearance, D::Clearance, "clearance"),
        (F::MinEdgeClearance, D::Manufacturing, "board_outline"),
        (F::MinWidth, D::Physical, "width"),
        (F::MaxWidth, D::Physical, "width"),
        (F::MinDiameter, D::Physical, "via_style"),
        (F::MinDrill, D::Physical, "via_style"),
        (F::MinPadGap, D::Physical, "via_style"),
        (F::MinHole, D::Manufacturing, "hole_size"),
        (F::MaxHole, D::Manufacturing, "hole_size"),
        (F::MinAnnular, D::Manufacturing, "annular_ring"),
        (F::MinHoleSpacing, D::Manufacturing, "hole_to_hole"),
        (F::MinSilkPad, D::Manufacturing, "silk"),
        (F::MinSilkSpacing, D::Manufacturing, "silk"),
        (F::MinSliver, D::Manufacturing, "mask_sliver"),
        (F::MinMaskExpansion, D::Mask, "solder_mask"),
        (F::MaxMaskExpansion, D::Mask, "solder_mask"),
        (F::MinPasteExpansion, D::Mask, "paste_mask"),
        (F::MaxPasteExpansion, D::Mask, "paste_mask"),
        (F::MinCourtyardSpacing, D::Placement, "component_clearance"),
    ];
    assert_eq!(cases.len(), LimitField::ALL.len());
    for (field, domain, id) in cases {
        let group = group_of(field).expect("a group");
        assert_eq!((group.domain, group.id), (domain, id), "{field:?}");
    }
}

#[test]
fn domains_keep_tab_order_and_signal_integrity_has_no_columns() {
    let ids: Vec<&str> = Domain::ALL.iter().map(|d| d.as_str()).collect();
    assert_eq!(
        ids,
        [
            "clearance",
            "physical",
            "high_speed",
            "manufacturing",
            "smt",
            "mask",
            "plane",
            "testpoint",
            "placement",
            "signal_integrity"
        ]
    );
    for domain in Domain::ALL {
        assert_eq!(Domain::from_id(domain.as_str()), Some(domain));
        let groups = domain.groups().count();
        assert_eq!(groups == 0, domain == Domain::SignalIntegrity, "{domain:?}");
    }
    // The catalog runs domain by domain, in tab order.
    let order: Vec<Domain> = catalog().iter().map(|g| g.domain).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
}

#[test]
fn the_object_pair_card_lists_the_objects_a_clearance_rule_measures() {
    assert_eq!(object_kinds(), ["track", "pad", "via"]);
    for [a, b] in object_pairs() {
        let text = format!(
            "rule p on clearance between {a} and {b} require clearance >= 0.2mm then error\n"
        );
        let compiled = compile(&text);
        assert!(
            compiled.diagnostics.is_empty(),
            "{a}-{b}: {:?}",
            compiled.diagnostics
        );
        assert_eq!(compiled.rules.len(), 1, "{a}-{b} is measured");
    }
}

// ---- resolution -------------------------------------------------------

const TEXT: &str = "\
rule all_track on track require width >= 0.15mm and width <= 2mm then error
rule pwr_track on track when class == \"PWR\" require width >= 0.3mm then error
rule vcc_track on track when net == \"VCC\" require width >= 0.25mm then error
rule pwr_clear on clearance when class == \"PWR\" require clearance >= 0.3mm then error
rule pwr_clear2 on clearance when class == \"PWR\" require clearance >= 0.35mm then error
rule all_via on via require diameter >= 0.5mm then error
disabled rule off_via on via when net == \"VCC\" require diameter >= 0.9mm then error
";

fn cell(scope: &GridScope, parents: &[GridScope], field: LimitField) -> Option<GridCell> {
    resolve(&table(TEXT), &no_sets(), scope, parents, field)
}

#[test]
fn a_cell_is_its_own_rule_else_the_nearest_parent_s() {
    let vcc = [class("PWR"), GridScope::All];
    // The net's own rule wins, though looser than its class's.
    assert_eq!(
        cell(&net("VCC"), &vcc, LimitField::MinWidth),
        Some(GridCell {
            value: "0.25mm".into(),
            source: CellSource::Own {
                rule: "vcc_track".into()
            },
        })
    );
    // The maximum comes from All.
    assert_eq!(
        cell(&net("VCC"), &vcc, LimitField::MaxWidth),
        Some(GridCell {
            value: "2mm".into(),
            source: CellSource::Inherited {
                from: GridScope::All,
                rule: "all_track".into()
            },
        })
    );
    // Between two class rules the stricter one governs.
    assert_eq!(
        cell(&net("VCC"), &vcc, LimitField::MinClearance).map(|c| c.source),
        Some(CellSource::Inherited {
            from: class("PWR"),
            rule: "pwr_clear2".into()
        })
    );
    // A disabled rule takes no part.
    assert_eq!(
        cell(&net("VCC"), &vcc, LimitField::MinDiameter).map(|c| c.value),
        Some("0.5mm".into())
    );
    // Nothing sets a hole size.
    assert_eq!(cell(&net("VCC"), &vcc, LimitField::MinHole), None);
    // On All, All's own rule.
    assert_eq!(
        cell(&GridScope::All, &[], LimitField::MinWidth).map(|c| c.source),
        Some(CellSource::Own {
            rule: "all_track".into()
        })
    );
}

#[test]
fn a_tier_floor_is_the_default_and_a_set_rule_names_its_set() {
    let base = "rule fab_track on track require width >= 0.1mm then error\n";
    let set = new_set("min_clearance=0.8mm, min_width=0.3mm", None).unwrap();
    let (text, set) = apply_set(base, "HV_ISO", &set, "class == \"HV\"").unwrap();
    let sets = BTreeMap::from([("HV_ISO".to_string(), set.clone())]);
    let rows = table(&text);
    assert_eq!(
        resolve(&rows, &sets, &GridScope::All, &[], LimitField::MinWidth),
        Some(GridCell {
            value: "0.1mm".into(),
            source: CellSource::Default
        })
    );
    let on_class = resolve(
        &rows,
        &sets,
        &class("HV"),
        &[GridScope::All],
        LimitField::MinClearance,
    )
    .expect("the set's clearance");
    assert_eq!(on_class.value, "0.8mm");
    let CellSource::Set { set: name, rule } = &on_class.source else {
        panic!("{on_class:?}");
    };
    assert_eq!(name, "HV_ISO");
    assert_eq!(writer_of(&sets, rule).map(|(s, _)| s), Some("HV_ISO"));
    assert_eq!(
        writer_of(&sets, &format!("user::{rule}")).map(|(s, _)| s),
        Some("HV_ISO")
    );
    assert_eq!(writer_of(&sets, "fab_track"), None);
    assert_eq!(
        applied_scopes(&set),
        [signex_drc::NetScope::Class("HV".into())]
    );
    // A net of the class inherits it from the class row.
    let on_net = resolve(
        &rows,
        &sets,
        &net("HV_A"),
        &[class("HV"), GridScope::All],
        LimitField::MinClearance,
    );
    assert_eq!(
        on_net.map(|c| c.source),
        Some(CellSource::Inherited {
            from: class("HV"),
            rule: rule.clone()
        })
    );
}

#[test]
fn a_layer_row_holds_its_board_wide_layer_rules() {
    let text = "rule outer_track on track when layer == outer require width >= 0.2mm then error\n";
    let rows = table(text);
    let layer = GridScope::Layer("outer".into());
    assert_eq!(
        resolve(
            &rows,
            &no_sets(),
            &layer,
            &[GridScope::All],
            LimitField::MinWidth
        )
        .map(|c| c.source),
        Some(CellSource::Own {
            rule: "outer_track".into()
        })
    );
    assert_eq!(
        resolve(
            &rows,
            &no_sets(),
            &GridScope::All,
            &[],
            LimitField::MinWidth
        ),
        None
    );
    assert!(scoped_rules(&rows).is_empty());
}

/// The grid and the check pick the same rule and value for a net row:
/// nets 1 `GND`, 2 `VCC`, 3 `SIG`, with `VCC` in class `PWR`.
#[test]
fn grid_resolution_agrees_with_the_check_for_a_net_row() {
    let board = BaseRule::Clearance.board(false);
    let mut pwr = NetClass::named("PWR");
    pwr.nets = vec!["VCC".into()];
    let classes = vec![pwr];
    let compiled = compile(TEXT);
    let drc = DrcRules::for_board(&board, &classes)
        .expect("rules")
        .with_user_rules(&board, &classes, compiled.rules);
    let rows = table(TEXT);
    let nets = [
        (2, net("VCC"), vec![class("PWR"), GridScope::All]),
        (3, net("SIG"), vec![GridScope::All]),
    ];
    for (number, scope, parents) in nets {
        for field in LimitField::ALL {
            let kind = field.limit_kind();
            let checked = drc.effective(kind, number, u32::MAX, [None, None]);
            let shown = resolve(&rows, &no_sets(), &scope, &parents, field);
            match (&checked.source, &shown) {
                (Source::Rule(name), Some(shown)) => {
                    assert_eq!(
                        shown.source.rule(),
                        Some(name.as_str()),
                        "{scope:?} {field:?}"
                    );
                    let row = rows.row(name).expect("the rule's row");
                    let nm = row.compiled.as_ref().and_then(|r| r.limit(kind));
                    assert_eq!(nm, checked.value_nm, "{scope:?} {field:?}");
                }
                (Source::Rule(name), None) => panic!("{scope:?} {field:?}: the check uses {name}"),
                (_, Some(shown)) => panic!("{scope:?} {field:?}: the grid shows {shown:?}"),
                (_, None) => {}
            }
        }
    }
}

// ---- editing ----------------------------------------------------------

/// Odd spacing, so a rewrite of these rules would show.
const KEEP: &str = "rule  keep_a on track   require width >= 0.2mm then error\n\
                    rule keep_b on clearance  when class == \"HV\" require clearance >= 0.8mm then error\n";

#[test]
fn a_new_cell_appends_one_rule_and_its_reset_takes_it_out() {
    let rows = table(KEEP);
    let set = set_cell(KEEP, &rows, &class("PWR"), LimitField::MinWidth, "0.3mm").unwrap();
    assert!(
        set.starts_with(KEEP),
        "the other rules stay as typed: {set}"
    );
    let added = &set[KEEP.len()..];
    assert_eq!(
        added,
        "rule class_PWR_track on track when class == \"PWR\" require width >= 0.3mm then error\n"
    );
    let reset = reset_cell(&set, &table(&set), &class("PWR"), LimitField::MinWidth).unwrap();
    assert_eq!(reset, KEEP);
    // An empty value is a reset.
    assert_eq!(
        set_cell(&set, &table(&set), &class("PWR"), LimitField::MinWidth, " ").unwrap(),
        KEEP
    );
}

#[test]
fn an_own_cell_rewrites_only_its_rule() {
    let rows = table(KEEP);
    let out = set_cell(KEEP, &rows, &class("HV"), LimitField::MinClearance, "1mm").unwrap();
    let first = KEEP.lines().next().unwrap();
    assert!(out.starts_with(first), "{out}");
    assert!(out.contains("clearance >= 1mm"), "{out}");
    assert_eq!(table(&out).rows.len(), 2);
    // A second limit joins the rule; its reset leaves the first.
    let all = set_cell(KEEP, &rows, &GridScope::All, LimitField::MaxWidth, "3mm").unwrap();
    assert!(all.contains("width <= 3mm"), "{all}");
    let keep_b = &KEEP[KEEP.find("rule keep_b").unwrap()..];
    assert!(all.ends_with(keep_b), "keep_b stays as typed: {all}");
    let back = reset_cell(&all, &table(&all), &GridScope::All, LimitField::MaxWidth).unwrap();
    let row = table(&back).row("keep_a").cloned().expect("keep_a stays");
    assert_eq!(row.limits.len(), 1);
    assert_eq!(
        row.limit(LimitField::MinWidth).map(|l| l.value.as_str()),
        Some("0.2mm")
    );
    assert!(
        back.ends_with(&KEEP[KEEP.find("rule keep_b").unwrap()..]),
        "{back}"
    );
}

#[test]
fn a_board_wide_kind_takes_no_class_cell_and_a_tier_rule_is_not_edited() {
    let rows = table(KEEP);
    assert!(set_cell(KEEP, &rows, &class("HV"), LimitField::MinSliver, "0.1mm").is_err());
    let fab = "rule fab_track on track require width >= 0.1mm then error\n";
    let out = set_cell(
        fab,
        &table(fab),
        &GridScope::All,
        LimitField::MinWidth,
        "0.2mm",
    )
    .unwrap();
    assert!(out.starts_with(fab), "the tier floor stays: {out}");
    assert!(
        out.contains("rule all_track on track require width >= 0.2mm"),
        "{out}"
    );
    // Resetting a cell with no own rule changes nothing.
    assert_eq!(
        reset_cell(fab, &table(fab), &GridScope::All, LimitField::MinWidth).unwrap(),
        fab
    );
    // A bad length is refused.
    assert!(set_cell(KEEP, &rows, &GridScope::All, LimitField::MinHole, "wide").is_err());
}

// ---- scoped rules -----------------------------------------------------

#[test]
fn rules_without_a_grid_row_are_scoped_rules() {
    let text = "\
rule plain on track require width >= 0.2mm then error
rule pattern on track when net matches \"DDR_*\" require width >= 0.1mm then error
rule paired on clearance between track and pad require clearance >= 0.3mm then error
rule layered on track when class == \"HV\" and layer == outer require width >= 0.3mm then error
rule cell on clearance when class == \"HV\" and other_class == \"LV\" require clearance >= 1mm then error
rule outer_w on track when layer == outer require width >= 0.25mm then error
rule two on track when net == \"A\" and class == \"B\" require width >= 0.2mm then error
";
    let rows = table(text);
    let names: Vec<&str> = scoped_rules(&rows)
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, ["pattern", "paired", "layered", "two"]);
    assert_eq!(
        written_scopes(&rows),
        [GridScope::All, GridScope::Layer("outer".into())]
    );
}

// ---- review fixes (#2530) ---------------------------------------------

/// The cell shows the stricter of two equal rules; a typed value rewrites
/// that rule, not the first one written.
#[test]
fn a_typed_value_rewrites_the_rule_the_cell_shows() {
    let rows = table(TEXT);
    let pwr = class("PWR");
    let out = set_cell(TEXT, &rows, &pwr, LimitField::MinClearance, "0.32mm").unwrap();
    assert!(
        out.contains(
            "rule pwr_clear on clearance when class == \"PWR\" require clearance >= 0.3mm"
        ),
        "the losing rule stays as typed: {out}"
    );
    assert!(
        out.contains(
            "rule pwr_clear2 on clearance when class == \"PWR\" require clearance >= 0.32mm"
        ),
        "{out}"
    );
    let shown = resolve(
        &table(&out),
        &no_sets(),
        &pwr,
        &[],
        LimitField::MinClearance,
    )
    .expect("a value");
    assert_eq!(shown.value, "0.32mm", "the cell shows what was typed");
}

#[test]
fn a_value_the_cell_would_not_show_is_refused_naming_the_winner() {
    let rows = table(TEXT);
    let refused = set_cell(
        TEXT,
        &rows,
        &class("PWR"),
        LimitField::MinClearance,
        "0.25mm",
    );
    assert_eq!(
        refused,
        Err(super::edit::EditError::Overruled {
            rule: "pwr_clear2".into(),
            by: Some("pwr_clear".into()),
        })
    );
    let said = refused.unwrap_err().to_string();
    assert!(said.contains("pwr_clear"), "{said}");
    // Below a fab floor on All.
    let fab = "rule fab_track on track require width >= 0.1mm then error\n";
    let below = set_cell(
        fab,
        &table(fab),
        &GridScope::All,
        LimitField::MinWidth,
        "0.05mm",
    );
    assert!(
        matches!(&below, Err(super::edit::EditError::Overruled { by: Some(by), .. }) if by == "fab_track"),
        "{below:?}"
    );
}

fn scoped_names(text: &str) -> Vec<String> {
    scoped_rules(&table(text))
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

/// A rule another rule of its row beats on every limit is listed, on All,
/// a class or a net row alike; the winners and the fab floor are not.
#[test]
fn a_losing_duplicate_is_a_scoped_rule_on_any_row() {
    let text = "\
rule wide_a on track require width >= 0.2mm then error
rule wide_b on track require width >= 0.3mm then error
rule cls_a on track when class == \"HV\" require width >= 0.2mm then error
rule cls_b on track when class == \"HV\" require width >= 0.3mm then error
rule net_a on track when net == \"GND\" require width >= 0.2mm then error
rule net_b on track when net == \"GND\" require width >= 0.3mm then error
rule fab_track on track require width >= 0.1mm then error
";
    assert_eq!(scoped_names(text), ["wide_a", "cls_a", "net_a"]);
}

/// A rule no cell shows because it is not applied stays listed, so it can
/// be switched on or removed: one turned off, and one the compiler
/// refuses (a minimum above its maximum).
#[test]
fn a_rule_that_is_off_or_refused_is_a_scoped_rule() {
    let text = "\
rule live on track require width >= 0.2mm then error
disabled rule resting on track when class == \"HV\" require width >= 0.3mm then error
rule vias on via require diameter >= 0.3mm then error
";
    assert_eq!(scoped_names(text), ["resting"]);
    let refused = "\
rule live on track require width >= 0.2mm then error
rule inverted on track when net == \"GND\" require width >= 2mm and width <= 1mm then error
";
    let rows = table(refused);
    assert!(
        !rows.row("inverted").expect("listed in the table").applied,
        "the compiler refuses a minimum above its maximum"
    );
    assert_eq!(scoped_names(refused), ["inverted"]);
}

/// A typed value equal to another rule of the row is the value the cell
/// shows (the first written keeps it), so it is accepted.
#[test]
fn a_typed_value_equal_to_the_winning_rule_is_accepted() {
    let rows = table(TEXT);
    let pwr = class("PWR");
    let out = set_cell(TEXT, &rows, &pwr, LimitField::MinClearance, "0.3mm")
        .expect("equal to pwr_clear, not refused");
    assert!(
        out.contains(
            "rule pwr_clear2 on clearance when class == \"PWR\" require clearance >= 0.3mm"
        ),
        "{out}"
    );
    let shown = resolve(
        &table(&out),
        &no_sets(),
        &pwr,
        &[],
        LimitField::MinClearance,
    )
    .expect("a value");
    assert_eq!(shown.value, "0.3mm");
}
