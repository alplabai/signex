//! Properties of a selected directive (#2420, #49). Every directive starts
//! with its Location; a Blanket then edits its net class and, one checkbox
//! per ERC rule, the rules it does not report. Typing in the class field is
//! one undo step (the engine's typing run); each checkbox is one step.
//! The Suppressed rules section is shared with the No-ERC directive.
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use crate::panels::SelectionFormMsg;
use crate::panels::multi_selection::{counted_section, inset_caption, rule_matches};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{ids, t};
use signex_widgets::enable_section::{EnableSection, dependents_veil};
use signex_widgets::inert::Inert;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One Properties edit of a Blanket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlanketEdit {
    /// The net class field as typed; blank sets none.
    NetClass(String),
    /// Report (`false`) or do not report (`true`) the rule `rule_id` inside.
    Rule { rule_id: String, excluded: bool },
}

/// The `Type` row the engine writes for a Blanket.
const BLANKET_TYPE: &str = "Blanket";

/// The rule list of a directive, as the Suppressed rules section shows it.
pub(super) struct RuleList<'a> {
    /// The collapse key of the section.
    pub key: &'static str,
    /// Whether All rules is on: every rule silenced, the list disabled.
    pub all_on: bool,
    /// What ticking or unticking All rules sends.
    pub all_toggled: PanelMsg,
    /// Whether the rule `id` is ticked.
    pub ticked: Box<dyn Fn(&str) -> bool + 'a>,
    /// What ticking (`true`) or unticking a rule sends.
    pub rule_toggled: Box<dyn Fn(String, bool) -> PanelMsg + 'a>,
}

/// The built-in ERC rules: id and the name signex-erc gives it.
fn builtin_rules() -> impl Iterator<Item = (String, &'static str)> {
    signex_erc::RuleKind::ALL.into_iter().map(|kind| {
        (
            signex_erc::RuleId::builtin(kind).as_str().to_string(),
            kind.label(),
        )
    })
}

/// Suppressed rules: the All rules enable row first, then the filter, a
/// hairline and one box per rule. While All rules is on, the filter and the
/// boxes are disabled and a caption says why. The header counts the rules
/// that are off.
pub(super) fn suppressed_rules_section<'a>(
    ctx: &'a PanelContext,
    list: RuleList<'a>,
    st: PanelStyle,
) -> Element<'a, PanelMsg> {
    let total = signex_erc::RuleKind::ALL.len();
    let off = if list.all_on {
        total
    } else {
        builtin_rules().filter(|(id, _)| (list.ticked)(id)).count()
    };
    let pill = t!(ctx.i18n, ids::PROPERTIES_RULES_OFF, "count" => off)
        .to_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    counted_section(
        ctx,
        list.key,
        ids::PROPERTIES_SUPPRESSED_RULES,
        Some(pill),
        move || rules_body(ctx, list, total, st),
    )
}

fn rules_body<'a>(
    ctx: &'a PanelContext,
    list: RuleList<'a>,
    total: usize,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let RuleList {
        all_on,
        all_toggled,
        ticked,
        rule_toggled,
        ..
    } = list;
    let header = EnableSection::new(
        t!(i18n, ids::PROPERTIES_ALL_RULES, "count" => total),
        all_on,
    )
    .on_toggle(move |_| all_toggled.clone())
    .view(
        crate::panels::enable_section::enable_section_style(&ctx.tokens),
        Space::new().boxed(),
    );
    let query = ctx.selection_form.rule_query.clone();
    let filter = signex_widgets::text_input(t!(i18n, ids::PROPERTIES_RULE_FILTER), query.clone())
        .on_input(|s| PanelMsg::SelectionForm(SelectionFormMsg::RuleQueryChanged(s)))
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .style(crate::panels::form_look::text_input_style);
    let mut rules = Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(
            container(filter)
                .padding([SP.xs, SP.l])
                .width(Length::Fill)
                .boxed(),
        )
        .push(thin_sep(st.border_c));
    let mut shown = 0;
    for (rule_id, name) in builtin_rules().filter(|(_, name)| rule_matches(i18n, name, &query)) {
        let on = ticked(&rule_id);
        shown += 1;
        rules = rules.push(form_check_row(
            // raw: the rule's name comes from signex-erc (#2062).
            raw(name),
            on,
            rule_toggled(rule_id, !on),
            st.muted,
            st.palette.panel_bg,
        ));
    }
    if shown == 0 {
        rules = rules.push(inset_caption(
            t!(i18n, ids::PROPERTIES_RULE_NO_MATCH, "query" => query.trim()),
            st.muted,
        ));
    }
    let rules: Element<'a, PanelMsg> = match dependents_veil(st.palette.panel_bg, !all_on) {
        Some(veil) => Inert::new(rules.boxed(), veil).boxed(),
        None => rules.boxed(),
    };
    let mut body = Column::new().spacing(0).width(Length::Fill).push(header);
    if all_on {
        body = body.push(inset_caption(
            t!(i18n, ids::PROPERTIES_ALL_RULES_ON),
            st.muted,
        ));
    }
    body.push(rules)
}

pub(super) fn push_directive_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    col = super::placed_kinds::push_placed_kind_sections(col, ctx, st);
    let Some(id) = uuid else {
        return col;
    };
    if info_row(&ctx.selection_info, "Type") != BLANKET_TYPE {
        return col;
    }
    let muted = st.muted;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let class = info_row(&ctx.selection_info, "Net Class");
    let excluded: Vec<String> = signex_types::schematic::parse_erc_exclusions(&info_row(
        &ctx.selection_info,
        "ERC Exclusions",
    ));
    let all = signex_types::schematic::ALL_ERC_RULES;
    let all_on = excluded.iter().any(|r| r == all);
    col = col.push(
        ctx.collapsible(
            "sel_blanket",
            ids::PANELS_DIRECTIVE_BLANKET,
            &ctx.collapsed_sections,
            move || {
                Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_edit_row(
                        t!(i18n, ids::PANELS_DIRECTIVE_NET_CLASS),
                        &class,
                        muted,
                        move |value| PanelMsg::EditBlanket(id, BlanketEdit::NetClass(value)),
                    ))
            },
        )
        .boxed(),
    );
    let list = RuleList {
        key: "sel_blanket_rules",
        all_on,
        all_toggled: PanelMsg::EditBlanket(
            id,
            BlanketEdit::Rule {
                rule_id: all.to_string(),
                excluded: !all_on,
            },
        ),
        ticked: Box::new(move |rule: &str| excluded.iter().any(|r| r == rule)),
        rule_toggled: Box::new(move |rule_id: String, excluded: bool| {
            PanelMsg::EditBlanket(id, BlanketEdit::Rule { rule_id, excluded })
        }),
    };
    col.push(suppressed_rules_section(ctx, list, st))
}
