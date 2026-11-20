//! Properties of a selected No-ERC directive (#1699): its Location section,
//! then one checkbox per ERC rule for the rules it silences. "All Rules" is
//! checked when it names none, which silences every rule. Each checkbox is
//! one undo step. Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use iced::Widget;
use signex_i18n::{ids, t};

/// One Properties edit of a No-ERC directive's rule list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoErcRuleEdit {
    /// Silence every rule: the list is cleared.
    AllRules,
    /// Silence (`true`) or stop silencing (`false`) the rule `rule_id`.
    /// Unticking the last listed rule leaves the list empty, so every rule.
    Rule { rule_id: String, silenced: bool },
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
    let muted = st.muted;
    let surface = st.palette.panel_bg;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let listed: Vec<String> = signex_types::schematic::parse_erc_exclusions(&info_row(
        &ctx.selection_info,
        signex_engine::info_keys::NO_ERC_RULES,
    ));
    col.push(
        ctx.collapsible(
            "sel_no_erc_rules",
            ids::PANELS_NO_ERC_RULES,
            &ctx.collapsed_sections,
            move || {
                let every = listed.is_empty();
                let mut rows = Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_check_row(
                        t!(i18n, ids::PANELS_NO_ERC_ALL_RULES),
                        every,
                        PanelMsg::EditNoErcRules(id, NoErcRuleEdit::AllRules),
                        muted,
                        surface,
                    ));
                for kind in signex_erc::RuleKind::ALL {
                    let rule_id = signex_erc::RuleId::builtin(kind).as_str().to_string();
                    let on = listed.contains(&rule_id);
                    rows = rows.push(form_check_row(
                        // raw: the rule's name comes from signex-erc (#2062).
                        raw(kind.label()),
                        on,
                        PanelMsg::EditNoErcRules(
                            id,
                            NoErcRuleEdit::Rule {
                                rule_id,
                                silenced: !on,
                            },
                        ),
                        muted,
                        surface,
                    ));
                }
                rows
            },
        )
        .boxed(),
    )
}
