//! Shared Graphite empty-state cards. [`empty_state_card`]: a title, a line
//! of explanation and an optional muted caption, on the card surface, with
//! optional action buttons under the text; used by panels with nothing to
//! list yet (Selection Filter, Navigator, Net Inspector). [`planned_card`]:
//! the one card every planned panel shows (#1283), with a dashed outline so
//! it is never taken for an empty real panel.

use iced::widget::canvas::{self, Canvas, LineDash, Path, Stroke};
use iced::widget::{Column, Row, column, container, stack, svg};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    Widget as _, mouse,
};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use crate::styles::{self, PillTone, Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A button of [`empty_state_card_with_actions`]: its label, what a press
/// emits, and whether it is the card's primary action.
#[derive(Debug, Clone)]
pub struct EmptyStateAction<M> {
    pub label: Localized,
    pub on_press: M,
    pub primary: bool,
}

/// Side of the planned card's icon disc, in `body`-size characters.
const DISC_EMS: f32 = 34.0 / 12.0;
/// Side of the icon inside the disc, in `body`-size characters.
const ICON_EMS: f32 = 18.0 / 12.0;
/// Dash and gap of the planned card's outline, in px.
const DASH: [f32; 2] = [4.0, 3.0];
/// The card, 12 px padded, filling the panel width.
pub fn empty_state_card<'a, M: 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
) -> Element<'a, M> {
    card(tokens, lines(tokens, title, body, caption))
}

/// [`empty_state_card`] with `actions` as a row of buttons under the text:
/// the primary one in the Graphite primary look, the others secondary. No
/// actions draws the plain card.
pub fn empty_state_card_with_actions<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
    actions: Vec<EmptyStateAction<M>>,
) -> Element<'a, M> {
    let mut content = lines(tokens, title, body, caption);
    if !actions.is_empty() {
        let mut buttons: Row<Element<'a, M>> = Row::new().spacing(SP.s);
        for action in actions {
            buttons = buttons.push(action_button(tokens, action));
        }
        content = content.push(
            container(buttons)
                .padding(iced::Padding {
                    top: SP.xs,
                    ..iced::Padding::ZERO
                })
                .boxed(),
        );
    }
    card(tokens, content)
}

fn lines<'a, M: 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
) -> Column<Element<'a, M>> {
    let mut lines = column![
        text(title).size(TS.label).color(ti(tokens.text)),
        text(body).size(TS.dense).color(ti(tokens.text_secondary)),
    ]
    .spacing(6);
    if let Some(caption) = caption {
        lines = lines.push(
            text(caption)
                .size(TS.caption)
                .color(resolve(tokens).ink4)
                .boxed(),
        );
    }
    lines
}

fn card<'a, M: 'a>(tokens: &ThemeTokens, content: Column<Element<'a, M>>) -> Element<'a, M> {
    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(styles::card(tokens))
        .boxed()
}

fn action_button<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    action: EmptyStateAction<M>,
) -> Element<'a, M> {
    let pressed = button(text(action.label).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(action.on_press);
    if action.primary {
        pressed.style(styles::primary_button(tokens)).boxed()
    } else {
        pressed.style(styles::outline_button(tokens)).boxed()
    }
}

/// A planned panel's card, centred in the panel body: `icon` (a 16 × 16
/// `currentColor` stroke glyph) on a disc, the panel's `name`, the `planned`
/// pill and what the panel will do (`what`), inside a dashed outline. The
/// dock title stays the panel's only title row.
pub fn planned_card<'a, M: 'a>(
    tokens: &ThemeTokens,
    icon: &'static [u8],
    name: Localized,
    planned: Localized,
    what: Localized,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let disc_fill = r.sunken;
    let disc = container(
        svg(crate::icons::static_glyph(icon))
            .width(TS.body * ICON_EMS)
            .height(TS.body * ICON_EMS)
            .style(move |_: &Theme, _| svg::Style {
                color: Some(r.ink3),
                ..Default::default()
            }),
    )
    .center_x(TS.body * DISC_EMS)
    .center_y(TS.body * DISC_EMS)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(disc_fill)),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let pill = container(text(planned).size(TS.caption))
        .padding([0.0, SP.m])
        .style(styles::pill(tokens, PillTone::Warn));
    let lines = column![
        disc,
        text(name)
            .size(TS.label)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
            })
            .color(ti(tokens.text)),
        pill,
        text(what)
            .size(TS.body)
            .color(r.ink3)
            .align_x(iced::alignment::Horizontal::Center),
    ]
    .spacing(SP.s)
    .align_x(Alignment::Center)
    .width(Length::Fill);
    let face = container(lines).width(Length::Fill).padding([SP.xl, SP.l]);
    let outline = Canvas::new(DashedOutline {
        color: r.line_strong,
        radius: Radii::GRAPHITE.card,
    })
    .width(Length::Fill)
    .height(Length::Fill);
    container(stack![face, outline].width(Length::Fill))
        .padding([SP.l, SP.m])
        .center_x(Length::Fill)
        .boxed()
}

/// A 1 px dashed rounded rectangle around its bounds: the planned card's
/// outline (a container border cannot be dashed).
struct DashedOutline {
    color: Color,
    radius: f32,
}

impl<M> canvas::Program<M> for DashedOutline {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let inset = 0.5;
        let size = Size::new(
            (bounds.width - 2.0 * inset).max(0.0),
            (bounds.height - 2.0 * inset).max(0.0),
        );
        let path = Path::rounded_rectangle(Point::new(inset, inset), size, self.radius.into());
        frame.stroke(
            &path,
            Stroke {
                line_dash: LineDash {
                    segments: &DASH,
                    offset: 0,
                },
                ..Stroke::default().with_color(self.color).with_width(1.0)
            },
        );
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::{I18n, ids, menu_ids, t};
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::super::widgets::view_stub;
    use crate::layout_probe::lay_out_texts;

    /// The texts of a card with `actions` buttons, each labelled Open
    /// Project…, that read as that label.
    fn buttons_of(actions: usize) -> usize {
        let i18n = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let actions: Vec<EmptyStateAction<usize>> = (0..actions)
            .map(|n| EmptyStateAction {
                label: t!(i18n, ids::PANELS_NAVIGATOR_OPEN_PROJECT),
                on_press: n,
                primary: n == 0,
            })
            .collect();
        let card = empty_state_card_with_actions(
            &tokens,
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT),
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT_BODY),
            None,
            actions,
        );
        let want = i18n.message_plain(ids::PANELS_NAVIGATOR_OPEN_PROJECT, &[]);
        lay_out_texts(card, iced::Size::new(300.0, 400.0))
            .into_iter()
            .filter(|t| t.text == want)
            .count()
    }

    #[test]
    fn the_card_builds_with_zero_one_and_two_actions() {
        assert_eq!(buttons_of(0), 0);
        assert_eq!(buttons_of(1), 1);
        assert_eq!(buttons_of(2), 2);
    }

    /// Mockup row 60: a planned panel draws one card: its name once, the
    /// planned pill and what it will do. No upper-case section title and no
    /// rule of box-drawing glyphs above it: the dock title is the only title.
    #[test]
    fn a_planned_panel_shows_one_card_without_a_second_title() {
        let (app, _boot) = crate::app::Signex::new();
        let ctx = app.document_view.panel_ctx.clone();
        for (kind, what) in [
            (ids::PANELS_KIND_FAVORITES, ids::PANELS_STUB_FAVORITES),
            (ids::PANELS_KIND_SNIPPETS, ids::PANELS_STUB_SNIPPETS),
            (ids::PANELS_KIND_TODO, ids::PANELS_STUB_TODO),
            (ids::PANELS_KIND_WIKI, ids::PANELS_STUB_WIKI),
        ] {
            let texts: Vec<String> =
                lay_out_texts(view_stub(kind, what, &ctx), iced::Size::new(220.0, 300.0))
                    .into_iter()
                    .map(|laid| laid.text)
                    .collect();
            let expected = [
                ctx.i18n.message_plain(kind, &[]),
                ctx.i18n.message_plain(menu_ids::MENU_PLANNED_TOOLTIP, &[]),
                ctx.i18n.message_plain(what, &[]),
            ];
            assert_eq!(texts, expected);
            let upper = ctx.i18n.upper_for_display(&expected[0]);
            assert!(
                upper == expected[0] || !texts.contains(&upper),
                "no upper-case title row"
            );
            assert!(!texts.iter().any(|t| t.starts_with('\u{2500}')), "no rule");
        }
    }
}
