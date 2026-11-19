//! The design rules the schematic's Net Parameters directives ask for
//! (#1281): per class (or per net, for a directive with no class) a track
//! width rule `Width_<CLASS>`, a clearance rule `Clearance_<CLASS>` and a via
//! rule `Via_<CLASS>`. Update PCB lists them beside the board's changes,
//! differences first, and Execute writes them into the project's DRC rule
//! text as one step of the rule history, each name recorded in
//! `drc.from_schematic` so the Rules tab shows where it came from. The
//! schematic wins: a rule of the same name edited in the Rules tab is
//! rewritten (DECISIONS, "schematic wins on Update PCB").

use signex_drc::NetScope;
use signex_drc_dsl::table::{self, EditError, LimitField, RowLimit, RuleDraft, RuleKind};
use signex_types::schematic::{NetRules, SchDirective, SchematicSheet};

/// One rule the schematic asks for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SchematicRule {
    pub name: String,
    pub draft: RuleDraft,
}

impl SchematicRule {
    /// The rule as its line of rule text.
    pub(crate) fn text(&self) -> String {
        table::render(&self.draft)
    }
}

/// How a wanted rule stands against the project's rule text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuleChange {
    /// A rule of that name is written differently; Execute rewrites it.
    Changed { was: String },
    /// No rule has that name; Execute adds it.
    Added,
    /// Written as asked; nothing to do.
    Unchanged,
}

/// One row of the Rules from the schematic list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SchematicRuleRow {
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

/// Every rule the Net Parameters directives of `sheets` ask for, in sheet
/// and directive order; a later directive asking for a rule name already
/// asked for is left out (the first one wins, as for a class).
pub(crate) fn schematic_rules<'s>(
    sheets: impl IntoIterator<Item = &'s SchematicSheet>,
) -> Vec<SchematicRule> {
    let mut out: Vec<SchematicRule> = Vec::new();
    for sheet in sheets {
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
                None => (NetScope::Net(on.net_name.clone()), on.net_name.clone()),
            };
            for rule in rules_for(&scope, &name, &on.rules) {
                if !out.iter().any(|r| r.name == rule.name) {
                    out.push(rule);
                }
            }
        }
    }
    out
}

/// How each of `rules` stands against the rule text `dsl`: changed rows
/// first (the differences with a rule edited in the Rules tab), then added,
/// then unchanged.
pub(crate) fn rule_rows(rules: &[SchematicRule], dsl: &str) -> Vec<SchematicRuleRow> {
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
pub(crate) fn pending(rows: &[SchematicRuleRow]) -> usize {
    rows.iter()
        .filter(|r| r.change != RuleChange::Unchanged)
        .count()
}

/// `dsl` with every row of `rows` written as asked: a changed rule
/// rewritten in place, an added one appended.
pub(crate) fn merged_text(dsl: &str, rows: &[SchematicRuleRow]) -> Result<String, EditError> {
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

impl crate::app::Signex {
    /// The schematic's rules for the open Update PCB dialog against the
    /// project's rule text now; empty before the list is read.
    pub(crate) fn update_pcb_rule_rows(
        &self,
        state: &super::UpdatePcbState,
    ) -> Vec<SchematicRuleRow> {
        let Some(snapshot) = state.snapshot.as_ref() else {
            return Vec::new();
        };
        let dsl = self
            .rules_text_of(state.project)
            .map(|(text, _)| text)
            .unwrap_or_default();
        rule_rows(&snapshot.schematic_rules, &dsl)
    }

    /// Execute's rule half: write the changed and added rules into the
    /// project's DRC rules as one rule-history step, each marked as from
    /// the schematic. Nothing pending writes nothing.
    pub(crate) fn apply_update_pcb_rules(
        &mut self,
        project: crate::app::state::ProjectId,
        rows: &[SchematicRuleRow],
    ) -> Result<(), String> {
        if pending(rows) == 0 {
            return Ok(());
        }
        let dsl = self
            .rules_text_of(project)
            .map(|(text, _)| text)
            .unwrap_or_default();
        let text = merged_text(&dsl, rows).map_err(|e| e.to_string())?;
        let names: Vec<String> = rows.iter().map(|r| r.rule.name.clone()).collect();
        self.commit_drc_section(project, move |drc| {
            drc.dsl = (!text.trim().is_empty()).then_some(text);
            for name in names {
                if !drc.from_schematic.contains(&name) {
                    drc.from_schematic.push(name);
                }
            }
        })
    }
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
