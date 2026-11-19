//! Property illustration samples, to check the widget against the approved
//! Properties panel drawings: a track and an SMD pad, idle, with a field
//! hovered, with a callout hovered and with a feature switched off.

use iced::widget::{column, container, row};
use iced::{Element, Length, Point, Rectangle, Size, Widget};
use signex_i18n::UserText;
use signex_types::theme::{BoardLayerColors, ThemeId, ThemeTokens, canvas_colors};
use signex_widgets::property_illustration::{
    Anchor, Callout, Cap, FieldId, Illustration, IllustrationHover, IllustrationMsg, Ink, Leader,
    Paint, Part, Primitive, Style, Text, TextLabel, TextRole, Tint, Weight, property_illustration,
};
use signex_widgets::theme_ext::to_color;

use crate::catalog::Message;

/// The caption step of the panel type scale the samples are drawn at.
const CAPTION: f32 = 11.0;
/// Width of one sample, as wide as the Properties panel, in captions.
const PANEL_EM: f32 = 24.0;
/// The view box of the approved drawings.
const VIEW: Size = Size::new(264.0, 140.0);

const PASTE_ON: FieldId = FieldId::new("pasteon");

fn layers() -> BoardLayerColors {
    canvas_colors(ThemeId::DEFAULT).board_layer_colors()
}

/// Sample label text. The samples stand for catalog messages and user text;
/// one helper keeps them in one place.
fn sample_text(text: &'static str) -> Text {
    UserText::new(text).into()
}

fn label(at: Point, text: &'static str, anchor: Anchor, role: TextRole) -> TextLabel {
    TextLabel {
        at,
        text: sample_text(text),
        anchor,
        role,
    }
}

/// #95 Track: a routed segment with its Width callout, net and layer note.
fn track() -> Illustration {
    let copper = Paint::Document(to_color(&layers().top_copper));
    let band = Primitive::Band {
        points: vec![
            Point::new(22.0, 96.0),
            Point::new(96.0, 96.0),
            Point::new(140.0, 52.0),
            Point::new(240.0, 52.0),
        ],
        width: 16.0,
        cap: Cap::Round,
    };
    Illustration::new(VIEW.width, VIEW.height)
        .part(
            Part::new(band, Style::edge().with_fill(copper, Tint::Body))
                .linked_all(["w", "net", "layer", "ends"]),
        )
        .part(
            Part::callout(
                Callout::new(Point::new(60.0, 88.0), Point::new(60.0, 104.0))
                    .label(sample_text("Width 0.200"))
                    .label_at(Point::new(60.0, 124.0), Anchor::Middle),
            )
            .linked("w"),
        )
        .part(
            Part::label(
                label(
                    Point::new(190.0, 34.0),
                    "SPI1_SCK",
                    Anchor::Middle,
                    TextRole::Note,
                ),
                Ink::Faint,
            )
            .linked("net"),
        )
        .part(
            Part::label(
                label(
                    Point::new(258.0, 132.0),
                    "Top Layer · round ends",
                    Anchor::End,
                    TextRole::Note,
                ),
                Ink::Faint,
            )
            .linked_all(["layer", "ends"]),
        )
}

/// #97 Pad: SMD top view with X / Y, mask and paste, beside a side view.
fn pad(paste_on: bool) -> Illustration {
    let colors = layers();
    let copper = Paint::Document(to_color(&colors.top_copper));
    let mask = Paint::Document(to_color(&colors.solder_mask));
    let rect = |x: f32, y: f32, w: f32, h: f32| Rectangle::new(Point::new(x, y), Size::new(w, h));
    Illustration::new(VIEW.width, VIEW.height)
        .part(
            Part::new(
                Primitive::Rect {
                    rect: rect(22.0, 41.0, 80.0, 50.0),
                },
                Style::edge().with_fill(copper, Tint::Body),
            )
            .linked_all(["sx", "sy", "shape"]),
        )
        .part(
            Part::new(
                Primitive::Rect {
                    rect: rect(26.0, 43.5, 72.0, 45.0),
                },
                Style::dashed(),
            )
            .linked_all(["paste", "pasteon"])
            .requires(PASTE_ON),
        )
        .part(
            Part::new(
                Primitive::RoundedRect {
                    rect: rect(17.0, 36.0, 90.0, 60.0),
                    radius: 2.0,
                },
                Style::outline(Ink::Secondary, Weight::Thin).with_stroke_paint(mask),
            )
            .linked("mexp"),
        )
        .part(
            Part::callout(
                Callout::new(Point::new(22.0, 26.0), Point::new(102.0, 26.0))
                    .label(sample_text("X"))
                    .extension(Point::new(22.0, 41.0), Point::new(22.0, 23.0))
                    .extension(Point::new(102.0, 41.0), Point::new(102.0, 23.0)),
            )
            .linked("sx"),
        )
        .part(
            Part::callout(
                Callout::new(Point::new(8.0, 41.0), Point::new(8.0, 91.0))
                    .label(sample_text("Y"))
                    .extension(Point::new(22.0, 41.0), Point::new(5.0, 41.0))
                    .extension(Point::new(22.0, 91.0), Point::new(5.0, 91.0)),
            )
            .linked("sy"),
        )
        .part(
            Part::leader(Leader {
                at: Point::new(26.0, 88.0),
                label_at: Point::new(40.0, 124.0),
                label: sample_text("Paste 0%"),
                anchor: Anchor::Start,
            })
            .linked_all(["paste", "pasteon"])
            .requires(PASTE_ON),
        )
        .part(Part::label(
            label(
                Point::new(258.0, 12.0),
                "Side view",
                Anchor::End,
                TextRole::Note,
            ),
            Ink::Faint,
        ))
        .part(Part::new(
            Primitive::Rect {
                rect: rect(140.0, 50.0, 118.0, 48.0),
            },
            Style::filled(Paint::Ink(Ink::Rule), Tint::Light),
        ))
        .part(
            Part::new(
                Primitive::Rect {
                    rect: rect(170.0, 45.0, 58.0, 5.0),
                },
                Style::outline(Ink::Secondary, Weight::Thin).with_fill(copper, Tint::Strong),
            )
            .linked("sx"),
        )
        .part(
            Part::new(
                Primitive::Rect {
                    rect: rect(166.0, 38.0, 66.0, 4.0),
                },
                Style::dashed(),
            )
            .linked_all(["paste", "pasteon"])
            .requires(PASTE_ON),
        )
        .part(
            Part::label(
                label(
                    Point::new(258.0, 132.0),
                    "Top Layer · Rectangle",
                    Anchor::End,
                    TextRole::Note,
                ),
                Ink::Faint,
            )
            .linked("shape"),
        )
        .feature(PASTE_ON, paste_on)
}

fn hovered(message: IllustrationMsg) -> IllustrationHover {
    let mut hover = IllustrationHover::default();
    hover.update(message);
    hover
}

fn sample<'a>(
    tokens: &ThemeTokens,
    illustration: Illustration,
    hover: &IllustrationHover,
) -> Element<'a, Message> {
    // Static samples: the hover each one shows is fixed, so the widget's
    // hover messages are dropped.
    let drawing = property_illustration(illustration, hover, tokens, CAPTION)
        .map(|_| Message::TreeInteracted);
    container(drawing)
        .width(Length::Fixed(CAPTION * PANEL_EM))
        .boxed()
}

/// Top row: track idle, then with the Width field hovered. Bottom row: pad
/// with the X callout hovered, then with Paste switched off.
pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let idle = IllustrationHover::default();
    let field = hovered(IllustrationMsg::FieldHovered(FieldId::new("w")));
    let callout = hovered(IllustrationMsg::CalloutHovered(FieldId::new("sx")));
    column![
        row![
            sample(tokens, track(), &idle),
            sample(tokens, track(), &field),
        ]
        .spacing(CAPTION * 2.0),
        row![
            sample(tokens, pad(true), &callout),
            sample(tokens, pad(false), &idle),
        ]
        .spacing(CAPTION * 2.0),
    ]
    .spacing(CAPTION * 2.0)
    .boxed()
}
