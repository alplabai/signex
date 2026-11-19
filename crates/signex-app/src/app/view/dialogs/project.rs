//! Editor modals — Grid Properties and the Custom Selection Filter. Rename
//! and Enable Version Control live in `rename.rs` and `version_control.rs`.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length};
use signex_i18n::{Localized, MessageId, ids, t};
use signex_widgets::{Label, raw, text};

use super::widgets::modal_title_header;
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_footer, dialog_intro,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a footprint
    /// editor). Mirrors Altium's "Cartesian Grid Editor [mm]" with a
    /// stripped-down field set: Step X / Step Y + link toggle, plus
    /// OK / Cancel. Display style + multiplier + per-grid-color land
    /// in v0.18.11.x as the underlying canvas/grid system grows them.
    pub(in crate::app::view) fn view_grid_properties_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_grid_properties_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::GridProperties,
            dialog,
            super::sizes::GRID_PROPERTIES,
        )
    }

    pub(in crate::app::view) fn view_grid_properties_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.grid_properties else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_GRID_TITLE).into_string(),
            Message::GridProperties(GridPropertiesMsg::Close),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::GridProperties,
        );

        fn mk_input<'a>(
            placeholder: signex_i18n::Localized,
            value: &'a str,
            on_input: fn(String) -> Message,
        ) -> Element<'a, Message> {
            use signex_widgets::text_input;
            text_input(placeholder, value)
                .style(crate::panels::form_look::text_input_style)
                .size(TS.label)
                .padding(6)
                .width(Length::Fixed(140.0))
                .on_input(on_input)
                .on_submit(Message::GridProperties(GridPropertiesMsg::Apply))
                .boxed()
        }

        // Display style + multiplier rows.
        use crate::library::editor::footprint::state::GridDisplay as Gd;
        let mk_display_row =
            |label: Localized, current: Gd, setter: fn(Gd) -> Message| -> Element<'_, Message> {
                let seg = move |label: Localized, target: Gd, active: bool| {
                    grid_segment(label, active, setter(target), text_c, border_c, tokens)
                };
                row![
                    container(text(label).size(TS.body).color(text_muted))
                        .width(Length::Fixed(80.0)),
                    seg(
                        t!(i18n, ids::DIALOG_GRID_LINES),
                        Gd::Lines,
                        current == Gd::Lines
                    ),
                    seg(
                        t!(i18n, ids::DIALOG_GRID_DOTS),
                        Gd::Dots,
                        current == Gd::Dots
                    ),
                    seg(
                        t!(i18n, ids::DIALOG_GRID_HIDDEN),
                        Gd::Hidden,
                        current == Gd::Hidden
                    ),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed()
            };
        let mk_mult_row = |current: u32| -> Element<'_, Message> {
            // raw: the multiplier labels are numbers, not words.
            let seg = move |label: &'static str, target: u32, active: bool| {
                let msg = Message::GridProperties(GridPropertiesMsg::SetMultiplier(target));
                grid_segment(raw(label), active, msg, text_c, border_c, tokens)
            };
            row![
                container(
                    text(t!(i18n, ids::DIALOG_GRID_MULTIPLIER))
                        .size(TS.body)
                        .color(text_muted)
                )
                .width(Length::Fixed(80.0)),
                seg("1×", 1, current == 1),
                seg("2×", 2, current == 2),
                seg("5×", 5, current == 5),
                seg("10×", 10, current == 10),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed()
        };

        let body = column![
            row![
                container(
                    text(t!(i18n, ids::DIALOG_GRID_STEP))
                        .size(TS.body)
                        .color(text_muted)
                )
                .width(Length::Fixed(80.0)),
                mk_input(t!(i18n, ids::PLACEHOLDER_GRID_STEP), &st.step_mm, |v| {
                    Message::GridProperties(GridPropertiesMsg::SetStep(v))
                }),
                text(t!(i18n, ids::DIALOG_GRID_STEP_UNIT))
                    .size(TS.caption)
                    .color(text_muted),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
            mk_display_row(t!(i18n, ids::DIALOG_GRID_FINE), st.fine_display, |d| {
                Message::GridProperties(GridPropertiesMsg::SetFineDisplay(d))
            }),
            mk_display_row(t!(i18n, ids::DIALOG_GRID_COARSE), st.coarse_display, |d| {
                Message::GridProperties(GridPropertiesMsg::SetCoarseDisplay(d))
            }),
            mk_mult_row(st.multiplier),
            text(t!(i18n, ids::DIALOG_GRID_HELP))
                .size(TS.caption)
                .color(text_muted),
        ]
        .spacing(10);
        let body = with_graphite_intro(
            DialogIcon::Settings,
            t!(i18n, ids::DIALOG_GRID_INTRO).into_string(),
            body,
            10.0,
            tokens,
        );

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::GridProperties(GridPropertiesMsg::Close)),
                        tokens
                    ),
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_APPLY),
                        DialogButtonKind::Primary,
                        Some(Message::GridProperties(GridPropertiesMsg::Apply)),
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::GridProperties,
                super::sizes::GRID_PROPERTIES,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    /// v0.18.14.1 — Custom Selection Filter modal. 8 rows of
    /// per-kind checkboxes (Pads / Tracks / Arcs / Pours / 3D Bodies
    /// / Keepouts / Cutouts / Texts) + Apply / Cancel. Apply writes
    /// the draft into `editor.state.selection_filter`.
    pub(in crate::app::view) fn view_selection_filter_custom_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_selection_filter_custom_body();
        self.wrap_dialog(
            crate::app::state::ModalId::SelectionFilterCustom,
            dialog,
            super::sizes::SELECTION_FILTER_CUSTOM,
        )
    }

    pub(in crate::app::view) fn view_selection_filter_custom_body(&self) -> Element<'_, Message> {
        use crate::library::editor::footprint::state::SelectionFilterKind as K;

        let Some(ref st) = self.ui_state.selection_filter_custom else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_SELECTION_FILTER_TITLE).into_string(),
            Message::SelectionFilter(SelectionFilterMsg::CloseCustom),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::SelectionFilterCustom,
        );

        // A real checkbox per kind; the whole row toggles it too.
        let mk_row = |label: MessageId, kind: K, on: bool| -> Element<'_, Message> {
            let toggle = Message::SelectionFilter(SelectionFilterMsg::ToggleCustomKind(kind));
            let check = signex_widgets::checkbox(on)
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.title)
                .on_toggle({
                    let toggle = toggle.clone();
                    move |_| toggle.clone()
                });
            let name = text(t!(i18n, label))
                .size(TS.body)
                .color(text_c)
                .width(Length::Fill);
            iced::widget::mouse_area(
                container(
                    row![check, Space::new().width(SP.m), name].align_y(iced::Alignment::Center),
                )
                .padding([SP.s, SP.m])
                .width(Length::Fill),
            )
            .on_press(toggle)
            .boxed()
        };

        let body = column![
            text(t!(i18n, ids::DIALOG_SELECTION_FILTER_HELP))
                .size(TS.caption)
                .color(text_muted),
            mk_row(ids::DIALOG_SELECTION_FILTER_PADS, K::Pads, st.pads),
            mk_row(ids::DIALOG_SELECTION_FILTER_TRACKS, K::Tracks, st.tracks),
            mk_row(ids::DIALOG_SELECTION_FILTER_ARCS, K::Arcs, st.arcs),
            mk_row(ids::DIALOG_SELECTION_FILTER_POURS, K::Pours, st.pours),
            mk_row(
                ids::DIALOG_SELECTION_FILTER_BODIES_3D,
                K::Bodies3d,
                st.bodies_3d
            ),
            mk_row(
                ids::DIALOG_SELECTION_FILTER_KEEPOUTS,
                K::Keepouts,
                st.keepouts
            ),
            mk_row(ids::DIALOG_SELECTION_FILTER_CUTOUTS, K::Cutouts, st.cutouts),
            mk_row(ids::DIALOG_SELECTION_FILTER_TEXTS, K::Texts, st.texts),
        ]
        .spacing(SP.s);
        let body = with_graphite_intro(
            DialogIcon::Settings,
            t!(i18n, ids::DIALOG_SELECTION_FILTER_INTRO).into_string(),
            body,
            SP.s,
            tokens,
        );

        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom)),
                        tokens
                    ),
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_APPLY),
                        DialogButtonKind::Primary,
                        Some(Message::SelectionFilter(SelectionFilterMsg::ApplyCustom)),
                        tokens
                    ),
                ]),
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

/// `body` under the shell's title-only intro.
fn with_graphite_intro<'a>(
    icon: DialogIcon,
    title: String,
    body: iced::widget::Column<Element<'a, Message>>,
    gap: f32,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    column![dialog_intro(icon, title, None, tokens), body]
        .spacing(gap)
        .boxed()
}

/// One segment button of the Grid Properties display and multiplier rows:
/// tinted while `active`.
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
