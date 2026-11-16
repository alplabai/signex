use super::{FindSimilarMsg, FindSimilarState, SimilarScope};
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

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
pub fn view<'a>(
    state: &'a FindSimilarState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindSimilarMsg> {
    let text_primary = crate::styles::ti(tokens.text);
    let text_secondary = crate::styles::ti(tokens.text_secondary);
    let accent = crate::styles::ti(tokens.accent);

    let header = container(
        row![
            text(t!(i18n, ids::FIND_SIMILAR_TITLE))
                .size(TS.heading)
                .color(text_primary),
            Space::new().width(Length::Fill),
            button(
                text(t!(i18n, ids::DIALOG_BUTTON_CLOSE))
                    .size(TS.body)
                    .color(text_secondary)
            )
            .on_press(FindSimilarMsg::Closed)
            .style(crate::styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SP.m),
    )
    .padding([10, 14])
    .style(crate::styles::toolbar_strip(tokens));

    let choice = |label: MessageId, on: bool, msg: FindSimilarMsg| {
        button(
            text(t!(i18n, label))
                .size(TS.body)
                .color(if on { accent } else { text_secondary }),
        )
        .on_press(msg)
        .style(crate::styles::menu_item(tokens))
    };

    let mut table = column![
        row![
            text(t!(i18n, ids::FIND_SIMILAR_COL_PROPERTY))
                .size(TS.caption)
                .color(text_secondary)
                .width(130),
            text(t!(i18n, ids::FIND_SIMILAR_COL_SEED))
                .size(TS.caption)
                .color(text_secondary)
                .width(Length::Fill),
            text(t!(i18n, ids::FIND_SIMILAR_COL_MATCH))
                .size(TS.caption)
                .color(text_secondary)
                .width(110),
        ]
        .spacing(10)
    ]
    .spacing(SP.s);
    for v in &state.values {
        let same = state.same.contains(&v.property);
        // An empty value is ours to word; any other is the user's.
        let value = if v.value.is_empty() {
            text(t!(i18n, ids::FIND_SIMILAR_EMPTY))
        } else {
            text(UserText::new(shown(&v.value)))
        };
        table = table.push(
            row![
                // raw: the property's name comes from signex-engine (#2060).
                text(raw(v.property.label()))
                    .size(TS.body)
                    .color(text_primary)
                    .width(130),
                value
                    .size(TS.body)
                    .color(text_secondary)
                    .width(Length::Fill),
                row![
                    choice(
                        ids::FIND_SIMILAR_SAME,
                        same,
                        FindSimilarMsg::PropertySet(v.property, true)
                    ),
                    choice(
                        ids::FIND_SIMILAR_ANY,
                        !same,
                        FindSimilarMsg::PropertySet(v.property, false)
                    ),
                ]
                .width(110),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(10)
            .boxed(),
        );
    }

    let scope_row = row![
        text(t!(i18n, ids::FIND_SCOPE))
            .size(TS.body)
            .color(text_secondary)
            .width(130),
        choice(
            ids::FIND_SIMILAR_SCOPE_SHEET,
            state.scope == SimilarScope::Sheet,
            FindSimilarMsg::ScopeChosen(SimilarScope::Sheet)
        ),
        choice(
            ids::FIND_SIMILAR_SCOPE_PROJECT,
            state.scope == SimilarScope::Project,
            FindSimilarMsg::ScopeChosen(SimilarScope::Project)
        ),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(10);

    let ok: Element<'a, FindSimilarMsg> = if state.running {
        container(
            text(t!(i18n, ids::FIND_SEARCHING))
                .size(TS.body)
                .color(text_secondary),
        )
        .padding([6, 10])
        .boxed()
    } else {
        button(
            text(t!(i18n, ids::DIALOG_BUTTON_OK))
                .size(TS.body)
                .color(text_primary),
        )
        .on_press(FindSimilarMsg::ApplyPressed)
        .style(crate::styles::menu_item(tokens))
        .boxed()
    };
    let footer = row![
        text(t!(i18n, ids::FIND_SIMILAR_FOOTER))
            .size(TS.caption)
            .color(text_secondary),
        Space::new().width(Length::Fill),
        ok,
        button(
            text(t!(i18n, ids::DIALOG_BUTTON_CANCEL))
                .size(TS.body)
                .color(text_secondary)
        )
        .on_press(FindSimilarMsg::Closed)
        .style(crate::styles::menu_item(tokens)),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SP.m);

    // A board search reaches its own board only (#2403): no scope to pick.
    let scope: Element<'a, FindSimilarMsg> = if state.board {
        Space::new().height(0).boxed()
    } else {
        container(scope_row).padding([SP.m, 14.0]).boxed()
    };
    let dialog = container(
        column![
            header,
            container(
                // The seed is named by the selection handler, in English (#2061).
                text(t!(
                    i18n,
                    ids::FIND_SIMILAR_SEED,
                    "seed" => state.seed_label.as_str()
                ))
                .size(TS.label)
                .color(text_primary)
            )
            .padding([SP.l, 14.0]),
            container(scrollable(table).height(Length::Fill)).padding([0.0, 14.0]),
            scope,
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
    .style(crate::app::view::dialogs::scrim_style(tokens))
    .boxed()
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
        // The view shows an empty value with the catalog's same English.
        assert_eq!(
            I18n::en_us().message_plain(ids::FIND_SIMILAR_EMPTY, &[]),
            shown("")
        );
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
        let state = FindSimilarState::new("Symbol R1".into(), values, None);
        assert_eq!(state.criteria().len(), 1);
        assert_eq!(state.criteria()[0].property, SimilarProperty::Kind);
        assert_eq!(state.scope, SimilarScope::Sheet);
    }
}
