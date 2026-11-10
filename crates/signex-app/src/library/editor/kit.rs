//! Shared dialog kit for the Component Preview tabs.
//!
//! One place for section headers, cards, tables, buttons and empty
//! states so the tabs stop hand-rolling them. Everything is generic over
//! the caller's `Message`, takes `&ThemeTokens`, and draws only with
//! theme tokens ([`Overlay`] steps over `tokens.text`), so it reads on
//! light and dark themes alike.

use iced::widget::{Space, Widget, column, container, row};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text};

use crate::styles::tokens::{self, Overlay, resolve};
use crate::styles::{self, HAIRLINE, RADIUS, Radii, Spacing, TypeScale};

/// Type scale shared by every Component Preview tab.
pub(crate) const TS: TypeScale = TypeScale::DEFAULT;
/// Spacing scale shared by every Component Preview tab.
pub(crate) const SP: Spacing = Spacing::DEFAULT;

// ─── Style functions (pure; unit-tested) ─────────────────────

/// Card surface: faint overlay over the panel, 1 px token border.
pub(crate) fn card_style(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    // Raised fill, card radius (the `styles::card` look).
    let fill = resolve(tokens).raised;
    let radius = Radii::GRAPHITE.card;
    let border = theme_ext::border_color(tokens);
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            radius: radius.into(),
            color: border,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// 1 px rule in the border colour (header underline).
pub(crate) fn rule_style(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let border = theme_ext::border_color(tokens);
    move |_| container::Style {
        background: Some(Background::Color(border)),
        ..container::Style::default()
    }
}

/// 1 px soft rule between body rows.
pub(crate) fn row_separator_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let line = resolve(tokens).line_soft;
    move |_| container::Style {
        background: Some(Background::Color(line)),
        ..container::Style::default()
    }
}

/// Table header band: the sunken fill.
pub(crate) fn header_band_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let fill = resolve(tokens).sunken;
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        ..container::Style::default()
    }
}

/// Body-row fill: a hovered or pressed row takes the hover / pressed
/// overlay. Rows are flat and split by [`row_separator_style`].
pub(crate) fn row_background(tokens: &ThemeTokens, status: button::Status) -> Option<Background> {
    let color = match status {
        button::Status::Hovered => tokens::overlay(tokens, Overlay::Hover),
        button::Status::Pressed => tokens::overlay(tokens, Overlay::Pressed),
        _ => return None,
    };
    Some(Background::Color(color))
}

fn row_style(tokens: &ThemeTokens) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let tokens = *tokens;
    let text_c = theme_ext::text_primary(&tokens);
    move |_: &Theme, status| button::Style {
        background: row_background(&tokens, status),
        text_color: text_c,
        border: Border::default(),
        ..button::Style::default()
    }
}

/// Destructive button: outline shape with `tokens.error` text and border.
pub(crate) fn danger_button_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let danger = tokens::danger(tokens);
    let disabled = tokens::disabled_text(tokens);
    let hover = tokens::overlay(tokens, Overlay::Hover);
    let pressed = tokens::overlay(tokens, Overlay::Pressed);
    let button_radius = Radii::GRAPHITE.button;
    move |_: &Theme, status| {
        let ink = if status == button::Status::Disabled {
            disabled
        } else {
            danger
        };
        button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(hover)),
                button::Status::Pressed => Some(Background::Color(pressed)),
                _ => None,
            },
            text_color: ink,
            border: Border {
                width: HAIRLINE,
                radius: button_radius.into(),
                color: ink,
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

// ─── Layout pieces ───────────────────────────────────────────

fn rule<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(rule_style(tokens))
        .boxed()
}

/// Section title with an optional trailing element (a button, a count)
/// pushed to the right edge, over a header rule.
pub(crate) fn section_header<'a, M: 'a>(
    title: signex_i18n::Localized,
    trailing: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    // Muted section label; the catalog writes it in capitals.
    let heading = text(title)
        .size(TS.section)
        .style(styles::section_label(tokens))
        .boxed();
    let mut line: iced::widget::Row<Element<'a, M>> =
        row![heading, Space::new().width(Length::Fill).boxed()].align_y(iced::Alignment::Center);
    if let Some(t) = trailing {
        line = line.push(t);
    }
    column![container(line).padding([SP.s, 0.0]).boxed(), rule(tokens)]
        .width(Length::Fill)
        .boxed()
}

/// Bordered card around `content`.
pub(crate) fn card<'a, M: 'a>(content: Element<'a, M>, tokens: &ThemeTokens) -> Element<'a, M> {
    container(content)
        .padding(SP.l)
        .width(Length::Fill)
        .style(card_style(tokens))
        .boxed()
}

/// A table column: its heading (written in capitals in the catalog; `None`
/// for an untitled action column) and its width.
pub(crate) type TableColumn = (Option<signex_i18n::MessageId>, Length);

/// Table header: muted column titles over a header rule. Pass the same
/// widths to [`table_row`] cells.
pub(crate) fn table_header<'a, M: 'a>(
    columns: &[TableColumn],
    i18n: &signex_i18n::I18n,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let muted = resolve(tokens).ink3;
    let mut line: iced::widget::Row<Element<'a, M>> =
        row![].spacing(SP.m).align_y(iced::Alignment::Center);
    for (title, width) in columns {
        let cell: Element<'a, M> = match title {
            Some(id) => text(signex_i18n::t!(i18n, *id))
                .size(TS.section)
                .color(muted)
                .width(*width)
                .boxed(),
            None => Space::new().width(*width).boxed(),
        };
        line = line.push(cell);
    }
    column![
        container(line)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(header_band_style(tokens))
            .boxed(),
        rule(tokens)
    ]
    .width(Length::Fill)
    .boxed()
}

/// Table body row; with `on_press` the row also shows hover / pressed fills
/// (a row without a handler is inert). `_index` is the row's position; rows
/// are flat, so it no longer changes the look.
pub(crate) fn table_row<'a, M: Clone + 'a>(
    cells: Vec<(Element<'a, M>, Length)>,
    _index: usize,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let mut line: iced::widget::Row<Element<'a, M>> =
        row![].spacing(SP.m).align_y(iced::Alignment::Center);
    for (cell, width) in cells {
        line = line.push(container(cell).width(width).boxed());
    }
    let body = container(line.boxed())
        .padding([SP.s, SP.m])
        .width(Length::Fill);
    let item: Element<'a, M> = if on_press.is_none() {
        let fill = row_background(tokens, button::Status::Active);
        body.style(move |_: &Theme| container::Style {
            background: fill,
            ..container::Style::default()
        })
        .boxed()
    } else {
        button(body)
            .padding(0)
            .width(Length::Fill)
            .on_press_maybe(on_press)
            .style(row_style(tokens))
            .boxed()
    };
    column![
        item,
        container(Space::new())
            .width(Length::Fill)
            .height(HAIRLINE)
            .style(row_separator_style(tokens))
            .boxed()
    ]
    .width(Length::Fill)
    .boxed()
}

// ─── Buttons ─────────────────────────────────────────────────

fn labelled<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Element<'a, M> {
    button(text(raw(label)).size(TS.body).boxed())
        .padding([SP.xs, SP.m])
        .on_press_maybe(on_press)
        .style(style)
        .boxed()
}

/// Primary action (accent fill). `None` disables it.
pub(crate) fn primary_button<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    labelled(label, on_press, styles::primary_button(tokens))
}

/// Secondary action (outline). `None` disables it.
pub(crate) fn secondary_button<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    labelled(label, on_press, graphite_outline(tokens))
}

/// Graphite secondary button: the outline look at the button radius.
fn graphite_outline(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let base = styles::outline_button(tokens);
    move |theme: &Theme, status| {
        let mut style = base(theme, status);
        style.border.radius = Radii::GRAPHITE.button.into();
        style
    }
}

/// Destructive action (error-coloured outline). `None` disables it.
pub(crate) fn danger_button<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    labelled(label, on_press, danger_button_style(tokens))
}

/// Small filled label (role / source badge, tab count): `fill` behind
/// `ink` text, corners at [`RADIUS`].
pub(crate) fn pill<'a, M: 'a>(label: String, fill: Color, ink: Color) -> Element<'a, M> {
    pill_with_radius(label, fill, ink, RADIUS)
}

/// Status chip in a [`styles::PillTone`], fully rounded.
pub(crate) fn toned_pill<'a, M: 'a>(
    label: String,
    tone: styles::PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let (fill, ink) = styles::pill_colors(tokens, tone);
    pill_with_radius(label, fill, ink, Radii::GRAPHITE.pill)
}

fn pill_with_radius<'a, M: 'a>(
    label: String,
    fill: Color,
    ink: Color,
    radius: f32,
) -> Element<'a, M> {
    container(text(raw(label)).size(TS.caption).color(ink).boxed())
        .padding([SP.xs, SP.s])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The one row-action convention: ↑ / ↓ (disabled when `None`, i.e. at
/// the ends of the list) and a danger "Remove".
pub(crate) fn reorder_remove<'a, M: Clone + 'a>(
    up: Option<M>,
    down: Option<M>,
    remove: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    row![
        secondary_button("↑", up, tokens),
        secondary_button("↓", down, tokens),
        danger_button("Remove", Some(remove), tokens),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Display label of a lifecycle state, as shown on its pill.
pub(crate) fn lifecycle_label(state: signex_library::LifecycleState) -> &'static str {
    use signex_library::LifecycleState as L;
    match state {
        L::Draft => "Draft",
        L::InReview => "In review",
        L::Released => "Released",
        L::Deprecated => "Deprecated",
        L::Obsolete => "Obsolete",
        _ => "Unknown",
    }
}

/// Pill tone of a lifecycle state.
pub(crate) fn lifecycle_tone(state: signex_library::LifecycleState) -> styles::PillTone {
    use signex_library::LifecycleState as L;
    match state {
        L::Released => styles::PillTone::Ok,
        L::InReview => styles::PillTone::Info,
        L::Deprecated => styles::PillTone::Warn,
        L::Obsolete => styles::PillTone::Err,
        _ => styles::PillTone::Neutral,
    }
}

// ─── Empty state ─────────────────────────────────────────────

/// Centred message with an optional call-to-action under it.
pub(crate) fn empty_state<'a, M: 'a>(
    message: &'a str,
    action: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let mut col: iced::widget::Column<Element<'a, M>> = column![
        text(raw(message))
            .size(TS.label)
            .color(theme_ext::text_secondary(tokens))
            .boxed()
    ]
    .spacing(SP.m)
    .align_x(iced::Alignment::Center);
    if let Some(a) = action {
        col = col.push(a);
    }
    container(col.boxed())
        .padding(SP.xl)
        .center_x(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn bg(b: Option<Background>) -> Option<iced::Color> {
        match b {
            Some(Background::Color(c)) => Some(c),
            _ => None,
        }
    }

    #[test]
    fn rows_are_flat_on_every_builtin() {
        // No zebra: rows are flat with a soft separator.
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let flat = row_background(&t, button::Status::Active);
            assert!(flat.is_none(), "{id:?}");
        }
    }

    #[test]
    fn hover_differs_from_active_and_from_pressed() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let active = bg(row_background(&t, button::Status::Active));
            let hover = bg(row_background(&t, button::Status::Hovered));
            let pressed = bg(row_background(&t, button::Status::Pressed));
            assert!(hover.is_some() && hover != active, "{id:?}");
            assert_ne!(hover, pressed, "{id:?}");
        }
    }

    #[test]
    fn card_and_rule_use_the_token_border() {
        let t = theme_tokens(ThemeId::BUILTINS[0]);
        let border = theme_ext::border_color(&t);
        let card = card_style(&t)(&Theme::Dark);
        assert_eq!(card.border.color, border);
        assert_eq!(bg(rule_style(&t)(&Theme::Dark).background), Some(border));
    }

    #[test]
    fn danger_button_is_error_coloured_and_dims_when_disabled() {
        let t = theme_tokens(ThemeId::BUILTINS[0]);
        let style = danger_button_style(&t);
        let active = style(&Theme::Dark, button::Status::Active);
        let disabled = style(&Theme::Dark, button::Status::Disabled);
        assert_eq!(active.text_color, tokens::danger(&t));
        assert_ne!(active.text_color, disabled.text_color);
        assert_ne!(
            bg(style(&Theme::Dark, button::Status::Hovered).background),
            bg(active.background)
        );
    }
}
