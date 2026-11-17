//! Properties of a selected directive (#2420). Every directive lists its
//! region or position; a Blanket also edits its net class and, one checkbox
//! per ERC rule, the rules it does not report. Typing in the class field is
//! one undo step (the engine's typing run); each checkbox is one step.
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use iced::Widget;
use signex_i18n::{ids, t};

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
    let surface = st.palette.panel_bg;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let class = info_row(&ctx.selection_info, "Net Class");
    let excluded: Vec<String> = signex_types::schematic::parse_erc_exclusions(&info_row(
        &ctx.selection_info,
        "ERC Exclusions",
    ));
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
    col.push(
        ctx.collapsible(
            "sel_blanket_rules",
            ids::PANELS_DIRECTIVE_ERC_EXCLUSIONS,
            &ctx.collapsed_sections,
            move || {
                let all = signex_types::schematic::ALL_ERC_RULES;
                let every = excluded.iter().any(|r| r == all);
                let mut rows = Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_check_row(
                        t!(i18n, ids::PANELS_DIRECTIVE_ALL_RULES),
                        every,
                        PanelMsg::EditBlanket(
                            id,
                            BlanketEdit::Rule {
                                rule_id: all.to_string(),
                                excluded: !every,
                            },
                        ),
                        muted,
                        surface,
                    ));
                for kind in signex_erc::RuleKind::ALL {
                    let rule_id = signex_erc::RuleId::builtin(kind).as_str().to_string();
                    let on = excluded.contains(&rule_id);
                    rows = rows.push(form_check_row(
                        // raw: the rule's name comes from signex-erc (#2062).
                        raw(kind.label()),
                        on,
                        PanelMsg::EditBlanket(
                            id,
                            BlanketEdit::Rule {
                                rule_id,
                                excluded: !on,
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
