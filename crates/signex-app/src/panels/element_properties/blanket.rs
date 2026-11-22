//! Properties of a selected Net Blanket (#1281): a line-art illustration of
//! the outline over the wires around it (caught nets in accent, Width and
//! Height linked to the Outline rows), the Outline (X / Y, Width / Height),
//! the Directive (net class, track width, how pours treat the nets) and the
//! nets under the blanket; hovering a net row lights that net on the sheet.
//! The ERC exclusions follow in `directive.rs`. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`.

use super::super::*;
use super::directive::{
    BlanketCatch, BlanketEdit, DirectivePanel, DirectivePanelMsg, DraftField, OutlineField,
    caption_row, class_picker, control_row, illustration_view, linked_row, msg, unit, value_input,
};
use super::rows::PanelStyle;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::{Point as P, Rectangle as R, Size};
use signex_i18n::{ids, t};
use signex_types::schematic::{BlanketPour, SchDirective};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Fill, Illustration, Ink, Paint, Part, Primitive, Stroke, Style, Tint,
    Weight,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The Outline's Width / Height row and the two size callouts.
pub(super) const FIELD_SIZE: FieldId = FieldId::new("blanket_size");

/// The width of one of two value boxes on a row.
const HALF_W: f32 = 84.0;
const VALUE_W: f32 = 96.0;

pub(super) fn push_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    panel: &'a DirectivePanel,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let SchDirective::Blanket {
        start,
        end,
        track_width,
        pour,
        include_edge_nets,
        ..
    } = &panel.directive
    else {
        return col;
    };
    let Some(region) = panel.directive.region() else {
        return col;
    };
    let id = panel.directive.uuid();
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let muted = st.muted;
    let edit = move |e: BlanketEdit| PanelMsg::EditBlanket(id, e);
    let (origin, paper_height) = panel.page;
    let (w, h) = ((end.x - start.x).abs(), (end.y - start.y).abs());

    col = col.push(caption_row(
        t!(
            i18n,
            ids::PANELS_BLANKET_CAPTION,
            "sheet" => panel.sheet.as_str(),
            "width" => format!("{w:.2}"),
            "height" => format!("{h:.2}")
        ),
        ctx,
    ));
    col = col.push(illustration_view(
        illustration(panel, region, i18n),
        panel,
        ctx,
    ));

    // ── Outline: X / Y of the start corner, Width / Height from it ──
    let typed =
        move |field: OutlineField| move |text: String| edit(BlanketEdit::Outline(field, text));
    let pair = |a: Element<'a, PanelMsg>, b: Element<'a, PanelMsg>| {
        iced::widget::row![a, b, unit(t!(i18n, ids::PANELS_NP_UNIT_MM), muted)]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed()
    };
    let field = |f: OutlineField, value: f64| {
        value_input(
            panel.shown(DraftField::Outline(f), Some(value)),
            typed(f),
            HALF_W,
            panel.refused_ink(DraftField::Outline(f), ctx),
        )
    };
    let (x, y) = (start.x, origin.map_y(start.y, paper_height));
    col = col.push(
        ctx.collapsible(
            "sel_blanket_outline",
            ids::PANELS_BLANKET_OUTLINE,
            &ctx.collapsed_sections,
            move || {
                let xy = pair(field(OutlineField::X, x), field(OutlineField::Y, y));
                let size = pair(
                    field(OutlineField::Width, w),
                    field(OutlineField::Height, h),
                );
                Column::new()
                    .push(control_row(t!(i18n, ids::PANELS_BLANKET_XY), xy, muted))
                    .push(linked_row(
                        control_row(t!(i18n, ids::PANELS_BLANKET_SIZE), size, muted),
                        FIELD_SIZE,
                        panel,
                        ctx,
                    ))
            },
        )
        .boxed(),
    );

    // ── Directive: net class, track width, pours ──
    let class = panel.directive.net_class();
    let track = *track_width;
    let pour = *pour;
    col = col.push(
        ctx.collapsible(
            "sel_blanket_directive",
            ids::PANELS_BLANKET_DIRECTIVE,
            &ctx.collapsed_sections,
            move || {
                let picker = class_picker(
                    i18n,
                    ids::PANELS_NP_NONE,
                    &panel.net_classes,
                    class,
                    move |c| edit(BlanketEdit::NetClass(c.unwrap_or_default())),
                    true,
                );
                let width = iced::widget::row![
                    value_input(
                        panel.shown(DraftField::TrackWidth, track),
                        move |text| edit(BlanketEdit::TrackWidth(text)),
                        VALUE_W,
                        panel.refused_ink(DraftField::TrackWidth, ctx),
                    ),
                    unit(t!(i18n, ids::PANELS_NP_UNIT_MM), muted),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed();
                Column::new()
                    .push(control_row(
                        t!(i18n, ids::PANELS_DIRECTIVE_NET_CLASS),
                        picker,
                        muted,
                    ))
                    .push(control_row(
                        t!(i18n, ids::PANELS_BLANKET_TRACK_WIDTH),
                        width,
                        muted,
                    ))
                    .push(control_row(
                        t!(i18n, ids::PANELS_BLANKET_POUR),
                        pour_switch(ctx, pour, move |p| edit(BlanketEdit::Pour(p))),
                        muted,
                    ))
            },
        )
        .boxed(),
    );

    // ── Nets under the blanket ──
    let edge = *include_edge_nets;
    let surface = st.palette.panel_bg;
    let title = signex_i18n::Translatable::new(ids::PANELS_BLANKET_NETS)
        .arg("count", panel.caught_count() as u32);
    col = col.push(
        ctx.collapsible(
            "sel_blanket_nets",
            title,
            &ctx.collapsed_sections,
            move || {
                let mut rows = Column::new().spacing(0).width(Length::Fill);
                if panel.nets.is_empty() {
                    rows = rows.push(caption_row(t!(i18n, ids::PANELS_BLANKET_NO_NETS), ctx));
                }
                for net in &panel.nets {
                    rows = rows.push(net_row(ctx, panel, net, muted));
                }
                rows.push(form_check_row(
                    t!(i18n, ids::PANELS_BLANKET_INCLUDE_EDGE),
                    edge,
                    edit(BlanketEdit::IncludeEdgeNets(!edge)),
                    muted,
                    surface,
                ))
            },
        )
        .boxed(),
    );
    col.push(caption_row(t!(i18n, ids::PANELS_BLANKET_HOVER_HINT), ctx))
}

/// Normal / Keep short, a segmented control.
fn pour_switch<'a>(
    ctx: &PanelContext,
    pour: BlanketPour,
    on_pick: impl Fn(BlanketPour) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let r = crate::styles::tokens::resolve(&ctx.tokens);
    let style = r
        .segmented(TS.body)
        .resting_ink(crate::styles::ti(ctx.tokens.text_secondary));
    let i18n = &ctx.i18n;
    signex_widgets::segmented::segmented(
        [
            (
                BlanketPour::Normal,
                signex_widgets::Label::from(t!(i18n, ids::PANELS_BLANKET_POUR_NORMAL)),
            ),
            (
                BlanketPour::KeepShort,
                signex_widgets::Label::from(t!(i18n, ids::PANELS_BLANKET_POUR_KEEP_SHORT)),
            ),
        ],
        pour,
        style,
        on_pick,
    )
}

/// One net row: the name, then its pins when caught or a tag when it only
/// crosses the edge. Hovering it lights the net on the sheet.
fn net_row<'a>(
    ctx: &'a PanelContext,
    panel: &'a DirectivePanel,
    net: &'a super::directive::BlanketNetRow,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let r = crate::styles::tokens::resolve(&ctx.tokens);
    let detail: Element<'a, PanelMsg> = match net.catch {
        BlanketCatch::Inside => text(signex_i18n::UserText::new(net.members.clone()))
            .size(TS.caption)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .boxed(),
        BlanketCatch::EdgeOnly => pill(
            t!(i18n, ids::PANELS_BLANKET_EDGE_ONLY).into(),
            r.warn_soft,
            muted,
        ),
        BlanketCatch::EdgeIncluded => pill(
            t!(i18n, ids::PANELS_BLANKET_EDGE_INCLUDED).into(),
            r.ok_soft,
            muted,
        ),
    };
    let ink = if net.catch.is_caught() {
        crate::styles::ti(ctx.tokens.text)
    } else {
        muted
    };
    let lit = panel.hovered_net.as_deref() == Some(net.name.as_str());
    let tint = r.accent_soft;
    let row = iced::widget::row![
        text(signex_i18n::UserText::new(net.name.clone()))
            .size(TS.body)
            .color(ink)
            .font(iced::Font::MONOSPACE),
        Space::new().width(Length::Fill),
        detail,
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .padding([SP.xs, SP.m]);
    let framed = container(row)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: lit.then_some(Background::Color(tint)),
            ..container::Style::default()
        });
    iced::widget::mouse_area(framed)
        .on_enter(msg(DirectivePanelMsg::NetRowHovered(Some(
            net.name.clone(),
        ))))
        .on_exit(msg(DirectivePanelMsg::NetRowHovered(None)))
        .boxed()
}

/// A small tag on a soft status tint.
fn pill<'a>(label: signex_widgets::Label<'a>, tint: Color, ink: Color) -> Element<'a, PanelMsg> {
    container(text(label).size(TS.caption).color(ink))
        .padding([0.0, SP.s])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(tint)),
            border: Border {
                radius: SP.s.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The outline over the wires around it: caught nets in accent, the rest in
/// the secondary ink, with Width and Height callouts linked to the Outline
/// row. Follows the outline and the edge checkbox, as the panel is rebuilt
/// from the sheet after each edit.
fn illustration(
    panel: &DirectivePanel,
    region: signex_types::schematic::Aabb,
    i18n: &signex_i18n::I18n,
) -> Illustration {
    const W: f32 = 280.0;
    const H: f32 = 150.0;
    const PAD: f32 = 28.0;
    let margin = region.width().max(region.height()) * 0.35;
    let view = region.expand(margin);
    let scale = ((W - 2.0 * PAD) as f64 / view.width().max(1e-6))
        .min((H - 2.0 * PAD) as f64 / view.height().max(1e-6)) as f32;
    let at = |p: signex_types::schematic::Point| {
        P::new(
            PAD + (p.x - view.min_x) as f32 * scale,
            PAD + (p.y - view.min_y) as f32 * scale,
        )
    };
    let mut ill = Illustration::new(W, H);
    for (a, b, caught) in &panel.wires {
        let style = if *caught {
            Style::outline(Ink::Accent, Weight::Heavy)
        } else {
            Style::outline(Ink::Secondary, Weight::Regular)
        };
        ill = ill.part(Part::new(
            Primitive::Line {
                from: at(*a),
                to: at(*b),
            },
            style,
        ));
    }
    let tl = at(signex_types::schematic::Point::new(
        region.min_x,
        region.min_y,
    ));
    let br = at(signex_types::schematic::Point::new(
        region.max_x,
        region.max_y,
    ));
    let size = Size::new(br.x - tl.x, br.y - tl.y);
    let (w, h) = (region.width(), region.height());
    ill.part(Part::new(
        Primitive::RoundedRect {
            rect: R::new(tl, size),
            radius: 6.0,
        },
        Style {
            stroke: Some(Stroke {
                paint: Paint::Ink(Ink::Accent),
                weight: Weight::Regular,
                is_dashed: true,
            }),
            fill: Some(Fill {
                paint: Paint::Ink(Ink::Accent),
                tint: Tint::Wash,
            }),
        },
    ))
    .part(
        Part::callout(
            Callout::new(P::new(tl.x, tl.y - 10.0), P::new(br.x, tl.y - 10.0))
                .label(signex_i18n::UserText::new(format!("{w:.2} mm")))
                .extension(tl, P::new(tl.x, tl.y - 13.0))
                .extension(P::new(br.x, tl.y), P::new(br.x, tl.y - 13.0)),
        )
        .linked(FIELD_SIZE),
    )
    .part(
        Part::callout(
            Callout::new(P::new(br.x + 10.0, tl.y), P::new(br.x + 10.0, br.y))
                .label(signex_i18n::UserText::new(format!("{h:.2} mm")))
                .label_at(P::new(br.x + 14.0, (tl.y + br.y) / 2.0), Anchor::Start)
                .extension(P::new(br.x, tl.y), P::new(br.x + 13.0, tl.y))
                .extension(br, P::new(br.x + 13.0, br.y)),
        )
        .linked(FIELD_SIZE),
    )
    .part(Part::label(
        signex_widgets::property_illustration::TextLabel {
            at: P::new(8.0, H - 6.0),
            text: t!(i18n, ids::PANELS_BLANKET_CAUGHT_NOTE).into(),
            anchor: Anchor::Start,
            role: signex_widgets::property_illustration::TextRole::Note,
        },
        Ink::Faint,
    ))
}
