//! Chrome catalog MVU composition root.

use iced::widget::{column, container, scrollable};
use iced::{Background, Element, Length, Theme, Widget};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::passive_calculator::{CalculatorControl, CalculatorMessage};

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
    history_commits: Vec<signex_widgets::history_graph::GraphCommit>,
    /// SHA of the selected history-graph sample row.
    history_selected: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    SelectTheme(ThemeId),
    PassiveCalculator(CalculatorMessage),
    /// A project-tree sample row was interacted with; the samples are static.
    TreeInteracted,
    /// A history-graph sample row was clicked; carries its SHA.
    HistoryCommitSelected(String),
    /// A history-graph sample button was pressed; the samples are static.
    HistorySampleCommitPressed,
}

impl Catalog {
    fn new() -> Self {
        Self {
            theme: ThemeId::DEFAULT,
            tokens: theme_tokens(ThemeId::DEFAULT),
            passive_calculator: CalculatorControl::default(),
            history_commits: crate::history_graph::sample_commits(),
            history_selected: "9e01f4d".into(),
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SelectTheme(theme_id) => {
                self.theme = theme_id;
                self.tokens = theme_tokens(theme_id);
            }
            Message::PassiveCalculator(message) => self.passive_calculator.update(message),
            Message::TreeInteracted => {}
            Message::HistoryCommitSelected(sha) => self.history_selected = sha,
            Message::HistorySampleCommitPressed => {}
        }
    }

    fn iced_theme(&self) -> Theme {
        if self.theme.is_light() {
            Theme::Light
        } else {
            Theme::Dark
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tokens = &self.tokens;
        let body = column![
            crate::graphite_tokens::section(self.theme, tokens),
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
                "Enable-header subsections (on, off)",
                tokens,
                crate::enable_section::view(tokens)
            ),
            section::view(
                "Project tree leaf indicators",
                tokens,
                project_tree::view(tokens),
            ),
            section::view("Chips and mode switch", tokens, crate::chips::view(tokens)),
            section::view(
                "BOM modal — Altium-style layout (table + properties sidebar)",
                tokens,
                bom_modal::view(tokens),
            ),
            section::view("Flow progress", tokens, crate::flow_progress::view(tokens)),
            section::view(
                "Board palette and line-art inks",
                tokens,
                crate::board_palette::view(tokens)
            ),
            section::view(
                "Constraint grid and class matrix",
                tokens,
                crate::constraint_grid::view_samples(tokens)
            ),
            section::view(
                "Passive network calculator control",
                tokens,
                self.passive_calculator
                    .view(tokens)
                    .map(Message::PassiveCalculator)
                    .boxed(),
            ),
            section::view(
                "Property illustrations",
                tokens,
                crate::illustration::view(tokens)
            ),
            section::view(
                "Dimension labels",
                tokens,
                crate::dimension_label::view(tokens)
            ),
            section::view(
                "History graph (Graphite Light and Dark)",
                tokens,
                crate::history_graph::view(&self.history_commits, &self.history_selected),
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
            .boxed()
    }
}
