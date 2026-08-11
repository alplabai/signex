use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;

pub(crate) fn ti(color: signex_types::theme::Color) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0)
}

pub(crate) fn chrome_separator(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(Background::Color(border)),
        ..container::Style::default()
    }
}

pub(crate) fn toolbar_strip(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let background = ti(tokens.toolbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(background.into()),
        text_color: Some(text),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

pub(crate) fn left_toolbar(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let background = ti(tokens.toolbar_bg);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(background.into()),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

pub(super) fn status_bar(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let background = ti(tokens.statusbar_bg);
    let text = ti(tokens.text);
    let border = ti(tokens.border);
    move |_| container::Style {
        background: Some(background.into()),
        text_color: Some(text),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

pub(crate) fn rail_tab(
    tokens: &ThemeTokens,
    is_active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let active = ti(tokens.hover);
    let border = ti(tokens.border);
    move |_: &Theme, status: button::Status| {
        let background = match (is_active, status) {
            (true, _) | (false, button::Status::Hovered) => Some(Background::Color(active)),
            _ => None,
        };
        button::Style {
            background,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..button::Style::default()
        }
    }
}
