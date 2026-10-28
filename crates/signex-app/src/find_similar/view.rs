use super::{FindSimilarMsg, FindSimilarState, SimilarScope};
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const DIALOG_W: f32 = 460.0;
const DIALOG_H: f32 = 420.0;
/// The seed value column is clipped to this many characters.
const VALUE_SHOWN_CHARS: usize = 28;

fn shown(value: &str) -> String {
    if value.is_empty() {
        return "(empty)".to_owned();
    }
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(VALUE_SHOWN_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// The dialog as a centred card on a scrim in the main window.
pub fn view<'a>(state: &'a FindSimilarState, tokens: &ThemeTokens) -> Element<'a, FindSimilarMsg> {
    let text_primary = crate::styles::ti(tokens.text);
    let text_secondary = crate::styles::ti(tokens.text_secondary);
    let accent = crate::styles::ti(tokens.accent);

    let header = container(
        row![
            text("Find Similar Objects")
                .size(TS.heading)
                .color(text_primary),
            Space::new().width(Length::Fill),
            button(text("Close").size(TS.body).color(text_secondary))
                .on_press(FindSimilarMsg::Closed)
                .style(crate::styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SP.m),
    )
    .padding([10, 14])
    .style(crate::styles::toolbar_strip(tokens));

    let choice = |label: &'static str, on: bool, msg: FindSimilarMsg| {
        button(
            text(label)
                .size(TS.body)
                .color(if on { accent } else { text_secondary }),
        )
        .on_press(msg)
        .style(crate::styles::menu_item(tokens))
    };

    let mut table = column![
        row![
            text("Property")
                .size(TS.caption)
                .color(text_secondary)
                .width(130),
            text("Seed value")
                .size(TS.caption)
                .color(text_secondary)
                .width(Length::Fill),
            text("Match")
                .size(TS.caption)
                .color(text_secondary)
                .width(110),
        ]
        .spacing(10)
    ]
    .spacing(SP.s);
    for v in &state.values {
        let same = state.same.contains(&v.property);
        let toggle = FindSimilarMsg::PropertyToggled(v.property);
        table = table.push(
            row![
                text(v.property.label())
                    .size(TS.body)
                    .color(text_primary)
                    .width(130),
                text(shown(&v.value))
                    .size(TS.body)
                    .color(text_secondary)
                    .width(Length::Fill),
                row![
                    choice("Same", same, toggle.clone()),
                    choice("Any", !same, toggle),
                ]
                .width(110),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(10),
        );
    }

    let scope_row = row![
        text("Scope").size(TS.body).color(text_secondary).width(130),
        choice(
            "Current sheet",
            state.scope == SimilarScope::Sheet,
            FindSimilarMsg::ScopeChosen(SimilarScope::Sheet)
        ),
        choice(
            "Whole project",
            state.scope == SimilarScope::Project,
            FindSimilarMsg::ScopeChosen(SimilarScope::Project)
        ),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(10);

    let ok: Element<'a, FindSimilarMsg> = if state.running {
        container(text("Searching...").size(TS.body).color(text_secondary))
            .padding([6, 10])
            .into()
    } else {
        button(text("OK").size(TS.body).color(text_primary))
            .on_press(FindSimilarMsg::ApplyPressed)
            .style(crate::styles::menu_item(tokens))
            .into()
    };
    let footer = row![
        text("Selects every match. Matches on other sheets are only counted.")
            .size(TS.caption)
            .color(text_secondary),
        Space::new().width(Length::Fill),
        ok,
        button(text("Cancel").size(TS.body).color(text_secondary))
            .on_press(FindSimilarMsg::Closed)
            .style(crate::styles::menu_item(tokens)),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SP.m);

    let dialog = container(
        column![
            header,
            container(
                text(format!("Seed: {}", state.seed_label))
                    .size(TS.label)
                    .color(text_primary)
            )
            .padding([SP.l, 14.0]),
            container(scrollable(table).height(Length::Fill)).padding([0.0, 14.0]),
            container(scope_row).padding([SP.m, 14.0]),
            container(footer).padding([10, 14]),
        ]
        .width(Length::Fixed(DIALOG_W))
        .height(Length::Fixed(DIALOG_H)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_engine::{SimilarProperty, SimilarValue};

    #[test]
    fn long_and_empty_values_are_shortened_for_display() {
        assert_eq!(shown(""), "(empty)");
        assert_eq!(shown("10k"), "10k");
        let long = "x".repeat(VALUE_SHOWN_CHARS + 5);
        assert_eq!(shown(&long).chars().count(), VALUE_SHOWN_CHARS + 1);
        assert!(shown(&long).ends_with('…'));
    }

    #[test]
    fn a_new_dialog_starts_with_kind_same_and_the_rest_any() {
        let values = vec![
            SimilarValue {
                property: SimilarProperty::Kind,
                value: "Symbol".into(),
            },
            SimilarValue {
                property: SimilarProperty::Value,
                value: "10k".into(),
            },
        ];
        let state = FindSimilarState::new("Symbol R1".into(), values);
        assert_eq!(state.criteria().len(), 1);
        assert_eq!(state.criteria()[0].property, SimilarProperty::Kind);
        assert_eq!(state.scope, SimilarScope::Sheet);
    }
}
