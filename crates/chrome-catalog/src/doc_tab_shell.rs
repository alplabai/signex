//! Document-tab shell sample (#2523): the Layer Stack tab's frame with a
//! sample toolbar, four sub-tabs (Stack active, Impedance tagged), a body
//! placeholder and a footer. Static: presses are ignored. Switch the theme
//! at the top to see Graphite Light and Dark.

use iced::widget::container;
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget};
use signex_i18n::UserText;
use signex_types::theme::{Color as ThemeColor, ThemeTokens};
use signex_widgets::chips::{ChipMetrics, ChipTone, count_text, kind_chip};
use signex_widgets::doc_tab_shell::{
    DocTabShell, DocTabShellStyle, Footer, SubTab, SubTabs, Toolbar,
};
use signex_widgets::theme_ext::TypeScale;
use signex_widgets::{Label, button, text};

use crate::catalog::Message;
use crate::theme;

const BODY: f32 = TypeScale::DEFAULT.body;
/// Height of the sample, so the body placeholder shows.
const SAMPLE_HEIGHT_EM: f32 = 20.0;

/// Sample text: fixture content, not catalog copy.
fn sample(s: &str) -> Label<'static> {
    UserText::new(s.to_owned()).into()
}

fn ext(token: Option<ThemeColor>, fallback: ThemeColor) -> Color {
    theme::color(token.unwrap_or(fallback))
}

fn ignore(_: usize) -> Message {
    Message::TreeInteracted
}

/// A small toolbar button.
fn tool<'a>(label: &str, t: &ThemeTokens) -> Element<'a, Message> {
    let ink = theme::color(t.text);
    let edge = theme::color(t.border);
    let hover = theme::color(t.hover);
    let fill = ext(t.extended.field, t.panel_bg);
    button(text(sample(label)).size(BODY).color(ink))
        .padding(Padding::from([BODY / 4.0, BODY * 2.0 / 3.0]))
        .on_press(Message::TreeInteracted)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered | button::Status::Pressed => hover,
                _ => fill,
            })),
            text_color: ink,
            border: Border {
                color: edge,
                width: 1.0,
                radius: signex_widgets::theme_ext::Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}

fn caption<'a>(label: &str, t: &ThemeTokens) -> Element<'a, Message> {
    text(sample(label))
        .size(TypeScale::DEFAULT.caption)
        .color(ext(t.extended.ink3, t.text_secondary))
        .boxed()
}

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let t = tokens;
    let style = DocTabShellStyle::from_tokens(t, BODY);
    let chips = ChipMetrics::from_text_size(BODY);
    let proposal = ChipTone {
        fill: ext(t.extended.info_soft, t.selection),
        ink: ext(t.extended.info, t.accent),
    };
    let count = ChipTone {
        fill: ext(t.extended.accent_soft, t.selection),
        ink: ext(t.extended.accent_soft_ink, t.accent),
    };

    let toolbar = Toolbar::new()
        .push(caption("Stackup  sensor-node 4L", t))
        .separator()
        .push(tool("Add", t))
        .push(tool("Move Up", t))
        .push(tool("Move Down", t))
        .separator()
        .push(tool("Presets", t))
        .push(tool("Import…", t))
        .push(tool("Export", t))
        .push_end(caption("4 copper layers · 1.586 mm", t));
    let sub_tabs = SubTabs::new(
        vec![
            SubTab::new(sample("Stack")),
            SubTab::new(sample("Materials")).tag(kind_chip(
                sample(&count_text(crate::locale::en_us(), 12)),
                count,
                chips,
            )),
            SubTab::new(sample("Finish")),
            SubTab::new(sample("Impedance")).tag(kind_chip(sample("proposal"), proposal, chips)),
        ],
        0,
    )
    .on_select(ignore);
    let line = ext(t.extended.line_strong, t.border);
    let placeholder = container(caption("Body: the stack table and cross-section", t))
        .center(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                color: line,
                width: 1.0,
                radius: signex_widgets::theme_ext::Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let body = container(placeholder)
        .padding(style.metrics.pad_x)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    let footer = Footer::new(sample("4 copper layers · 13 rows")).note(sample(
        "Changes apply to the board when you save the board.",
    ));
    let shell = DocTabShell::new(body, style)
        .toolbar(toolbar)
        .sub_tabs(sub_tabs)
        .footer(footer)
        .view();
    let edge = theme::color(t.border);
    container(shell)
        .width(Length::Fill)
        .height(BODY * SAMPLE_HEIGHT_EM)
        .style(move |_: &Theme| container::Style {
            border: Border {
                color: edge,
                width: 1.0,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn the_sample_builds_on_both_graphite_themes() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let _ = view(&theme_tokens(id));
        }
    }
}
