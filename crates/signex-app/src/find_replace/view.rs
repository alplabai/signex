use super::{FindReplaceMsg, FindReplaceState, FindScope};
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

const DIALOG_W: f32 = 440.0;
const DIALOG_H: f32 = 400.0;

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

    let scope_button = |label: &'static str, scope: FindScope| {
        let chosen = state.scope == scope;
        button(
            text(label)
                .size(11)
                .color(if chosen { accent } else { text_secondary }),
        )
        .on_press(FindReplaceMsg::ScopeChanged(scope))
        .style(crate::styles::menu_item(tokens))
    };
    let scope_row = row![
        text("Scope").size(11).color(text_secondary).width(80),
        scope_button("Sheet", FindScope::Sheet),
        scope_button("Project", FindScope::Project),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(10);

    let mut form = column![query_row, scope_row].spacing(10);
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
    let count = if state.replace_job.is_some() {
        "Replacing...".to_string()
    } else if state.scanning {
        "Searching...".to_string()
    } else {
        format!("{} match(es)", state.matches.len())
    };
    form = form.push(container(text(count).size(10).color(text_secondary)).padding([2, 0]));
    if let Some(note) = &state.skipped_note {
        form = form.push(text(note.clone()).size(10).color(text_secondary));
    }

    let results: Vec<Element<'a, FindReplaceMsg>> = if state.matches.is_empty() {
        vec![
            container(text("No matches").size(11).color(text_secondary))
                .padding([10, 12])
                .width(Length::Fill)
                .into(),
        ]
    } else {
        let result_row = |idx: usize, hit: &'a super::FindMatch| -> Element<'a, FindReplaceMsg> {
            let is_active = state.selected_index == Some(idx);
            let row_bg = if is_active {
                Some(Background::Color(hover))
            } else {
                None
            };
            button(
                row![
                    column![
                        text(hit.hit.kind.label()).size(11).color(text_primary),
                        text(match &hit.placement_label {
                            Some(label) => format!("{} · {label}", hit.hit.text),
                            None => hit.hit.text.clone(),
                        })
                        .size(10)
                        .color(text_secondary),
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
        };
        let mut rows: Vec<Element<'a, FindReplaceMsg>> = Vec::new();
        let mut current_sheet: Option<&std::path::Path> = None;
        for (idx, hit) in state.matches.iter().enumerate() {
            if state.scope == FindScope::Project && hit.sheet.as_deref() != current_sheet {
                current_sheet = hit.sheet.as_deref();
                let name = current_sheet
                    .and_then(|p| p.file_name())
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                rows.push(
                    container(text(name).size(11).color(accent))
                        .padding([6, 4])
                        .into(),
                );
            }
            rows.push(result_row(idx, hit));
        }
        rows
    };

    let replace_current: Element<'a, FindReplaceMsg> =
        if state.replace_mode && state.selected_index.is_some() && !state.is_busy() {
            button(text("Replace Current").size(11).color(text_primary))
                .on_press(FindReplaceMsg::ReplaceCurrent)
                .style(crate::styles::menu_item(tokens))
                .into()
        } else {
            container(text("Replace Current").size(11).color(text_secondary))
                .padding([6, 10])
                .into()
        };

    let replace_all: Element<'a, FindReplaceMsg> =
        if state.replace_mode && !state.matches.is_empty() && !state.is_busy() {
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
        container(
            text("Targets: labels, notes, designators, values")
                .size(10)
                .color(text_secondary)
        ),
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
        background: Some(Background::Color(crate::styles::tokens::scrim(0.5))),
        ..container::Style::default()
    })
    .into()
}
