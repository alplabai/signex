//! The board-style preview of a differential pair class: the two traces on
//! top copper over the dark board ground, to scale across the pair, with the
//! width, the gap and the impedance called out. It follows the fields; the
//! values are typed in the fields above it.

use iced::{Element, Length, Point as P, Rectangle as R, Size, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::diff_pair_class::DiffPairClass;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, IllustrationHover, Ink, Paint, Part, Primitive, Style,
    TextLabel, TextRole, Tint, property_illustration,
};

use crate::app::Message;
use crate::styles::board_palette::{BoardPalette, BoardSwatch};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const W: f32 = 500.0;
const H: f32 = 150.0;
/// Where the traces run, left and right.
const X0: f32 = 70.0;
const X1: f32 = 430.0;
/// What a class with no width or gap is drawn with, in mm.
const DRAWN_WIDTH: f64 = 0.2;
const DRAWN_GAP: f64 = 0.15;

/// `value` mm without trailing zeros, or `—`.
fn shown(value: Option<f64>) -> String {
    value.map_or_else(
        || "—".to_string(),
        |v| {
            let text = format!("{v:.3}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        },
    )
}

/// The drawing of `class`.
pub(super) fn illustration(class: &DiffPairClass, i18n: &I18n) -> Illustration {
    let copper = BoardPalette::new().swatch(BoardSwatch::TopCopper);
    let w = class.width_mm.filter(|v| *v > 0.0).unwrap_or(DRAWN_WIDTH);
    let gap = class.gap_mm.filter(|v| *v > 0.0).unwrap_or(DRAWN_GAP);
    let scale = (56.0 / (2.0 * w + gap)).clamp(20.0, 400.0) as f32;
    let cy = H / 2.0 + 8.0;
    let tw = (w as f32 * scale).max(1.0);
    let half = (gap + w) as f32 * scale / 2.0;
    let (y_p, y_n) = (cy - half, cy + half);
    let trace = |y: f32| {
        Part::new(
            Primitive::Rect {
                rect: R::new(P::new(X0, y - tw / 2.0), Size::new(X1 - X0, tw)),
            },
            Style::filled(Paint::Document(copper), Tint::Solid),
        )
    };
    let net = |y: f32, name: signex_i18n::Localized| {
        Part::label(
            TextLabel {
                at: P::new(X1 + 8.0, y + 3.0),
                text: name.into(),
                anchor: Anchor::Start,
                role: TextRole::Mono,
            },
            Ink::Secondary,
        )
    };
    let xd = X0 + (X1 - X0) * 0.28;
    let xg = X0 + (X1 - X0) * 0.62;
    let width_label = t!(i18n, ids::DIALOG_DPC_WIDTH_LABEL, "value" => shown(class.width_mm));
    let gap_label = t!(i18n, ids::DIALOG_DPC_GAP_LABEL, "value" => shown(class.gap_mm));
    let impedance = t!(
        i18n,
        ids::DIALOG_DPC_ZDIFF_LABEL,
        "ohm" => shown(class.impedance_ohm),
        "pct" => shown(class.impedance_tolerance_pct)
    );
    Illustration::new(W, H)
        .height_em(12.0)
        .part(trace(y_p))
        .part(trace(y_n))
        .part(net(y_p, t!(i18n, ids::DIALOG_DPC_P_NET)))
        .part(net(y_n, t!(i18n, ids::DIALOG_DPC_N_NET)))
        .part(Part::callout(
            Callout::new(P::new(xd, y_p - tw / 2.0), P::new(xd, y_p + tw / 2.0))
                .label(width_label)
                .label_at(P::new(xd - 9.0, y_p), Anchor::End),
        ))
        .part(Part::callout(
            Callout::new(P::new(xg, y_p + tw / 2.0), P::new(xg, y_n - tw / 2.0))
                .label(gap_label)
                .label_at(P::new(xg + 9.0, cy), Anchor::Start),
        ))
        .part(Part::label(
            TextLabel {
                at: P::new(12.0, 18.0),
                text: impedance.into(),
                anchor: Anchor::Start,
                role: TextRole::Caption,
            },
            Ink::Secondary,
        ))
}

/// The preview on the dark board ground, which board views keep under the
/// light chrome too; its hover changes nothing.
pub(super) fn preview<'a>(class: &DiffPairClass, i18n: &I18n) -> Element<'a, Message> {
    let ground = BoardPalette::new().canvas;
    let dark = theme_tokens(ThemeId::GraphiteDark);
    let drawing = property_illustration(
        illustration(class, i18n),
        &IllustrationHover::default(),
        &dark,
        TS.caption,
    )
    .map(|_| Message::noop());
    iced::widget::container(drawing)
        .padding(SP.s)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(ground)),
            border: iced::Border {
                radius: SP.xs.into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let ill = illustration(&class, &i18n);
        assert_eq!(ill.visible_parts().count(), 7);
        assert_eq!(shown(Some(0.15)), "0.15");
        assert_eq!(shown(None), "—");
    }
}
