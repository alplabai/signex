//! The theme cards of Appearance ▸ Look (mockup row 144): Graphite Light and Graphite
//! Dark side by side, each a radio, a line-art preview drawn in the theme's
//! own colours, its name, a pill and a description. Choosing a card sends
//! `DraftTheme`; nothing is written to disk.

use super::PrefMsg;
use crate::preferences::setting_row::tone_pill;
use crate::styles::{self, PillTone, Radii, ti, tokens};
use iced::font::Weight;
use iced::widget::{Space, canvas, column, container, row};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Point, Rectangle, Size, Theme,
    Widget as _, mouse,
};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::{button, text};

const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Height of a card's preview (96 px).
const THUMB_H: f32 = TS.body * 8.0;
/// Side of the radio mark (14 px).
const RADIO: f32 = TS.body * 7.0 / 6.0;
/// Inset of the radio's dot.
const RADIO_DOT_INSET: f32 = 3.0;

/// A theme card: id, card title, pill, description.
const FEATURED: [(ThemeId, MessageId, MessageId, MessageId); 2] = [
    (
        ThemeId::GraphiteLight,
        ids::PREFERENCES_THEME_LIGHT_NAME,
        ids::PREFERENCES_THEME_LIGHT_TAG,
        ids::PREFERENCES_THEME_LIGHT_DESCRIPTION,
    ),
    (
        ThemeId::GraphiteDark,
        ids::PREFERENCES_THEME_DARK_NAME,
        ids::PREFERENCES_THEME_DARK_TAG,
        ids::PREFERENCES_THEME_DARK_DESCRIPTION,
    ),
];

/// The two theme cards side by side. `active` carries the dialog's own
/// tokens (labels, card chrome); each preview uses its theme's tokens.
pub(super) fn theme_cards<'a>(
    draft: ThemeId,
    active: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let mut cards = row![].spacing(SP.l).width(Length::Fill);
    for (id, name, pill, desc) in FEATURED {
        cards = cards.push(card(id, [name, pill, desc], draft == id, active, i18n));
    }
    container(cards)
        .padding(iced::Padding {
            top: SP.m,
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .boxed()
}

fn card<'a>(
    id: ThemeId,
    [name, pill, desc]: [MessageId; 3],
    selected: bool,
    active: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let a = tokens::resolve(active);
    let (accent, edge, ink) = (ti(active.accent), ti(active.border), ti(active.text));
    let meta = row![
        radio(selected, active),
        column![
            row![
                text(t!(i18n, name)).size(TS.label).color(ink).font(Font {
                    weight: Weight::Semibold,
                    ..signex_widgets::ui_font::regular()
                }),
                tone_pill(t!(i18n, pill), PillTone::Neutral, active),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center),
            text(t!(i18n, desc)).size(TS.caption).color(a.ink3),
        ]
        .spacing(SP.xs)
        .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Start);
    let body = column![
        thumbnail(&theme_tokens(id)),
        container(meta).padding([SP.m, SP.l]).width(Length::Fill),
    ];
    let panel = ti(active.panel_bg);
    button(body)
        .padding(0)
        .width(Length::FillPortion(1))
        .on_press(PrefMsg::DraftTheme(id))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(panel)),
            text_color: ink,
            border: Border {
                width: if selected { 2.0 } else { 1.0 },
                color: if selected { accent } else { edge },
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}

/// The radio mark: a ring, with an accent dot when chosen.
pub(super) fn radio<'a>(selected: bool, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (accent, ring, fill) = (ti(t.accent), r.line_strong, r.field);
    let dot = container(Space::new())
        .width(Length::Fixed(RADIO - 2.0 * RADIO_DOT_INSET))
        .height(Length::Fixed(RADIO - 2.0 * RADIO_DOT_INSET))
        .style(move |_: &Theme| container::Style {
            background: selected.then_some(Background::Color(accent)),
            border: Border {
                radius: RADIO.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(dot)
        .width(Length::Fixed(RADIO))
        .height(Length::Fixed(RADIO))
        .center_x(Length::Fixed(RADIO))
        .center_y(Length::Fixed(RADIO))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: if selected { accent } else { ring },
                radius: RADIO.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The card's preview in the theme's own colours: a side panel strip with
/// three rows, a dotted canvas, a part box and an accent wire.
fn thumbnail<'a>(t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let drawing = Thumbnail {
        backdrop: ti(t.bg),
        panel: ti(t.panel_bg),
        rule: ti(t.border),
        row_accent: r.accent_soft,
        paper: ti(t.paper),
        dot: r.line_strong,
        ink: ti(t.text),
        accent: ti(t.accent),
    };
    canvas::Canvas::new(drawing)
        .width(Length::Fill)
        .height(Length::Fixed(THUMB_H))
        .boxed()
}

/// Mockup geometry of the preview, in px.
const STRIP_W: f32 = 64.0;
const STRIP_PAD_X: f32 = 6.0;
const STRIP_PAD_Y: f32 = 8.0;
const STRIP_ROW_H: f32 = 5.0;
const STRIP_ROW_GAP: f32 = 5.0;
const DOT_PITCH: f32 = 10.0;
const DOT_R: f32 = 1.0;
/// The part: x, y, width, height from the canvas's left edge.
const PART: (f32, f32, f32, f32) = (30.0, 30.0, 60.0, 34.0);
/// The wire: x, y, length.
const WIRE: (f32, f32, f32) = (90.0, 44.0, 50.0);
const LINE_W: f32 = 1.5;

struct Thumbnail {
    backdrop: Color,
    panel: Color,
    rule: Color,
    row_accent: Color,
    paper: Color,
    dot: Color,
    ink: Color,
    accent: Color,
}

impl canvas::Program<PrefMsg> for Thumbnail {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        frame.fill_rectangle(Point::ORIGIN, Size::new(w, h), self.backdrop);
        // Side panel with three rows, the middle one in the accent tint.
        frame.fill_rectangle(Point::ORIGIN, Size::new(STRIP_W, h), self.panel);
        frame.fill_rectangle(Point::new(STRIP_W - 1.0, 0.0), Size::new(1.0, h), self.rule);
        let rows = [self.rule, self.row_accent, self.rule];
        for (i, colour) in rows.into_iter().enumerate() {
            let y = STRIP_PAD_Y + i as f32 * (STRIP_ROW_H + STRIP_ROW_GAP);
            frame.fill_rectangle(
                Point::new(STRIP_PAD_X, y),
                Size::new(STRIP_W - 2.0 * STRIP_PAD_X, STRIP_ROW_H),
                colour,
            );
        }
        // Dotted canvas.
        frame.fill_rectangle(
            Point::new(STRIP_W, 0.0),
            Size::new((w - STRIP_W).max(0.0), h),
            self.paper,
        );
        let dots = canvas::Path::new(|b| {
            let mut x = STRIP_W + DOT_PITCH / 2.0;
            while x < w {
                let mut y = DOT_PITCH / 2.0;
                while y < h {
                    b.circle(Point::new(x, y), DOT_R);
                    y += DOT_PITCH;
                }
                x += DOT_PITCH;
            }
        });
        frame.fill(&dots, self.dot);
        // A part and the wire leaving it.
        let (px, py, pw, ph) = PART;
        frame.stroke(
            &canvas::Path::rectangle(Point::new(STRIP_W + px, py), Size::new(pw, ph)),
            canvas::Stroke::default()
                .with_color(self.ink)
                .with_width(LINE_W),
        );
        let (wx, wy, wl) = WIRE;
        frame.stroke(
            &canvas::Path::line(
                Point::new(STRIP_W + wx, wy),
                Point::new(STRIP_W + wx + wl, wy),
            ),
            canvas::Stroke::default()
                .with_color(self.accent)
                .with_width(LINE_W),
        );
        vec![frame.into_geometry()]
    }
}
