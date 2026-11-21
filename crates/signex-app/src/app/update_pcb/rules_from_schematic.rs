//! Update PCB's half of the rules the schematic's Net Parameters directives
//! ask for (#1489): the dialog's rows against the project's rule text now,
//! the Messages line of a rule left out, and Execute writing them into the
//! project's DRC rule text as one step of the rule history, each name
//! recorded in `drc.from_schematic` so the Rules tab shows where it came
//! from. Which rules and how they merge is domain logic in
//! `signex_drc_dsl::schematic_rules`.

pub(crate) use signex_drc_dsl::schematic_rules::{
    RuleChange, RuleProblem, SchematicRule, SchematicRuleRow, merged_text, pending, rule_rows,
    schematic_rules,
};

/// The Messages line of a rule left out.
pub(crate) fn problem_line(
    i18n: &signex_i18n::I18n,
    problem: &RuleProblem,
) -> signex_i18n::Localized {
    use signex_i18n::{ids, t};
    match problem {
        RuleProblem::NetNotOnBoard { net } => {
            t!(i18n, ids::UPDATE_PCB_RULE_NET_NOT_ON_BOARD, "net" => net.as_str())
        }
        RuleProblem::NameClash {
            rule,
            kept,
            dropped,
        } => t!(
            i18n,
            ids::UPDATE_PCB_RULE_NAME_CLASH,
            "rule" => rule.as_str(),
            "kept" => kept.as_str(),
            "dropped" => dropped.as_str()
        ),
    }
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
