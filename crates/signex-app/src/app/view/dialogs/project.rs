//! Editor modals — the Custom Selection Filter. Rename, Enable Version
//! Control and Grid Properties live in `rename.rs`, `version_control.rs` and
//! `grid_properties/`.
//!
//! Methods of the same `Signex` view impl, split across sibling files
//! (ADR-0001, issue #164).

use super::*;
use crate::library::editor::footprint::SelectionFilterKind;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{Label, text};

use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer_split};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// Custom Selection Filter modal: a 2-column grid of toggle tiles for
    /// the eight footprint kinds, All / None, a preset name, then Cancel /
    /// Apply. Apply writes the draft into `editor.state.selection_filter`
    /// and saves it as a preset under the typed name.
    pub(in crate::app::view) fn view_selection_filter_custom_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_selection_filter_custom_body();
        self.wrap_dialog(
            crate::app::state::ModalId::SelectionFilterCustom,
            dialog,
            super::sizes::SELECTION_FILTER_CUSTOM,
        )
    }

    pub(in crate::app::view) fn view_selection_filter_custom_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.selection_filter_custom else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let text_c = crate::styles::ti(tokens.text);
        let border_c = crate::styles::ti(tokens.border);
        let r = crate::styles::tokens::resolve(tokens);
        let msg = Message::SelectionFilter;

        let header_content: Element<'_, Message> = super::header::graphite_header(
            t!(i18n, ids::DIALOG_SELECTION_FILTER_TITLE).into_string(),
            Some(t!(i18n, ids::DIALOG_SELECTION_FILTER_SUBTITLE).into_string()),
            None,
            msg(SelectionFilterMsg::CloseCustom),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::SelectionFilterCustom,
        );

        // "Selectable kinds" with All / None beside it.
        let heading = row![
            text(t!(i18n, ids::DIALOG_SELECTION_FILTER_INTRO))
                .size(TS.section)
                .style(crate::styles::section_label(tokens))
                .width(Length::Fill),
            grid_segment(
                t!(i18n, ids::DIALOG_SELECTION_FILTER_ALL),
                false,
                msg(SelectionFilterMsg::CustomKindsAllSet(true)),
                text_c,
                border_c,
                tokens,
            ),
            grid_segment(
                t!(i18n, ids::DIALOG_SELECTION_FILTER_NONE),
                false,
                msg(SelectionFilterMsg::CustomKindsAllSet(false)),
                text_c,
                border_c,
                tokens,
            ),
        ]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);

        // Two columns of toggle tiles, in the dialog's kind order.
        let tiles: Vec<Element<'_, Message>> = crate::app::SelectionFilterCustomState::SHOWN
            .iter()
            .filter_map(|&kind| {
                kind_label(kind).map(|label| kind_tile(kind, label, st.is_on(kind), tokens, i18n))
            })
            .collect();
        let mut grid = column![].spacing(SP.s);
        let mut tiles = tiles.into_iter();
        while let Some(left) = tiles.next() {
            let right = tiles
                .next()
                .unwrap_or_else(|| Space::new().width(Length::Fill).boxed());
            grid = grid.push(row![left, right].spacing(SP.s).boxed());
        }

        let preset = row![
            text(t!(i18n, ids::DIALOG_SELECTION_FILTER_PRESET_NAME))
                .size(TS.label)
                .color(r.ink3)
                .width(Length::Fixed(TS.label * PRESET_LABEL_EM)),
            signex_widgets::text_input(
                t!(i18n, ids::DIALOG_SELECTION_FILTER_PRESET_NAME),
                st.preset_name.as_str(),
            )
            .style(crate::panels::form_look::text_input_style)
            .size(TS.label)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .on_input(move |name| msg(SelectionFilterMsg::CustomPresetNameEdited(name)))
            .on_submit(msg(SelectionFilterMsg::ApplyCustom)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);

        let body = column![
            heading,
            grid,
            preset,
            text(t!(i18n, ids::DIALOG_SELECTION_FILTER_HELP))
                .size(TS.caption)
                .color(r.ink3),
        ]
        .spacing(SP.m);

        let count = st.shown_on_count();
        let total = crate::app::SelectionFilterCustomState::SHOWN.len();
        let selectable = text(t!(
            i18n,
            ids::DIALOG_SELECTION_FILTER_SELECTABLE,
            "count" => count,
            "total" => total
        ))
        .size(TS.caption)
        .color(r.ink3)
        .boxed();

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer_split(
                    vec![selectable],
                    vec![
                        dialog_button(
                            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                            DialogButtonKind::Cancel,
                            Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom)),
                            tokens
                        ),
                        dialog_button(
                            t!(i18n, ids::DIALOG_BUTTON_APPLY),
                            DialogButtonKind::Primary,
                            Some(msg(SelectionFilterMsg::ApplyCustom)),
                            tokens
                        ),
                    ]
                ),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::SelectionFilterCustom,
                super::sizes::SELECTION_FILTER_CUSTOM,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// One small bordered segment button (All / None beside the Custom
/// Selection Filter heading): tinted while `active`.
fn grid_segment<'a>(
    label: impl Into<Label<'a>>,
    active: bool,
    on_press: Message,
    text_c: iced::Color,
    border_c: iced::Color,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    // The soft accent on the field fill, as the form segments.
    let bg = if active {
        crate::styles::tokens::resolve(tokens).accent_soft
    } else {
        crate::styles::tokens::resolve(tokens).field
    };
    signex_widgets::button(text(label).size(TS.caption).color(text_c))
        .padding([3, 10])
        .on_press(on_press)
        .style(move |_: &iced::Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
                ..iced::Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .boxed()
}

/// Width of the "Preset name" label, in label-size ems.
const PRESET_LABEL_EM: f32 = 7.0;

/// The catalog id naming `kind` in the Custom Selection Filter; `None` for
/// the kinds the dialog does not show.
fn kind_label(kind: SelectionFilterKind) -> Option<MessageId> {
    use SelectionFilterKind as K;
    match kind {
        K::Pads => Some(ids::DIALOG_SELECTION_FILTER_PADS),
        K::Tracks => Some(ids::DIALOG_SELECTION_FILTER_TRACKS),
        K::Arcs => Some(ids::DIALOG_SELECTION_FILTER_ARCS),
        K::Pours => Some(ids::DIALOG_SELECTION_FILTER_POURS),
        K::Bodies3d => Some(ids::DIALOG_SELECTION_FILTER_BODIES_3D),
        K::Keepouts => Some(ids::DIALOG_SELECTION_FILTER_KEEPOUTS),
        K::Cutouts => Some(ids::DIALOG_SELECTION_FILTER_CUTOUTS),
        K::Texts => Some(ids::DIALOG_SELECTION_FILTER_TEXTS),
        K::Vias | K::Regions | K::Fills | K::Other => None,
    }
}

/// Whether the canvas hit-test honours `kind` today; the rest are stored
/// for later and carry a "not wired" pill.
fn is_kind_wired(kind: SelectionFilterKind) -> bool {
    matches!(kind, SelectionFilterKind::Pads)
}

/// One toggle tile of the kind grid: checkbox, name and, for a kind the
/// canvas does not filter by yet, a "not wired" pill. On = soft accent.
fn kind_tile<'a>(
    kind: SelectionFilterKind,
    label: MessageId,
    on: bool,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, Message> {
    let toggle = Message::SelectionFilter(SelectionFilterMsg::ToggleCustomKind(kind));
    let r = crate::styles::tokens::resolve(tokens);
    let check = signex_widgets::checkbox(on)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.body)
        .on_toggle({
            let toggle = toggle.clone();
            move |_| toggle.clone()
        });
    let mut line = row![
        check,
        text(t!(i18n, label))
            .size(TS.label)
            .color(crate::styles::ti(tokens.text)),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if !is_kind_wired(kind) {
        line = line.push(
            container(text(t!(i18n, ids::DIALOG_SELECTION_FILTER_NOT_WIRED)).size(TS.caption))
                .padding([0.0, SP.s])
                .style(crate::styles::pill(
                    tokens,
                    crate::styles::PillTone::Neutral,
                ))
                .boxed(),
        );
    }
    let (fill, line_c) = if on {
        (r.accent_soft, crate::styles::ti(tokens.accent))
    } else {
        (r.field, crate::styles::ti(tokens.border))
    };
    iced::widget::mouse_area(
        container(line)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(fill)),
                border: iced::Border {
                    width: 1.0,
                    color: line_c,
                    radius: crate::styles::Radii::GRAPHITE.control.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            }),
    )
    .on_press(toggle)
    .boxed()
}

#[cfg(test)]
mod selection_filter_tests {
    use super::*;
    use crate::app::SelectionFilterCustomState as Draft;

    /// Only Pads is wired; every other shown kind gets the pill.
    #[test]
    fn only_pads_is_wired() {
        let unwired = Draft::SHOWN.iter().filter(|k| !is_kind_wired(**k)).count();
        assert_eq!(unwired, Draft::SHOWN.len() - 1);
        assert!(is_kind_wired(SelectionFilterKind::Pads));
    }

    /// Each shown kind has its own name.
    #[test]
    fn every_shown_kind_has_its_own_label() {
        let labels: std::collections::HashSet<_> =
            Draft::SHOWN.iter().filter_map(|k| kind_label(*k)).collect();
        assert_eq!(labels.len(), Draft::SHOWN.len());
    }
}
