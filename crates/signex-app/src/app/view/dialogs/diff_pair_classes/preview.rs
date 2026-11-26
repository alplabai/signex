//! The board-style preview of a differential pair class: the two traces on
//! top copper over the dark board ground, to scale across the pair, with the
//! width, the gap, the impedance and its tolerance written on it. Each value
//! can be typed on the drawing, a handle on the N trace drags the gap, and
//! the expand button draws the pair taller (mockup row 153). Where things
//! sit and what a label edits: `app/diff_pair_classes/preview.rs`.

use iced::widget::{column, container, mouse_area, stack};
use iced::{Element, Length, Point as P, Rectangle as R, Size, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::diff_pair_class::DiffPairClass;
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::dimension_label::{DimensionLabelMessage, DimensionLabelStyle, LabelAnchor};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, IllustrationHover, Ink, Paint, Part, Primitive, Style,
    TextLabel, TextRole, Tint, property_illustration,
};

use crate::app::Message;
use crate::app::diff_pair_classes::preview::{
    PairGeometry, PairPreview, PairPreviewMsg, TRACE_X, VIEW_W, preview_labels, value_texts,
};
use crate::app::diff_pair_classes::{DiffPairClassesMsg, DiffPairClassesState};
use crate::styles::board_palette::{BoardPalette, BoardSwatch};
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn preview_msg(msg: PairPreviewMsg) -> Message {
    Message::from(DiffPairClassesMsg::PreviewActed(msg))
}

/// The illustration anchor a label anchor stands for.
fn anchor(at: LabelAnchor) -> Anchor {
    match at {
        LabelAnchor::Start => Anchor::Start,
        LabelAnchor::Middle => Anchor::Middle,
        LabelAnchor::End => Anchor::End,
    }
}

/// The drawing of `class`.
pub(super) fn illustration(
    class: &DiffPairClass,
    preview: &PairPreview,
    i18n: &I18n,
) -> Illustration {
    let copper = BoardPalette::new().swatch(BoardSwatch::TopCopper);
    let g = PairGeometry::of(class, preview);
    let (x0, x1) = TRACE_X;
    let tw = g.trace_w;
    let trace = |y: f32| {
        Part::new(
            Primitive::Rect {
                rect: R::new(P::new(x0, y - tw / 2.0), Size::new(x1 - x0, tw)),
            },
            Style::filled(Paint::Document(copper), Tint::Solid),
        )
    };
    let net = |y: f32, name: signex_i18n::Localized| {
        Part::label(
            TextLabel {
                at: P::new(x1 + 8.0, y + 3.0),
                text: name.into(),
                anchor: Anchor::Start,
                role: TextRole::Mono,
            },
            Ink::Secondary,
        )
    };
    let [width, gap, impedance, tolerance] = value_texts(class, i18n);
    let spots = g.value_spots(&impedance);
    let xd = PairGeometry::width_x();
    let xg = PairGeometry::gap_x();
    let value = |text: signex_i18n::Localized, (at, at_anchor): (P, LabelAnchor)| {
        Part::label(
            TextLabel {
                at,
                text: text.into(),
                anchor: anchor(at_anchor),
                role: TextRole::Caption,
            },
            Ink::Secondary,
        )
    };
    let (width_at, width_anchor) = spots[0];
    let (gap_at, gap_anchor) = spots[1];
    Illustration::new(VIEW_W, g.view_h)
        .height_em(g.height_em())
        .part(trace(g.y_p))
        .part(trace(g.y_n))
        .part(net(g.y_p, t!(i18n, ids::DIALOG_DPC_P_NET)))
        .part(net(g.y_n, t!(i18n, ids::DIALOG_DPC_N_NET)))
        .part(Part::callout(
            Callout::new(P::new(xd, g.y_p - tw / 2.0), P::new(xd, g.y_p + tw / 2.0))
                .label(width)
                .label_at(width_at, anchor(width_anchor)),
        ))
        .part(Part::callout(
            Callout::new(P::new(xg, g.y_p + tw / 2.0), P::new(xg, g.y_n - tw / 2.0))
                .label(gap)
                .label_at(gap_at, anchor(gap_anchor)),
        ))
        .part(value(impedance, spots[2]))
        .part(value(tolerance, spots[3]))
}

/// The value labels' look, from the Graphite Dark ink, so they read on the
/// board ground under both chrome themes.
fn label_style() -> DimensionLabelStyle {
    let dark = theme_tokens(ThemeId::GraphiteDark);
    let resolved = crate::styles::tokens::resolve(&dark);
    DimensionLabelStyle {
        ink: ti(dark.warning),
        fill: ti(dark.bg),
        field: resolved.field,
        selection: ti(dark.selection),
        handle: ti(dark.text),
        text_size: TS.caption,
    }
}

/// The preview on the dark board ground, which board views keep under the
/// light chrome too: the drawing, its value labels and handle, the expand
/// button, and the hint under it. While the handle is held, a release
/// anywhere on the preview lets go of it.
pub(super) fn preview<'a>(
    state: &'a DiffPairClassesState,
    class: &DiffPairClass,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ground = BoardPalette::new().canvas;
    let dark = theme_tokens(ThemeId::GraphiteDark);
    let g = PairGeometry::of(class, &state.preview);
    let drawing = property_illustration(
        illustration(class, &state.preview, i18n),
        &IllustrationHover::default(),
        &dark,
        TS.caption,
    )
    .map(|_| Message::noop());
    let labels = preview_labels(class, &state.preview, i18n);
    let overlay = state
        .preview
        .labels
        .view(&labels, label_style())
        .map(|m| preview_msg(PairPreviewMsg::Labels(m)));
    let expanded = state.preview.expanded;
    let tip = if expanded {
        ids::DIALOG_GRID_COLLAPSE
    } else {
        ids::DIALOG_GRID_EXPAND
    };
    let expand = super::super::grid_properties::expand_button(
        expanded,
        tip,
        preview_msg(PairPreviewMsg::ExpandToggled),
        i18n,
        &dark,
    );
    let corner = container(expand)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right);
    let board = container(stack![drawing, overlay, corner.boxed()])
        .width(Length::Fixed(g.width_px()))
        .height(Length::Fixed(g.height_px()));
    let framed =
        container(board)
            .padding(SP.s)
            .center_x(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(ground)),
                border: iced::Border {
                    radius: SP.xs.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
    let mut framed: Element<'a, Message> = framed.boxed();
    if state.preview.labels.dragging_index().is_some() {
        framed = mouse_area(framed)
            .on_release(preview_msg(PairPreviewMsg::Labels(
                DimensionLabelMessage::PointerReleased,
            )))
            .boxed();
    }
    column![
        framed,
        signex_widgets::text(t!(i18n, ids::DIALOG_DPC_PREVIEW_HINT))
            .size(TS.caption)
            .color(ti(tokens.text_secondary))
            .wrapping(iced::widget::text::Wrapping::Word),
    ]
    .spacing(SP.xs)
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::diff_pair_classes::preview::shown_value;

    #[test]
    fn the_preview_draws_both_traces_and_names_the_values() {
        let i18n = I18n::en_us();
        let class = DiffPairClass {
            width_mm: Some(0.2),
            gap_mm: Some(0.15),
            impedance_ohm: Some(90.0),
            impedance_tolerance_pct: Some(10.0),
            ..DiffPairClass::named("USB")
        };
        let ill = illustration(&class, &PairPreview::default(), &i18n);
        // Two traces, two net names, two callouts, the impedance and its
        // tolerance.
        assert_eq!(ill.visible_parts().count(), 8);
        assert_eq!(shown_value(Some(0.15)), "0.15");
        assert_eq!(shown_value(None), "—");
    }

    /// The expanded drawing is taller, the same width, and keeps the pair
    /// inside it.
    #[test]
    fn the_expanded_preview_is_taller_and_keeps_the_pair_inside() {
        let class = DiffPairClass {
            width_mm: Some(0.1),
            gap_mm: Some(0.1),
            ..DiffPairClass::named("USB")
        };
        let mut preview = PairPreview::default();
        let small = PairGeometry::of(&class, &preview);
        preview.expanded = true;
        let large = PairGeometry::of(&class, &preview);
        assert_eq!(large.width_px(), small.width_px());
        assert!(large.height_px() > small.height_px());
        for g in [small, large] {
            assert!(g.y_p - g.trace_w / 2.0 > 0.0);
            assert!(g.y_n + g.trace_w / 2.0 < g.view_h);
        }
    }
}
