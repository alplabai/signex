//! Properties of a selected No-ERC directive (#1699, #49): its Location
//! section, then the Suppressed rules section it shares with a Blanket. A
//! directive that names no rule silences every rule, so All rules reads as
//! on. Unticking it opens the list for picking (view state; the document is
//! unchanged until a rule is ticked). Each checkbox is one undo step.
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::directive::{RuleList, suppressed_rules_section};
use super::rows::{PanelStyle, info_row};
use crate::panels::SelectionFormMsg;

/// One Properties edit of a No-ERC directive's rule list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoErcRuleEdit {
    /// Silence every rule: the list is cleared.
    AllRules,
    /// Silence (`true`) or stop silencing (`false`) the rule `rule_id`.
    /// Unticking the last listed rule leaves the list empty, so every rule.
    Rule { rule_id: String, silenced: bool },
}

/// What toggling All rules sends: on → open the list for picking; off with
/// rules listed → clear the list (every rule); off with none listed (the
/// list was only opened) → close it again.
fn all_rules_toggled(id: uuid::Uuid, all_on: bool, listed_any: bool) -> PanelMsg {
    if all_on {
        PanelMsg::SelectionForm(SelectionFormMsg::AllRulesUnticked(id))
    } else if listed_any {
        PanelMsg::EditNoErcRules(id, NoErcRuleEdit::AllRules)
    } else {
        PanelMsg::SelectionForm(SelectionFormMsg::AllRulesTicked(id))
    }
}

pub(super) fn push_no_erc_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    col = super::placed_kinds::push_placed_kind_sections(col, ctx, st);
    let Some(id) = uuid else {
        return col;
    };
    let listed: Vec<String> = signex_types::schematic::parse_erc_exclusions(&info_row(
        &ctx.selection_info,
        signex_engine::info_keys::NO_ERC_RULES,
    ));
    let all_on = ctx.selection_form.no_erc_all_rules_on(id, &listed);
    let list = RuleList {
        key: "sel_no_erc_rules",
        all_on,
        all_toggled: all_rules_toggled(id, all_on, !listed.is_empty()),
        ticked: Box::new(move |rule: &str| listed.iter().any(|r| r == rule)),
        rule_toggled: Box::new(move |rule_id: String, silenced: bool| {
            PanelMsg::EditNoErcRules(id, NoErcRuleEdit::Rule { rule_id, silenced })
        }),
    };
    col.push(suppressed_rules_section(ctx, list, st))
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNUSED_PIN: &str = "Unused pin";

    /// The No-ERC Properties view with `listed` rules, as `picking` says.
    fn view_with(listed: &str, picking: bool) -> (PanelContext, uuid::Uuid) {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        let id = uuid::Uuid::now_v7();
        ctx.selected_kind = Some(signex_types::schematic::SelectedKind::NoErcDirective);
        ctx.selected_uuid = Some(id);
        ctx.selection_info = vec![
            ("Type".into(), "No-ERC".into()),
            (signex_engine::info_keys::NO_ERC_RULES.into(), listed.into()),
        ];
        if picking {
            ctx.selection_form.picking_rules_of = Some(id);
        }
        (ctx, id)
    }

    fn clicked(ctx: &PanelContext, label: &str) -> Vec<PanelMsg> {
        let view = super::super::view_selected_element_properties(
            ctx,
            Color::WHITE,
            Color::WHITE,
            Color::BLACK,
        );
        crate::layout_probe::messages_on_click(view, iced::Size::new(400.0, 4000.0), label)
    }

    fn rule_label() -> String {
        signex_erc::RuleKind::ALL
            .into_iter()
            .map(|kind| kind.label())
            .find(|label| *label == UNUSED_PIN)
            .unwrap_or_else(|| signex_erc::RuleKind::ALL[0].label())
            .to_string()
    }

    /// #49: while All rules is on, the per-rule boxes take no click.
    #[test]
    fn all_rules_on_disables_the_per_rule_boxes() {
        let (ctx, _) = view_with("", false);
        assert!(
            ctx.selection_form
                .no_erc_all_rules_on(ctx.selected_uuid.unwrap(), &[])
        );
        let messages = clicked(&ctx, &rule_label());
        assert!(messages.is_empty(), "{messages:?}");
    }

    /// Once the list is open, a rule box ticks that rule.
    #[test]
    fn an_open_list_ticks_a_rule() {
        let (ctx, id) = view_with("", true);
        let messages = clicked(&ctx, &rule_label());
        assert!(
            messages.iter().any(|m| matches!(
                m,
                PanelMsg::EditNoErcRules(at, NoErcRuleEdit::Rule { silenced: true, .. }) if *at == id
            )),
            "{messages:?}"
        );
    }

    #[test]
    fn all_rules_toggles_open_close_or_clear_the_list() {
        let id = uuid::Uuid::now_v7();
        assert!(matches!(
            all_rules_toggled(id, true, false),
            PanelMsg::SelectionForm(SelectionFormMsg::AllRulesUnticked(_))
        ));
        assert!(matches!(
            all_rules_toggled(id, false, true),
            PanelMsg::EditNoErcRules(_, NoErcRuleEdit::AllRules)
        ));
        assert!(matches!(
            all_rules_toggled(id, false, false),
            PanelMsg::SelectionForm(SelectionFormMsg::AllRulesTicked(_))
        ));
    }
}
