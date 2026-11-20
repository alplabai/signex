//! The DRC panel before the first run (#87): the "Design rule check"
//! heading with Rules, the full-width Run DRC, the idle hint, the
//! default-clearance notice with its "Set it in Rules" link, and the base
//! rules (DECISIONS 2026-11-17, #2474): each rule's name, its rule text, how
//! to write it, and whether this board writes it, uses the default, or
//! always checks it. The panel keeps the name "DRC" (an open decision on
//! #87, flagged).

use signex_drc_dsl::table::{LimitField, RuleTable};
use signex_i18n::{I18n, MessageId, ids, t};

use super::validation_kit::{
    Tone, banner, hint_card, link, link_ink, notice, run_button, status_pill,
};
use super::*;
use crate::styles::{DialogButtonKind, PillTone, Spacing, TypeScale, dialog_button};
use signex_drc_dsl::table::catalog::Domain;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// One of the 15 base DRC rules every board is checked against (#2474).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseRule {
    Clearance,
    TrackWidth,
    AnnularRing,
    HoleSize,
    HoleToHole,
    ViaInPad,
    SilkToPad,
    SilkToSilk,
    Mask,
    Paste,
    Edge,
    Courtyard,
    Keepout,
    Unrouted,
    Short,
}

impl BaseRule {
    /// In the order the panel lists them.
    pub const ALL: [BaseRule; 15] = [
        BaseRule::Clearance,
        BaseRule::TrackWidth,
        BaseRule::AnnularRing,
        BaseRule::HoleSize,
        BaseRule::HoleToHole,
        BaseRule::ViaInPad,
        BaseRule::SilkToPad,
        BaseRule::SilkToSilk,
        BaseRule::Mask,
        BaseRule::Paste,
        BaseRule::Edge,
        BaseRule::Courtyard,
        BaseRule::Keepout,
        BaseRule::Unrouted,
        BaseRule::Short,
    ];

    /// The limits a written rule sets for it; none for a rule always checked.
    pub fn fields(self) -> &'static [LimitField] {
        use LimitField as F;
        match self {
            BaseRule::Clearance => &[F::MinClearance],
            BaseRule::TrackWidth => &[F::MinWidth, F::MaxWidth],
            BaseRule::AnnularRing => &[F::MinAnnular],
            BaseRule::HoleSize => &[F::MinHole, F::MaxHole],
            BaseRule::HoleToHole => &[F::MinHoleSpacing],
            BaseRule::ViaInPad => &[F::MinPadGap],
            BaseRule::SilkToPad => &[F::MinSilkPad],
            BaseRule::SilkToSilk => &[F::MinSilkSpacing],
            BaseRule::Mask => &[F::MinSliver, F::MinMaskExpansion, F::MaxMaskExpansion],
            BaseRule::Paste => &[F::MinPasteExpansion, F::MaxPasteExpansion],
            BaseRule::Edge => &[F::MinEdgeClearance],
            BaseRule::Courtyard => &[F::MinCourtyardSpacing],
            BaseRule::Keepout | BaseRule::Unrouted | BaseRule::Short => &[],
        }
    }

    /// Checked on every run, with nothing to write.
    pub fn is_always(self) -> bool {
        self.fields().is_empty()
    }

    /// Whether an enabled rule of `table` sets one of its limits.
    pub fn is_written_in(self, table: &RuleTable) -> bool {
        table
            .rows
            .iter()
            .any(|row| row.enabled && self.fields().iter().any(|f| row.limit(*f).is_some()))
    }

    /// The Rules tab domain that holds it: the catalog domain of its first
    /// limit, as `rules_list` names a rule's; Clearance for a rule always
    /// checked (nothing to write).
    pub fn domain(self) -> Domain {
        self.fields()
            .first()
            .and_then(|field| signex_drc_dsl::table::catalog::group_of(*field))
            .map_or(Domain::Clearance, |group| group.domain)
    }

    /// `(name, rule text, how to write it)`.
    fn words(self) -> (MessageId, Option<MessageId>, MessageId) {
        match self {
            BaseRule::Clearance => (
                ids::DRC_BASE_CLEARANCE,
                Some(ids::DRC_BASE_CLEARANCE_TEXT),
                ids::DRC_BASE_CLEARANCE_HOW,
            ),
            BaseRule::TrackWidth => (
                ids::DRC_BASE_TRACK_WIDTH,
                Some(ids::DRC_BASE_TRACK_WIDTH_TEXT),
                ids::DRC_BASE_TRACK_WIDTH_HOW,
            ),
            BaseRule::AnnularRing => (
                ids::DRC_BASE_ANNULAR_RING,
                Some(ids::DRC_BASE_ANNULAR_RING_TEXT),
                ids::DRC_BASE_ANNULAR_RING_HOW,
            ),
            BaseRule::HoleSize => (
                ids::DRC_BASE_HOLE_SIZE,
                Some(ids::DRC_BASE_HOLE_SIZE_TEXT),
                ids::DRC_BASE_HOLE_SIZE_HOW,
            ),
            BaseRule::HoleToHole => (
                ids::DRC_BASE_HOLE_TO_HOLE,
                Some(ids::DRC_BASE_HOLE_TO_HOLE_TEXT),
                ids::DRC_BASE_HOLE_TO_HOLE_HOW,
            ),
            BaseRule::ViaInPad => (
                ids::DRC_BASE_VIA_IN_PAD,
                Some(ids::DRC_BASE_VIA_IN_PAD_TEXT),
                ids::DRC_BASE_VIA_IN_PAD_HOW,
            ),
            BaseRule::SilkToPad => (
                ids::DRC_BASE_SILK_TO_PAD,
                Some(ids::DRC_BASE_SILK_TO_PAD_TEXT),
                ids::DRC_BASE_SILK_TO_PAD_HOW,
            ),
            BaseRule::SilkToSilk => (
                ids::DRC_BASE_SILK_TO_SILK,
                Some(ids::DRC_BASE_SILK_TO_SILK_TEXT),
                ids::DRC_BASE_SILK_TO_SILK_HOW,
            ),
            BaseRule::Mask => (
                ids::DRC_BASE_MASK,
                Some(ids::DRC_BASE_MASK_TEXT),
                ids::DRC_BASE_MASK_HOW,
            ),
            BaseRule::Paste => (
                ids::DRC_BASE_PASTE,
                Some(ids::DRC_BASE_PASTE_TEXT),
                ids::DRC_BASE_PASTE_HOW,
            ),
            BaseRule::Edge => (
                ids::DRC_BASE_EDGE,
                Some(ids::DRC_BASE_EDGE_TEXT),
                ids::DRC_BASE_EDGE_HOW,
            ),
            BaseRule::Courtyard => (
                ids::DRC_BASE_COURTYARD,
                Some(ids::DRC_BASE_COURTYARD_TEXT),
                ids::DRC_BASE_COURTYARD_HOW,
            ),
            BaseRule::Keepout => (
                ids::DRC_BASE_KEEPOUT,
                Some(ids::DRC_BASE_KEEPOUT_TEXT),
                ids::DRC_BASE_KEEPOUT_HOW,
            ),
            BaseRule::Unrouted => (ids::DRC_BASE_UNROUTED, None, ids::DRC_BASE_UNROUTED_HOW),
            BaseRule::Short => (ids::DRC_BASE_SHORT, None, ids::DRC_BASE_SHORT_HOW),
        }
    }
}

/// How the board stands on a base rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BaseRuleState {
    Written,
    Default,
    Always,
}

pub(super) fn base_rule_state(drc: &DrcPanel, rule: BaseRule) -> BaseRuleState {
    if rule.is_always() {
        BaseRuleState::Always
    } else if drc.written_base_rules.contains(&rule) {
        BaseRuleState::Written
    } else {
        BaseRuleState::Default
    }
}

fn drc_msg(msg: super::DrcPanelMsg) -> PanelMsg {
    PanelMsg::Drc(msg)
}

/// The small Rules button: the project's rules as a document tab (#2398).
pub(super) fn rules_button<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    dialog_button(
        t!(ctx.i18n, ids::DRC_PANEL_RULES),
        DialogButtonKind::Secondary,
        Some(PanelMsg::OpenRules),
        &ctx.tokens,
    )
}

/// The default-clearance notice with "Set it in Rules", which opens the
/// Rules tab on the Clearance domain.
pub(super) fn default_clearance<'a>(
    ctx: &'a PanelContext,
    drc: &DrcPanel,
) -> Option<Element<'a, PanelMsg>> {
    let mm = drc.default_clearance_mm.as_ref()?;
    let i18n = &*ctx.i18n;
    let id = if drc.written_clearance {
        ids::DRC_PANEL_DEFAULT_CLEARANCE_WRITTEN
    } else {
        ids::DRC_PANEL_DEFAULT_CLEARANCE
    };
    let words = t!(i18n, id, "clearance" => mm.clone());
    let set = link(
        t!(i18n, ids::DRC_PANEL_SET_IN_RULES),
        drc_msg(super::DrcPanelMsg::BaseRuleOpened(BaseRule::Clearance)),
        link_ink(&ctx.tokens),
    );
    Some(banner(
        Tone::Info,
        notice(words, Some(set), &ctx.tokens),
        None,
        &ctx.tokens,
    ))
}

/// The heading, Run DRC, the idle hint and the default-clearance notice.
pub(super) fn idle_top<'a>(ctx: &'a PanelContext, drc: &DrcPanel) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let heading = row![
        text(t!(i18n, ids::DRC_HEADING))
            .size(TS.label)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
            })
            .color(theme_ext::text_primary(tokens)),
        Space::new().width(Length::Fill).height(Length::Shrink),
        rules_button(ctx),
    ]
    .align_y(iced::Alignment::Center);
    let (label, on_press) = if drc.running {
        (t!(i18n, ids::DRC_RUNNING), None)
    } else {
        (t!(i18n, ids::DRC_RUN), Some(PanelMsg::RunDrc))
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.s)
        .push(heading.boxed())
        .push(run_button(label, "", on_press, tokens));
    if !drc.running {
        col = col.push(hint_card(t!(i18n, ids::DRC_IDLE), tokens));
    }
    if let Some(note) = default_clearance(ctx, drc) {
        col = col.push(note);
    }
    col.boxed()
}

/// One base rule: name, rule text in mono, how to write it (with "Write
/// it" while it is not written), and its state pill.
fn base_rule<'a>(ctx: &'a PanelContext, drc: &DrcPanel, rule: BaseRule) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let r = crate::styles::tokens::resolve(tokens);
    let (name, rule_text, how) = rule.words();
    let state = base_rule_state(drc, rule);
    let mut lines: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs / 2.0).width(Length::Fill).push(
            text(t!(i18n, name))
                .size(TS.body)
                .color(theme_ext::text_primary(tokens))
                .boxed(),
        );
    if let Some(rule_text) = rule_text {
        lines = lines.push(
            text(t!(i18n, rule_text))
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(r.ink4)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .boxed(),
        );
    }
    let how = if rule == BaseRule::Clearance {
        let mm = drc
            .default_clearance_mm
            .clone()
            .unwrap_or_else(|| format!("{:.1}", signex_drc::DEFAULT_CLEARANCE_MM));
        t!(i18n, how, "clearance" => mm)
    } else {
        t!(i18n, how)
    };
    let mut how_line = row![text(how).size(TS.caption).color(r.ink3)].spacing(SP.xs);
    if state == BaseRuleState::Default {
        how_line = how_line.push(link(
            t!(i18n, ids::DRC_BASE_WRITE_IT),
            drc_msg(super::DrcPanelMsg::BaseRuleOpened(rule)),
            link_ink(tokens),
        ));
    }
    lines = lines.push(how_line.wrap().vertical_spacing(SP.xs).boxed());
    let (pill_words, tone) = match state {
        BaseRuleState::Written => (ids::DRC_BASE_STATE_WRITTEN, PillTone::Ok),
        BaseRuleState::Default => (ids::DRC_BASE_STATE_DEFAULT, PillTone::Warn),
        BaseRuleState::Always => (ids::DRC_BASE_STATE_ALWAYS, PillTone::Neutral),
    };
    row![lines, status_pill(t!(i18n, pill_words), tone, tokens)]
        .spacing(SP.s)
        .padding([SP.xs, SP.m])
        .align_y(iced::Alignment::Start)
        .boxed()
}

/// "The base rules" with their count, folding, then each rule while open.
pub(super) fn base_rules<'a>(ctx: &'a PanelContext, drc: &DrcPanel) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let open = !drc.view.base_rules_closed;
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new()
            .width(Length::Fill)
            .push(super::validation_kit::group_header(
                super::validation_kit::GroupHeader {
                    open,
                    badge: None,
                    title: t!(i18n, ids::DRC_BASE_RULES).into(),
                    title_ink: theme_ext::text_primary(tokens),
                    count: Some(BaseRule::ALL.len()),
                    trailing: None,
                    on_toggle: drc_msg(super::DrcPanelMsg::BaseRulesToggled),
                },
                i18n,
                tokens,
            ));
    if open {
        for rule in BaseRule::ALL {
            col = col.push(base_rule(ctx, drc, rule));
        }
    }
    col.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(src: &str) -> Vec<BaseRule> {
        let table = signex_drc_dsl::table::table(src);
        BaseRule::ALL
            .into_iter()
            .filter(|r| r.is_written_in(&table))
            .collect()
    }

    /// #2474: a base rule is written when an enabled rule sets one of its
    /// limits; keepout, unrouted net and short are always checked.
    #[test]
    fn a_base_rule_is_written_by_an_enabled_rule_that_sets_its_limit() {
        assert!(written("").is_empty());
        let src = "rule Default on clearance require clearance >= 0.15mm then error\n\
                   rule Tracks on track require width >= 0.15mm and width <= 1mm then error";
        assert_eq!(written(src), [BaseRule::Clearance, BaseRule::TrackWidth]);
        let off = "disabled rule Default on clearance require clearance >= 0.15mm then error";
        assert!(written(off).is_empty(), "a disabled rule writes nothing");
        for rule in [BaseRule::Keepout, BaseRule::Unrouted, BaseRule::Short] {
            assert!(rule.is_always());
        }
        assert_eq!(BaseRule::ALL.iter().filter(|r| r.is_always()).count(), 3);
    }

    #[test]
    fn the_idle_list_shows_each_rule_with_its_state() {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.pcb_drc = Some(DrcPanel {
            written_base_rules: vec![BaseRule::TrackWidth],
            default_clearance_mm: Some("0.2".into()),
            ..DrcPanel::default()
        });
        let shown: Vec<String> = crate::layout_probe::lay_out_texts(
            super::super::drc_results::view(&ctx),
            iced::Size::new(300.0, 4000.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
        assert!(shown.iter().any(|t| t == "Design rule check"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "The base rules"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "15"), "{shown:?}");
        assert!(
            shown
                .iter()
                .any(|t| t == "on clearance require clearance >= …"),
            "{shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|t| t
                    == "Replace … with a length, such as 0.15mm. Until then DRC checks 0.2mm."),
            "{shown:?}"
        );
        assert_eq!(shown.iter().filter(|t| *t == "written").count(), 1);
        assert_eq!(shown.iter().filter(|t| *t == "always").count(), 3);
        assert_eq!(shown.iter().filter(|t| *t == "default").count(), 11);
        assert!(
            shown
                .iter()
                .any(|t| t == "No clearance rule set on this board; checked with 0.2 mm."),
            "{shown:?}"
        );
    }
}
