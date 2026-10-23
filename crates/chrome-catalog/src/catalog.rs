//! Chrome catalog MVU composition root.

use iced::widget::{column, container, scrollable};
use iced::{Background, Element, Length, Theme};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::passive_calculator::{CalculatorControl, CalculatorMessage};
use signex_widgets::trace_calculator::{TraceCalculatorControl, TraceCalculatorMessage};

use crate::{bom_modal, modal_card, project_tree, section, tabs, theme, theme_picker};

pub(crate) fn run() -> iced::Result {
    iced::application(Catalog::new, Catalog::update, Catalog::view)
        .title("Signex Chrome Catalog")
        .theme(|state: &Catalog| state.iced_theme())
        .window_size((1200.0, 900.0))
        .run()
}

struct Catalog {
    theme: ThemeId,
    tokens: ThemeTokens,
    passive_calculator: CalculatorControl,
    trace_calculator: TraceCalculatorControl,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    SelectTheme(ThemeId),
    PassiveCalculator(CalculatorMessage),
    TraceCalculator(TraceCalculatorMessage),
}

impl Catalog {
    fn new() -> Self {
        Self {
            theme: ThemeId::Signex,
            tokens: theme_tokens(ThemeId::Signex),
            passive_calculator: CalculatorControl::default(),
            trace_calculator: TraceCalculatorControl::default(),
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SelectTheme(theme_id) => {
                self.theme = theme_id;
                self.tokens = theme_tokens(theme_id);
            }
            Message::PassiveCalculator(message) => self.passive_calculator.update(message),
            Message::TraceCalculator(message) => {
                // The failure it returns is for the app's log window; the
                // catalog has none, and the control draws it anyway.
                let _shown_in_control = self.trace_calculator.update(message);
            }
        }
    }

    fn iced_theme(&self) -> Theme {
        match self.theme {
            ThemeId::SolarizedLight => Theme::Light,
            _ => Theme::Dark,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tokens = &self.tokens;
        let body = column![
            section::view(
                "Document tabs (3 visible, middle is active)",
                tokens,
                tabs::document_strip(tokens),
            ),
            section::view(
                "Document tabs — every state side-by-side",
                tokens,
                tabs::state_matrix(tokens),
            ),
            section::view(
                "Panel tabs (same widget; should match doc tabs)",
                tokens,
                tabs::panel_strip(tokens),
            ),
            section::view("Modal card chrome", tokens, modal_card::view(tokens)),
            section::view(
                "Project tree leaf indicators",
                tokens,
                project_tree::view(tokens),
            ),
            section::view(
                "BOM modal — Altium-style layout (table + properties sidebar)",
                tokens,
                bom_modal::view(tokens),
            ),
            section::view(
                "Passive network calculator control",
                tokens,
                self.passive_calculator
                    .view(tokens)
                    .map(Message::PassiveCalculator),
            ),
            section::view(
                "PCB trace calculator control",
                tokens,
                self.trace_calculator
                    .view(tokens)
                    .map(Message::TraceCalculator),
            ),
        ]
        .spacing(20)
        .padding(20);

        let content = column![
            theme_picker::view(self.theme, tokens),
            scrollable(body).width(Length::Fill).height(Length::Fill),
        ];
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(theme::color(tokens.bg))),
                ..container::Style::default()
            })
            .into()
    }
}
