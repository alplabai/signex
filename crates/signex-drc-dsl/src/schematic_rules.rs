//! The design rules the schematic's Net Parameters directives ask for
//! (#1489): per class (or per net, for a directive with no class) a track
//! width rule `Width_<CLASS>`, a clearance rule `Clearance_<CLASS>` and a via
//! rule `Via_<CLASS>`, how each stands against a project's rule text, and
//! that text with them written in. The schematic wins: a rule of the same
//! name edited in the Rules tab is rewritten (DECISIONS, "schematic wins on
//! Update PCB"). The app's Update PCB lists and writes them
//! (`signex-app` `app::update_pcb::rules_from_schematic`).

use std::collections::HashSet;

use crate::table::{self, EditError, LimitField, RowLimit, RuleDraft, RuleKind};
use signex_drc::NetScope;
use signex_types::schematic::{InstancePath, NetRules, SchDirective, SchematicSheet};

/// One rule the schematic asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct SchematicRule {
    pub name: String,
    pub draft: RuleDraft,
}

impl SchematicRule {
    /// The rule as its line of rule text.
    pub fn text(&self) -> String {
        table::render(&self.draft)
    }
}

/// How a wanted rule stands against the project's rule text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleChange {
    /// A rule of that name is written differently; Execute rewrites it.
    Changed { was: String },
    /// No rule has that name; Execute adds it.
    Added,
    /// Written as asked; nothing to do.
    Unchanged,
}

/// One row of the Rules from the schematic list.
#[derive(Debug, Clone, PartialEq)]
pub struct SchematicRuleRow {
    pub rule: SchematicRule,
    pub change: RuleChange,
}

/// A rule name part from a class or net name: letters, digits and `_`.
fn name_part(scope: &str) -> String {
    scope
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// A length limit as the rule text writes it.
fn mm(value: f64) -> String {
    let text = format!("{value:.4}");
    format!("{}mm", text.trim_end_matches('0').trim_end_matches('.'))
}

/// The rules one directive's `rules` ask for on `scope`.
fn rules_for(scope: &NetScope, scope_name: &str, rules: &NetRules) -> Vec<SchematicRule> {
    let part = name_part(scope_name);
    let draft = |kind: RuleKind, name: String, limits: Vec<RowLimit>| SchematicRule {
        name: name.clone(),
        draft: RuleDraft {
            scope: scope.clone(),
            limits,
            ..RuleDraft::new(kind, name)
        },
    };
    let limit = |field: LimitField, value: f64| RowLimit {
        field,
        value: mm(value),
    };
    let mut out = Vec::new();
    if let Some(w) = rules.width_mm {
        out.push(draft(
            RuleKind::Track,
            format!("Width_{part}"),
            vec![limit(LimitField::MinWidth, w)],
        ));
    }
    if let Some(c) = rules.clearance_mm {
        out.push(draft(
            RuleKind::Clearance,
            format!("Clearance_{part}"),
            vec![limit(LimitField::MinClearance, c)],
        ));
    }
    let via: Vec<RowLimit> = [
        rules
            .via_diameter_mm
            .map(|d| limit(LimitField::MinDiameter, d)),
        rules.via_hole_mm.map(|h| limit(LimitField::MinDrill, h)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !via.is_empty() {
        out.push(draft(RuleKind::Via, format!("Via_{part}"), via));
    }
    out
}

/// A wanted rule that Update PCB leaves out, said in Messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuleProblem {
    /// A directive with no class on a net that is not one board net by that
    /// name: a sheet placed under a sheet symbol (its nets are renamed by
    /// the placement, or per channel) or a name the project does not have.
    NetNotOnBoard { net: String },
    /// Two scopes map to one rule name (`V+BUS` and `V-BUS` both give
    /// `Width_V_BUS`); the first scope keeps it.
    NameClash {
        rule: String,
        kept: String,
        dropped: String,
    },
}

/// The class or net a scope names.
fn scope_label(scope: &NetScope) -> String {
    match scope {
        NetScope::Class(name) | NetScope::Net(name) => name.clone(),
        other => format!("{other:?}"),
    }
}

/// Every rule the Net Parameters directives of the placed `sheets` ask
/// for, in walk and directive order, and the rules left out. A directive
/// with no class makes a net rule only on a root page whose net is a net
/// of the stitched `project_nets`; elsewhere its net name is local and
/// would match no board net. A later directive asking for a rule name
/// already asked for on the same scope is left out (the first one wins, as
/// for a class); on another scope it is a clash and reported.
pub fn schematic_rules<'s>(
    sheets: impl IntoIterator<Item = (&'s InstancePath, &'s SchematicSheet)>,
    project_nets: &HashSet<&str>,
) -> (Vec<SchematicRule>, Vec<RuleProblem>) {
    let mut out: Vec<SchematicRule> = Vec::new();
    let mut problems: Vec<RuleProblem> = Vec::new();
    let mut note = |problem: RuleProblem| {
        if !problems.contains(&problem) {
            problems.push(problem);
        }
    };
    for (path, sheet) in sheets {
        if !sheet
            .directives
            .iter()
            .any(|d| matches!(d, SchDirective::NetParameters { .. }))
        {
            continue;
        }
        let netlist = signex_net::build_netlist(sheet);
        for on in signex_net::net_parameters(sheet, &netlist) {
            let (scope, name) = match &on.net_class {
                Some(class) => (NetScope::Class(class.clone()), class.clone()),
                None if path.is_root() && project_nets.contains(on.net_name.as_str()) => {
                    (NetScope::Net(on.net_name.clone()), on.net_name.clone())
                }
                None => {
                    note(RuleProblem::NetNotOnBoard {
                        net: on.net_name.clone(),
                    });
                    continue;
                }
            };
            for rule in rules_for(&scope, &name, &on.rules) {
                match out.iter().find(|r| r.name == rule.name) {
                    None => out.push(rule),
                    Some(kept) if kept.draft.scope == rule.draft.scope => {}
                    Some(kept) => note(RuleProblem::NameClash {
                        rule: rule.name.clone(),
                        kept: scope_label(&kept.draft.scope),
                        dropped: scope_label(&rule.draft.scope),
                    }),
                }
            }
        }
    }
    (out, problems)
}

/// How each of `rules` stands against the rule text `dsl`: changed rows
/// first (the differences with a rule edited in the Rules tab), then added,
/// then unchanged.
pub fn rule_rows(rules: &[SchematicRule], dsl: &str) -> Vec<SchematicRuleRow> {
    let written = table::table(dsl);
    let mut rows: Vec<SchematicRuleRow> = rules
        .iter()
        .map(|rule| {
            let change = match written.row(&rule.name) {
                None => RuleChange::Added,
                Some(row) => {
                    let was = row.text(dsl).trim().to_string();
                    if was == rule.text() {
                        RuleChange::Unchanged
                    } else {
                        RuleChange::Changed { was }
                    }
                }
            };
            SchematicRuleRow {
                rule: rule.clone(),
                change,
            }
        })
        .collect();
    rows.sort_by_key(|row| match row.change {
        RuleChange::Changed { .. } => 0,
        RuleChange::Added => 1,
        RuleChange::Unchanged => 2,
    });
    rows
}

/// How many of `rows` Execute changes.
pub fn pending(rows: &[SchematicRuleRow]) -> usize {
    rows.iter()
        .filter(|r| r.change != RuleChange::Unchanged)
        .count()
}

/// `dsl` with every row of `rows` written as asked: a changed rule
/// rewritten in place, an added one appended.
pub fn merged_text(dsl: &str, rows: &[SchematicRuleRow]) -> Result<String, EditError> {
    let mut text = dsl.to_string();
    for row in rows {
        text = match row.change {
            RuleChange::Unchanged => continue,
            RuleChange::Changed { .. } => {
                table::replace_rule(&text, &row.rule.name, &row.rule.draft)?
            }
            RuleChange::Added => table::add_rule(&text, &row.rule.draft)?.0,
        };
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power_in() -> Vec<SchematicRule> {
        rules_for(
            &NetScope::Class("POWER_IN".to_string()),
            "POWER_IN",
            &NetRules {
                width_mm: Some(0.8),
                clearance_mm: Some(0.3),
                via_diameter_mm: Some(0.6),
                via_hole_mm: Some(0.3),
                current_a: Some(3.0),
            },
        )
    }

    #[test]
    fn a_class_gets_width_clearance_and_via_rules_and_current_makes_none() {
        let rules = power_in();
        let names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["Width_POWER_IN", "Clearance_POWER_IN", "Via_POWER_IN"]
        );
        assert_eq!(
            rules[0].text(),
            "rule Width_POWER_IN on track when class == \"POWER_IN\" require width >= 0.8mm then error"
        );
        assert!(
            rules[2].text().contains("diameter >= 0.6mm"),
            "{}",
            rules[2].text()
        );
        assert!(
            rules[2].text().contains("drill >= 0.3mm"),
            "{}",
            rules[2].text()
        );
        // A net with no class names the rules after the net.
        let by_net = rules_for(
            &NetScope::Net("V+BUS".to_string()),
            "V+BUS",
            &NetRules {
                width_mm: Some(1.0),
                ..NetRules::default()
            },
        );
        assert_eq!(by_net[0].name, "Width_V_BUS");
    }

    /// One labelled wire per `(net, class)` with a resistor pin on its start
    /// (a net needs a terminal), each with a Net Parameters directive asking
    /// for a 1 mm track on it.
    fn sheet_with(nets: &[(&str, Option<&str>)]) -> SchematicSheet {
        use signex_types::schematic::{
            Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol,
            Wire,
        };
        let mut sheet = SchematicSheet::default();
        let mut body: LibSymbol = serde_json::from_str(r#"{"id":"R"}"#).expect("body");
        body.pins = vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::ZERO,
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }];
        sheet.lib_symbols.insert("R".to_string(), body);
        for (i, (net, class)) in nets.iter().enumerate() {
            let y = 20.0 * i as f64;
            sheet.symbols.push(Symbol {
                uuid: uuid::Uuid::from_u128(200 + i as u128),
                lib_id: "R".into(),
                reference: format!("R{}", i + 1),
                position: Point::new(0.0, y),
                ..Default::default()
            });
            sheet.wires.push(Wire {
                uuid: uuid::Uuid::from_u128(i as u128 + 1),
                start: Point::new(0.0, y),
                end: Point::new(10.0, y),
                ..Default::default()
            });
            sheet
                .labels
                .push(Label::new(*net, Point::new(0.0, y), LabelType::Net));
            sheet.directives.push(SchDirective::NetParameters {
                uuid: uuid::Uuid::from_u128(100 + i as u128),
                position: Point::new(5.0, y),
                net_class: class.map(str::to_string),
                diff_pair_class: None,
                rules: NetRules {
                    width_mm: Some(1.0),
                    ..NetRules::default()
                },
                show_on_flag: false,
            });
        }
        sheet
    }

    fn names(rules: &[SchematicRule]) -> Vec<&str> {
        rules.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn a_net_rule_comes_only_from_a_root_page_net_the_board_has() {
        let sheet = sheet_with(&[("VBUS", None), ("SDA", Some("I2C"))]);
        let board: HashSet<&str> = ["VBUS", "SDA"].into_iter().collect();
        let root = InstancePath::root();
        let (rules, problems) = schematic_rules([(&root, &sheet)], &board);
        assert_eq!(names(&rules), ["Width_VBUS", "Width_I2C"]);
        assert!(problems.is_empty(), "{problems:?}");

        // The same sheet placed under a sheet symbol (or repeated): its net
        // takes another name on the board, so no net rule, and it is said.
        let placed = InstancePath::from_chain(vec![uuid::Uuid::from_u128(7)]);
        let (rules, problems) = schematic_rules([(&placed, &sheet)], &board);
        assert_eq!(names(&rules), ["Width_I2C"], "a class rule still applies");
        assert_eq!(
            problems,
            [RuleProblem::NetNotOnBoard {
                net: "VBUS".to_string()
            }]
        );
    }

    #[test]
    fn two_scopes_on_one_rule_name_keep_the_first_and_say_so() {
        let sheet = sheet_with(&[("V+BUS", None), ("V-BUS", None)]);
        let board: HashSet<&str> = ["V+BUS", "V-BUS"].into_iter().collect();
        let root = InstancePath::root();
        let (rules, problems) = schematic_rules([(&root, &sheet)], &board);
        assert_eq!(names(&rules), ["Width_V_BUS"]);
        assert_eq!(rules[0].draft.scope, NetScope::Net("V+BUS".to_string()));
        assert_eq!(
            problems,
            [RuleProblem::NameClash {
                rule: "Width_V_BUS".to_string(),
                kept: "V+BUS".to_string(),
                dropped: "V-BUS".to_string(),
            }]
        );
    }

    #[test]
    fn a_rule_edited_in_the_rules_tab_is_listed_first_and_the_schematic_wins() {
        let rules = power_in();
        let edited = "rule Width_POWER_IN on track when class == \"POWER_IN\" require width >= 0.5mm then error\n";
        let rows = rule_rows(&rules, edited);
        assert_eq!(rows[0].rule.name, "Width_POWER_IN");
        assert_eq!(
            rows[0].change,
            RuleChange::Changed {
                was: edited.trim().to_string()
            }
        );
        assert!(rows[1..].iter().all(|r| r.change == RuleChange::Added));
        assert_eq!(pending(&rows), 3);

        let merged = merged_text(edited, &rows).expect("the rules merge");
        assert!(merged.contains("width >= 0.8mm"), "{merged}");
        assert!(!merged.contains("width >= 0.5mm"), "{merged}");
        let again = rule_rows(&rules, &merged);
        assert!(
            again.iter().all(|r| r.change == RuleChange::Unchanged),
            "{again:?}"
        );
        assert_eq!(pending(&again), 0);
    }
}
