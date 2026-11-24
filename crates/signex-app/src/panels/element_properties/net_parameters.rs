//! Properties of a selected Net Parameters directive (#1489): the net it
//! sits on, a line-art illustration of the flag (its Width, Clearance and
//! Via linked to the rule rows), the net class and diff-pair class, the
//! rule list and whether the flag shows the values. Each edit is one
//! `UpdateDirective`: a typed value continues a typing run, a pick or a
//! checkbox is a step of its own. Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::directive::{
    DirectivePanel, DirectivePanelMsg, DraftField, NetParametersEdit, NetRule, caption_row,
    class_picker, control_row, format_value, illustration_view, linked_row, msg, unit, value_input,
};
use super::rows::PanelStyle;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::{Point as P, Rectangle as R, Size};
use signex_i18n::{ids, t};
use signex_types::schematic::{NetRules, SchDirective};
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Ink, Paint, Part, Primitive, Style, TextLabel, TextRole, Tint,
    Weight,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The fields the illustration's parts link to.
pub(super) const FIELD_WIDTH: FieldId = FieldId::new("np_width");
pub(super) const FIELD_CLEARANCE: FieldId = FieldId::new("np_clearance");
pub(super) const FIELD_VIA: FieldId = FieldId::new("np_via");
pub(super) const FIELD_CURRENT: FieldId = FieldId::new("np_current");

/// The width of a value box, as the mockup's rule list draws them.
const VALUE_W: f32 = 96.0;
const VIA_DIA_W: f32 = 52.0;
const VIA_HOLE_W: f32 = 70.0;

pub(super) fn push_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    panel: &'a DirectivePanel,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let SchDirective::NetParameters {
        rules,
        show_on_flag,
        ..
    } = &panel.directive
    else {
        return col;
    };
    let id = panel.directive.uuid();
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let muted = st.muted;
    let caption = match &panel.net {
        Some(net) => {
            t!(i18n, ids::PANELS_NP_CAPTION, "net" => net.as_str(), "sheet" => panel.sheet.as_str())
        }
        None => t!(i18n, ids::PANELS_NP_CAPTION_NO_NET, "sheet" => panel.sheet.as_str()),
    };
    col = col.push(caption_row(caption, ctx));
    col = col.push(illustration_view(illustration(panel, i18n), panel, ctx));

    let edit = move |e: NetParametersEdit| msg(DirectivePanelMsg::NetParametersEdited(id, e));
    let class = panel.directive.net_class();
    let pair_class = panel.directive.diff_pair_class();
    col = col.push(
        ctx.collapsible(
            "sel_np_classes",
            ids::PANELS_NP_CLASSES,
            &ctx.collapsed_sections,
            move || {
                let net_class = class_picker(
                    i18n,
                    ids::PANELS_NP_NONE,
                    &panel.net_classes,
                    class,
                    move |c| edit(NetParametersEdit::NetClass(c)),
                    true,
                );
                // A net that is not half of a pair takes no diff-pair class:
                // the pick is greyed and reads "None (single net)".
                let pair = class_picker(
                    i18n,
                    ids::PANELS_NP_SINGLE_NET,
                    &panel.diff_pair_classes,
                    pair_class,
                    move |c| edit(NetParametersEdit::DiffPairClass(c)),
                    panel.in_pair,
                );
                Column::new()
                    .push(control_row(
                        t!(i18n, ids::PANELS_DIRECTIVE_NET_CLASS),
                        net_class,
                        muted,
                    ))
                    .push(control_row(
                        t!(i18n, ids::PANELS_NP_DIFF_PAIR_CLASS),
                        pair,
                        muted,
                    ))
            },
        )
        .boxed(),
    );

    col = col.push(
        ctx.collapsible(
            "sel_np_rules",
            ids::PANELS_NP_RULES,
            &ctx.collapsed_sections,
            move || rule_list(ctx, panel, rules, muted),
        )
        .boxed(),
    );
    let scope = class
        .map(str::to_string)
        .or_else(|| panel.net.clone())
        .unwrap_or_default();
    if !scope.is_empty() {
        col = col.push(caption_row(
            t!(i18n, ids::PANELS_NP_BECOMES_RULES, "class" => scope.as_str()),
            ctx,
        ));
    }

    let shown = *show_on_flag;
    let surface = st.palette.panel_bg;
    col.push(
        ctx.collapsible(
            "sel_np_display",
            ids::PANELS_NP_DISPLAY,
            &ctx.collapsed_sections,
            move || {
                Column::new().push(form_check_row(
                    t!(i18n, ids::PANELS_NP_SHOW_ON_FLAG),
                    shown,
                    edit(NetParametersEdit::ShowOnFlag(!shown)),
                    muted,
                    surface,
                ))
            },
        )
        .boxed(),
    )
}

/// The Rule | Value list: Width, Clearance, Via dia / hole (two boxes) and
/// Current (information only).
fn rule_list<'a>(
    ctx: &'a PanelContext,
    panel: &'a DirectivePanel,
    rules: &NetRules,
    muted: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let id = panel.directive.uuid();
    let typed = move |rule: NetRule| {
        move |text: String| {
            msg(DirectivePanelMsg::NetParametersEdited(
                id,
                NetParametersEdit::Rule(rule, text),
            ))
        }
    };
    let mm = || unit(t!(i18n, ids::PANELS_NP_UNIT_MM), muted);
    let boxed = |rule: NetRule, value: Option<f64>, width: f32| {
        value_input(
            panel.shown(DraftField::Rule(rule), value),
            typed(rule),
            width,
            panel.refused_ink(DraftField::Rule(rule), ctx),
        )
    };
    let head = iced::widget::row![
        text(t!(i18n, ids::PANELS_NP_RULE))
            .size(TS.caption)
            .color(muted),
        Space::new().width(Length::Fill),
        text(t!(i18n, ids::PANELS_NP_VALUE))
            .size(TS.caption)
            .color(muted),
    ]
    .padding([SP.xs, SP.m])
    .boxed();
    let line = |label: Element<'a, PanelMsg>, controls: Vec<Element<'a, PanelMsg>>| {
        let mut r = iced::widget::Row::new()
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .padding([SP.xs, SP.m])
            .push(label)
            .push(Space::new().width(Length::Fill).boxed());
        for c in controls {
            r = r.push(c);
        }
        r.boxed()
    };
    let label = |id: signex_i18n::MessageId| text(t!(i18n, id)).size(TS.body).boxed();
    let current_label = iced::widget::row![
        text(t!(i18n, ids::PANELS_NP_CURRENT)).size(TS.body),
        text(t!(i18n, ids::PANELS_NP_INFO))
            .size(TS.caption)
            .color(muted),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed();
    Column::new()
        .push(head)
        .push(linked_row(
            line(
                label(ids::PANELS_NP_WIDTH),
                vec![boxed(NetRule::Width, rules.width_mm, VALUE_W), mm()],
            ),
            FIELD_WIDTH,
            panel,
            ctx,
        ))
        .push(linked_row(
            line(
                label(ids::PANELS_NP_CLEARANCE),
                vec![boxed(NetRule::Clearance, rules.clearance_mm, VALUE_W), mm()],
            ),
            FIELD_CLEARANCE,
            panel,
            ctx,
        ))
        .push(linked_row(
            line(
                label(ids::PANELS_NP_VIA),
                vec![
                    boxed(NetRule::ViaDiameter, rules.via_diameter_mm, VIA_DIA_W),
                    boxed(NetRule::ViaHole, rules.via_hole_mm, VIA_HOLE_W),
                    mm(),
                ],
            ),
            FIELD_VIA,
            panel,
            ctx,
        ))
        .push(linked_row(
            line(
                current_label,
                vec![
                    boxed(NetRule::Current, rules.current_a, VALUE_W),
                    unit(t!(i18n, ids::PANELS_NP_UNIT_A), muted),
                ],
            ),
            FIELD_CURRENT,
            panel,
            ctx,
        ))
}

/// A value as the flag writes it: `0.8`, or `—` when unset.
fn flag_value(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_string(), format_value)
}

/// Rough width of `chars` characters of illustration caption text, in view
/// units; the flag is sized to its text, never narrower.
fn text_width(chars: usize) -> f32 {
    chars as f32 * 6.2 + 16.0
}

/// The flag on its wire: the net name on the wire, the flag with its class
/// line and, with "Show parameters on the flag", the rule values, each
/// linked to its row. Current sits below as information.
pub(super) fn illustration(panel: &DirectivePanel, i18n: &signex_i18n::I18n) -> Illustration {
    let SchDirective::NetParameters {
        rules,
        show_on_flag,
        ..
    } = &panel.directive
    else {
        return Illustration::new(1.0, 1.0);
    };
    const W: f32 = 280.0;
    const H: f32 = 150.0;
    let yw = H - 30.0;
    let xp = (W * 0.22).round();
    let class_line = match panel.directive.net_class() {
        Some(class) => t!(i18n, ids::PANELS_NP_FLAG_CLASS, "class" => class),
        None => t!(i18n, ids::PANELS_NP_FLAG_NO_CLASS),
    };
    let via = format!(
        "{} / {}",
        flag_value(rules.via_diameter_mm),
        flag_value(rules.via_hole_mm)
    );
    let values = [
        (
            ids::PANELS_NP_WIDTH,
            flag_value(rules.width_mm),
            FIELD_WIDTH,
        ),
        (
            ids::PANELS_NP_CLEARANCE,
            flag_value(rules.clearance_mm),
            FIELD_CLEARANCE,
        ),
        (ids::PANELS_NP_VIA_SHORT, via, FIELD_VIA),
    ];
    let longest = values
        .iter()
        .map(|(_, v, _)| v.chars().count() + 12)
        .chain([class_line.to_string().chars().count()])
        .max()
        .unwrap_or(0);
    let fw = text_width(longest).min(W - xp - 8.0);
    let fh = if *show_on_flag { 70.0 } else { 26.0 };
    let fy = (yw - 28.0 - fh).max(4.0);
    let accent = Style::outline(Ink::Accent, Weight::Regular);
    let mut ill = Illustration::new(W, H)
        .part(Part::new(
            Primitive::Line {
                from: P::new(10.0, yw),
                to: P::new(W - 10.0, yw),
            },
            Style::outline(Ink::Secondary, Weight::Regular),
        ))
        .part(Part::label(
            TextLabel {
                at: P::new(14.0, yw - 6.0),
                text: signex_i18n::UserText::new(panel.net.clone().unwrap_or_default()).into(),
                anchor: Anchor::Start,
                role: TextRole::Mono,
            },
            Ink::Secondary,
        ))
        .part(Part::new(
            Primitive::Line {
                from: P::new(xp, yw),
                to: P::new(xp, fy + fh),
            },
            accent,
        ))
        .part(Part::new(
            Primitive::Circle {
                center: P::new(xp, yw),
                radius: 2.5,
            },
            Style::filled(Paint::Ink(Ink::Accent), Tint::Solid),
        ))
        .part(Part::new(
            Primitive::RoundedRect {
                rect: R::new(P::new(xp, fy), Size::new(fw, fh)),
                radius: 3.0,
            },
            accent.with_fill(Paint::Ink(Ink::Accent), Tint::Wash),
        ))
        .part(Part::label(
            TextLabel {
                at: P::new(xp + 8.0, fy + 17.0),
                text: class_line.into(),
                anchor: Anchor::Start,
                role: TextRole::Caption,
            },
            Ink::Accent,
        ));
    if *show_on_flag {
        for (i, (label, value, field)) in values.into_iter().enumerate() {
            let y = fy + 32.0 + i as f32 * 14.0;
            ill = ill
                .part(
                    Part::label(
                        TextLabel {
                            at: P::new(xp + 8.0, y),
                            text: t!(i18n, label).into(),
                            anchor: Anchor::Start,
                            role: TextRole::Note,
                        },
                        Ink::Faint,
                    )
                    .linked(field.clone()),
                )
                .part(
                    Part::label(
                        TextLabel {
                            at: P::new(xp + 74.0, y),
                            text: signex_i18n::UserText::new(value).into(),
                            anchor: Anchor::Start,
                            role: TextRole::Mono,
                        },
                        Ink::Secondary,
                    )
                    .linked(field),
                );
        }
    } else {
        ill = ill.part(Part::label(
            TextLabel {
                at: P::new(xp + 8.0, fy + fh + 14.0),
                text: t!(i18n, ids::PANELS_NP_FLAG_HIDDEN).into(),
                anchor: Anchor::Start,
                role: TextRole::Note,
            },
            Ink::Faint,
        ));
    }
    let current = flag_value(rules.current_a);
    ill.part(
        Part::label(
            TextLabel {
                at: P::new(12.0, H - 10.0),
                text: t!(i18n, ids::PANELS_NP_CURRENT_INFO, "current" => current).into(),
                anchor: Anchor::Start,
                role: TextRole::Note,
            },
            Ink::Faint,
        )
        .linked(FIELD_CURRENT),
    )
}
