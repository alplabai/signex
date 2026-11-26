//! The Variant section of a channel tab's Properties (#2662, mockup row
//! 151): the active variant, a line naming the channels it leaves out, how
//! this channel fits ("Fitted", "Not fitted" or "mixed · 2 of 9 not
//! fitted") and the parts the channels fit differently, each with its
//! "mixed · 1 of 3 not fitted". Read-only: the line says where to edit.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, ListKind, Localized, UserText, ids, t};
use signex_widgets::text;

use super::super::{PanelContext, PanelMsg};
use crate::panels::compiled_design::{ChannelStripView, ChannelVariantSummary};
use crate::styles::{PillTone, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the label column: 80 px at the 12 px body.
const LABEL_W: f32 = TS.body * 80.0 / 12.0;

/// The section's rows for `strip`'s shown channel; empty on Editor.
pub(super) fn section<'a>(
    ctx: &'a PanelContext,
    strip: &'a ChannelStripView,
) -> Element<'a, PanelMsg> {
    let Some(summary) = strip.variant_summary() else {
        return Column::<Element<'a, PanelMsg>>::new().boxed();
    };
    let i18n = &*ctx.i18n;
    let res = resolve(&ctx.tokens);
    let variant = strip.variant.as_deref();
    let ink = ti(ctx.tokens.text);
    let active = match variant {
        Some(name) => text(UserText::new(name.to_string()))
            .size(TS.body)
            .color(ink),
        None => text(t!(i18n, ids::PROJECT_VARIANTS_BASE))
            .size(TS.body)
            .color(ink),
    };
    let mut rows: Column<Element<'a, PanelMsg>> = column![labelled(
        ctx,
        t!(i18n, ids::PROPERTIES_CHANNEL_VARIANT_ACTIVE),
        active.boxed(),
    )]
    .spacing(SP.s)
    .padding([SP.s, SP.m]);
    rows = rows.push(
        text(overview(i18n, variant, &summary))
            .size(TS.caption)
            .color(res.ink3)
            .width(Length::Fill)
            .boxed(),
    );
    if variant.is_some() && summary.is_compiled() {
        rows = rows.push(labelled(
            ctx,
            t!(i18n, ids::PROPERTIES_CHANNEL_VARIANT_THIS),
            this_channel(ctx, &summary),
        ));
        for part in summary.mixed {
            rows = rows.push(
                row![
                    text(UserText::new(part.designator))
                        .size(TS.body)
                        .font(crate::fonts::iced_font_for_family(
                            crate::fonts::CHROME_MONO_FONT
                        ))
                        .width(LABEL_W),
                    mixed_pill(ctx, part.not_fitted, part.of),
                ]
                .spacing(SP.s)
                .align_y(Alignment::Center)
                .boxed(),
            );
        }
    }
    rows.boxed()
}

/// One line: what the variant does to the channels, and where to edit it.
fn overview(i18n: &I18n, variant: Option<&str>, summary: &ChannelVariantSummary) -> Localized {
    let Some(variant) = variant else {
        return t!(i18n, ids::PROPERTIES_CHANNEL_VARIANT_BASE);
    };
    if !summary.is_compiled() {
        return t!(i18n, ids::PROPERTIES_CHANNEL_VARIANT_NOT_COMPILED, "variant" => variant);
    }
    if summary.left_out.is_empty() {
        return t!(i18n, ids::PROPERTIES_CHANNEL_VARIANT_ALL_FITTED, "variant" => variant);
    }
    t!(
        i18n,
        ids::PROPERTIES_CHANNEL_VARIANT_LEFT_OUT,
        "variant" => variant,
        "channels" => i18n.list(&summary.left_out, ListKind::And),
        "count" => summary.left_out.len()
    )
}

/// How the shown channel fits: Fitted, Not fitted, or mixed.
fn this_channel<'a>(ctx: &PanelContext, summary: &ChannelVariantSummary) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    match summary.not_fitted {
        0 => text(t!(i18n, ids::PROJECT_VARIANTS_FITTED))
            .size(TS.body)
            .color(ti(ctx.tokens.text))
            .boxed(),
        n if n == summary.parts => text(t!(i18n, ids::PROJECT_VARIANTS_NOT_FITTED))
            .size(TS.body)
            .color(resolve(&ctx.tokens).ink3)
            .boxed(),
        n => mixed_pill(ctx, n, summary.parts),
    }
}

/// "mixed · 1 of 3 not fitted" on the warning tint, as the matrix draws it.
fn mixed_pill<'a>(ctx: &PanelContext, not_fitted: usize, of: usize) -> Element<'a, PanelMsg> {
    container(
        text(t!(
            ctx.i18n,
            ids::PROJECT_VARIANTS_CHANNEL_MIXED,
            "count" => not_fitted,
            "total" => of
        ))
        .size(TS.caption)
        .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s])
    .style(crate::styles::pill(&ctx.tokens, PillTone::Warn))
    .boxed()
}

/// A label column and its value.
fn labelled<'a>(
    ctx: &PanelContext,
    label: Localized,
    value: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        text(label)
            .size(TS.body)
            .color(resolve(&ctx.tokens).ink3)
            .width(LABEL_W),
        value,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}
