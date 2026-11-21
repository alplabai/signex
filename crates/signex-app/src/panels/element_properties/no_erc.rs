//! Properties of a selected No-ERC directive (#1699, mockup row 49): its Location
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
    col.push(suppressed_rules_section(ctx, rule_list(ctx, id), st))
}

/// The rule list of the selected No-ERC directive `id`, as its
/// `selection_info` row stores it.
fn rule_list(ctx: &PanelContext, id: uuid::Uuid) -> RuleList<'static> {
    let listed: Vec<String> = signex_types::schematic::parse_erc_exclusions(&info_row(
        &ctx.selection_info,
        signex_engine::info_keys::NO_ERC_RULES,
    ));
    let all_on = ctx.selection_form.no_erc_all_rules_on(id, &listed);
    RuleList {
        key: "sel_no_erc_rules",
        all_on,
        all_toggled: all_rules_toggled(id, all_on, !listed.is_empty()),
        ticked: Box::new(move |rule: &str| listed.iter().any(|r| r == rule)),
        rule_toggled: Box::new(move |rule_id: String, silenced: bool| {
            PanelMsg::EditNoErcRules(id, NoErcRuleEdit::Rule { rule_id, silenced })
        }),
    }
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

    const WINDOW: iced::Size = iced::Size::new(400.0, 4000.0);
    const FOOTER: &str = "1 object selected";

    fn view_of(ctx: &PanelContext) -> Element<'_, PanelMsg> {
        super::super::view_selected_element_properties(
            ctx,
            Color::WHITE,
            Color::WHITE,
            Color::BLACK,
        )
    }

    fn laid_out(ctx: &PanelContext) -> Vec<crate::layout_probe::LaidOutText> {
        crate::layout_probe::lay_out_texts(view_of(ctx), WINDOW)
    }

    const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
    const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

    /// The checkbox of the row named `label`: the box comes first, one
    /// row gap before the name.
    fn rule_box(texts: &[crate::layout_probe::LaidOutText], label: &str) -> iced::Point {
        let at = crate::layout_probe::find(texts, label).bounds;
        iced::Point::new(at.x - SP.s - TS.body / 2.0, at.center_y())
    }

    /// The longest built-in rule name.
    fn longest_rule_name() -> &'static str {
        signex_erc::RuleKind::ALL
            .into_iter()
            .map(|kind| kind.label())
            .max_by_key(|label| label.len())
            .unwrap_or(UNUSED_PIN)
    }

    /// The maintainer's report (2026-11-21): rule names wrapped onto two or
    /// three lines in a narrow label column. At the default right dock
    /// width the longest name keeps one line, starts right after its box
    /// and runs across the row.
    #[test]
    fn a_long_rule_name_stays_on_one_row_at_the_default_dock_width() {
        let width = crate::dock::DEFAULT_RIGHT_DOCK_WIDTH;
        let (ctx, _) = view_with("", true);
        let texts =
            crate::layout_probe::lay_out_texts(view_of(&ctx), iced::Size::new(width, 4000.0));
        let name = longest_rule_name();
        let at = crate::layout_probe::find(&texts, name).bounds;
        let one_line =
            crate::layout_probe::natural::<PanelMsg>(text(raw(name)).size(TS.body).into(), name);
        assert!(
            at.height <= one_line.height + 1.0,
            "{name:?} wraps: {at:?} vs one line {one_line:?}"
        );
        assert!(
            at.x < crate::panels::PROPERTY_LABEL_WIDTH,
            "the name follows its box, not a label column: {at:?}"
        );
        assert!(
            at.x + at.width > width * 0.8,
            "the name runs across the row: {at:?}"
        );
    }

    /// With All rules on, every row reads ticked and every rule is
    /// silenced by the directive.
    #[test]
    fn all_rules_on_shows_every_rule_checked_and_suppressed() {
        let (ctx, id) = view_with("", false);
        let list = rule_list(&ctx, id);
        assert!(list.all_on);
        let rows = super::super::directive::rule_rows(&ctx.i18n, &list, "");
        assert_eq!(rows.len(), signex_erc::RuleKind::ALL.len());
        for row in rows {
            assert!(row.checked, "{} shows unticked", row.name);
            assert!(
                signex_types::schematic::no_erc_rules_suppress(&[], &row.id),
                "{} is not silenced",
                row.id
            );
        }
    }

    /// The filter narrows the rows to the names it matches.
    #[test]
    fn the_filter_narrows_the_rows() {
        let (mut ctx, id) = view_with("", true);
        ctx.selection_form.rule_query = "bus".into();
        let list = rule_list(&ctx, id);
        let rows = super::super::directive::rule_rows(&ctx.i18n, &list, "bus");
        assert!(!rows.is_empty());
        assert!(rows.len() < signex_erc::RuleKind::ALL.len());
        assert!(
            rows.iter()
                .all(|row| row.name.to_lowercase().contains("bus")),
            "{rows:?}"
        );
        let texts = laid_out(&ctx);
        assert!(texts.iter().any(|t| t.text == rows[0].name));
        assert!(!texts.iter().any(|t| t.text == UNUSED_PIN), "{texts:#?}");
    }

    /// A click on a rule's name toggles it, as its box does.
    #[test]
    fn a_click_on_a_rule_name_ticks_it() {
        let (ctx, id) = view_with("", true);
        let name = crate::layout_probe::find(&laid_out(&ctx), UNUSED_PIN).bounds;
        let messages = clicked_at(&ctx, name.center());
        assert!(
            messages.iter().any(|m| matches!(
                m,
                PanelMsg::EditNoErcRules(at, NoErcRuleEdit::Rule { silenced: true, .. }) if *at == id
            )),
            "{messages:?}"
        );
    }

    fn clicked_at(ctx: &PanelContext, at: iced::Point) -> Vec<PanelMsg> {
        crate::layout_probe::messages_on_click_at(view_of(ctx), WINDOW, None, at)
    }

    fn rule_label() -> String {
        signex_erc::RuleKind::ALL
            .into_iter()
            .map(|kind| kind.label())
            .find(|label| *label == UNUSED_PIN)
            .unwrap_or_else(|| signex_erc::RuleKind::ALL[0].label())
            .to_string()
    }

    /// Mockup row 49: while All rules is on, the per-rule boxes take no
    /// click. The veiled list reports no text to the probe, so the box is
    /// found in the open list; the All rules caption sits under the list
    /// (it moves the footer down), so the rows keep their place.
    #[test]
    fn all_rules_on_disables_the_per_rule_boxes() {
        let (ctx, _) = view_with("", false);
        assert!(
            ctx.selection_form
                .no_erc_all_rules_on(ctx.selected_uuid.unwrap(), &[])
        );
        let (open, _) = view_with("", true);
        let open_texts = laid_out(&open);
        let on_texts = laid_out(&ctx);
        let caption = crate::layout_probe::find(&on_texts, FOOTER).bounds.y
            - crate::layout_probe::find(&open_texts, FOOTER).bounds.y;
        assert!(caption > 0.0, "the All rules caption shows only while on");
        let veiled_box = rule_box(&open_texts, &rule_label());
        let messages = clicked_at(&ctx, veiled_box);
        assert!(messages.is_empty(), "{messages:?}");
    }

    /// Once the list is open, a rule box ticks that rule.
    #[test]
    fn an_open_list_ticks_a_rule() {
        let (ctx, id) = view_with("", true);
        let messages = clicked_at(&ctx, rule_box(&laid_out(&ctx), &rule_label()));
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
