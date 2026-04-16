use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_engine::TextTarget;
use signex_types::schematic::SelectedItem;
use signex_types::theme::ThemeTokens;

#[derive(Debug, Clone)]
pub struct FindMatch {
    pub item: SelectedItem,
    pub target: TextTarget,
    pub kind_label: String,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct FindReplaceState {
    pub open: bool,
    pub replace_mode: bool,
    pub query: String,
    pub replacement: String,
    pub matches: Vec<FindMatch>,
    pub selected_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum FindReplaceMsg {
    Close,
    QueryChanged(String),
    ReplacementChanged(String),
    SelectResult(usize),
    ReplaceCurrent,
    ReplaceAll,
}

const DIALOG_W: f32 = 720.0;
const DIALOG_H: f32 = 500.0;

pub fn view<'a>(state: &'a FindReplaceState, tokens: &ThemeTokens) -> Element<'a, FindReplaceMsg> {
    let text_primary = crate::styles::ti(tokens.text);
    let text_secondary = crate::styles::ti(tokens.text_secondary);
    let border = crate::styles::ti(tokens.border);
    let hover = crate::styles::ti(tokens.hover);
    let accent = crate::styles::ti(tokens.accent);
    let title = if state.replace_mode {
        "Find and Replace"
    } else {
        "Find"
    };

    let header = container(
        row![
            text(title).size(14).color(text_primary),
            Space::new().width(Length::Fill),
            button(text("Close").size(11).color(text_secondary))
                .on_press(FindReplaceMsg::Close)
                .style(crate::styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8),
    )
    .padding([10, 14])
    .style(crate::styles::toolbar_strip(tokens));

    let query_row = row![
        text("Find").size(11).color(text_secondary).width(80),
        text_input("Search text", &state.query)
            .on_input(FindReplaceMsg::QueryChanged)
            .padding([6, 8])
            .size(12)
            .width(Length::Fill),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(10);

    let mut form = column![query_row].spacing(10);
    if state.replace_mode {
        form = form.push(
            row![
                text("Replace").size(11).color(text_secondary).width(80),
                text_input("Replacement text", &state.replacement)
                    .on_input(FindReplaceMsg::ReplacementChanged)
                    .padding([6, 8])
                    .size(12)
                    .width(Length::Fill),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(10),
        );
    }
    form = form.push(
        container(
            text(format!("{} match(es)", state.matches.len()))
                .size(10)
                .color(text_secondary),
        )
        .padding([2, 0]),
    );

    let results: Vec<Element<'a, FindReplaceMsg>> = if state.matches.is_empty() {
        vec![container(text("No matches").size(11).color(text_secondary))
            .padding([10, 12])
            .width(Length::Fill)
            .into()]
    } else {
        state
            .matches
            .iter()
            .enumerate()
            .map(|(idx, hit)| {
                let is_active = state.selected_index == Some(idx);
                let row_bg = if is_active { Some(Background::Color(hover)) } else { None };
                button(
                    row![
                        column![
                            text(hit.kind_label.clone()).size(11).color(text_primary),
                            text(hit.text.clone()).size(10).color(text_secondary),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                    ]
                    .width(Length::Fill),
                )
                .width(Length::Fill)
                .padding([8, 10])
                .on_press(FindReplaceMsg::SelectResult(idx))
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered => Some(Background::Color(hover)),
                        _ => row_bg,
                    };
                    button::Style {
                        background: bg,
                        border: Border {
                            width: if is_active { 1.0 } else { 0.0 },
                            radius: 4.0.into(),
                            color: if is_active { accent } else { border },
                        },
                        text_color: text_primary,
                        ..button::Style::default()
                    }
                })
                .into()
            })
            .collect()
    };

    let replace_current: Element<'a, FindReplaceMsg> = if state.replace_mode && state.selected_index.is_some() {
        button(text("Replace Current").size(11).color(text_primary))
            .on_press(FindReplaceMsg::ReplaceCurrent)
            .style(crate::styles::menu_item(tokens))
            .into()
    } else {
        container(text("Replace Current").size(11).color(text_secondary))
            .padding([6, 10])
            .into()
    };

    let replace_all: Element<'a, FindReplaceMsg> = if state.replace_mode && !state.matches.is_empty() {
        button(text("Replace All").size(11).color(text_primary))
            .on_press(FindReplaceMsg::ReplaceAll)
            .style(crate::styles::menu_item(tokens))
            .into()
    } else {
        container(text("Replace All").size(11).color(text_secondary))
            .padding([6, 10])
            .into()
    };

    let footer = row![
        container(text("Targets: labels, notes, designators, values").size(10).color(text_secondary)),
        Space::new().width(Length::Fill),
        replace_current,
        replace_all,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8);

    let dialog = container(
        column![
            header,
            container(form).padding([12, 14]),
            container(scrollable(column(results).spacing(4)).height(Length::Fill))
                .padding([10, 10]),
            container(footer).padding([10, 14]),
        ]
        .width(DIALOG_W)
        .height(DIALOG_H),
    )
    .style(crate::styles::context_menu(tokens));

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                dialog,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
        ..container::Style::default()
    })
    .into()
}