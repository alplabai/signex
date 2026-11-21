//! Grid Properties (Ctrl+G in a footprint editor): one grid per footprint
//! editor, set by its step, multiplier and fine / coarse display.
//!
//! Title "Grid Properties" with the "Cartesian · mm" note, then a live
//! preview across the full dialog width ([`preview`]): the grids as they
//! will draw and a pad for scale, with the step and the coarse spacing as
//! labels the user can edit on the drawing (Enter commits, Esc closes the
//! label; text that is not a length is outlined red with a note, and Enter
//! and OK wait until it is fixed) and an expand button for a taller
//! drawing. Below it the form:
//! Step, Multiplier (with the coarse spacing), Fine display and Coarse
//! display. OK sends the shipped `GridPropertiesMsg::Apply`.

pub(crate) mod preview;
#[cfg(test)]
mod tests;

use iced::widget::{Space, column, container, row, stack};
use iced::{Element, Font, Length, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::dimension_label::DimensionLabelStyle;
use signex_widgets::property_illustration::{IllustrationHover, property_illustration};
use signex_widgets::{Label, LengthFormat, SegmentedStyle, segmented, text, text_input};

use super::*;
use crate::app::GridPropertiesState;
use crate::app::contracts::grid_properties::MULTIPLIERS;
use crate::app::state::ModalId;
use crate::library::editor::footprint::GridDisplay;
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split, ti};
use preview::PreviewSize;

/// Default type / spacing scales.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Form label column and Step field width, in body ems.
const LABEL_EM: f32 = 7.0;
const STEP_FIELD_EM: f32 = 8.0;
/// Width of the multiplier and display segments, in body ems.
const MULTIPLIER_EM: f32 = 16.5;
const DISPLAY_EM: f32 = 20.0;

fn msg(m: GridPropertiesMsg) -> Message {
    Message::GridProperties(m)
}

impl Signex {
    pub(in crate::app::view) fn view_grid_properties_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_grid_properties_dialog_body();
        self.wrap_dialog(
            ModalId::GridProperties,
            dialog,
            super::sizes::GRID_PROPERTIES,
        )
    }

    pub(in crate::app::view) fn view_grid_properties_dialog_body(&self) -> Element<'_, Message> {
        let Some(st) = self.ui_state.grid_properties.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n: &I18n = self.ui_state.i18n.as_ref();

        let header = self.modal_header(
            graphite_header(
                i18n.message_plain(ids::DIALOG_TITLE_GRID_PROPERTIES, &[]),
                Some(i18n.message_plain(ids::DIALOG_GRID_NOTE, &[])),
                None,
                msg(GridPropertiesMsg::CancelPressed),
                DIALOG_HEADER_H,
                tokens,
            ),
            ModalId::GridProperties,
        );
        let width = super::widgets::modal_card_width(
            self.ui_state.window_size,
            super::sizes::GRID_PROPERTIES.0,
        );
        let size = PreviewSize::new(width, st.preview_expanded, TS.caption);
        let footer = dialog_footer_split(
            vec![caption(t!(i18n, ids::DIALOG_GRID_FOOTER), tokens)],
            footer_buttons()
                .into_iter()
                .map(|(label, kind, action)| {
                    dialog_button(t!(i18n, label), kind, Some(msg(action)), tokens)
                })
                .collect(),
        );

        container(
            column![
                header,
                grid_preview(st, size, i18n, tokens),
                container(grid_form(st, i18n, tokens)).padding(SP.l),
                footer,
            ]
            .width(self.dialog_width(ModalId::GridProperties, super::sizes::GRID_PROPERTIES)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The footer buttons, left to right: Cancel discards, OK applies.
pub(crate) fn footer_buttons() -> [(MessageId, DialogButtonKind, GridPropertiesMsg); 2] {
    [
        (
            ids::DIALOG_BUTTON_CANCEL,
            DialogButtonKind::Cancel,
            GridPropertiesMsg::CancelPressed,
        ),
        (
            ids::DIALOG_BUTTON_OK,
            DialogButtonKind::Primary,
            GridPropertiesMsg::Apply,
        ),
    ]
}

/// The preview: the drawing, its editable labels and the expand button.
fn grid_preview<'a>(
    st: &'a GridPropertiesState,
    size: PreviewSize,
    i18n: &'a I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let drawing = property_illustration(
        preview::illustration(st, size, i18n),
        &IllustrationHover::default(),
        tokens,
        size.text_size,
    )
    .map(|_| Message::noop());
    let labels: Vec<_> = preview::labels(st, size, i18n)
        .into_iter()
        .map(|(_, label)| label.refused_ink(ti(tokens.error)))
        .collect();
    let r = crate::styles::tokens::resolve(tokens);
    let style = DimensionLabelStyle {
        ink: r.accent_text,
        fill: ti(tokens.panel_bg),
        field: r.field,
        selection: crate::styles::tokens::selection_tint(tokens),
        handle: ti(tokens.accent),
        text_size: size.text_size,
    };
    let overlay = st
        .preview_labels
        .view(&labels, style)
        .map(|m| msg(GridPropertiesMsg::PreviewLabelEdited(m)));
    let (tip, ink) = if st.preview_expanded {
        (ids::DIALOG_GRID_COLLAPSE, r.ink3)
    } else {
        (ids::DIALOG_GRID_EXPAND, r.ink3)
    };
    let expand = signex_widgets::tooltip(
        signex_widgets::button(
            container(signex_widgets::chevron::chevron(
                st.preview_expanded,
                TS.body,
                ink,
            ))
            .padding(SP.xs)
            .boxed(),
        )
        .padding(SP.xs)
        .on_press(msg(GridPropertiesMsg::PreviewExpandToggled))
        .style(crate::styles::ghost_button(tokens))
        .boxed(),
        text(t!(i18n, tip)).size(TS.caption).boxed(),
        iced::widget::tooltip::Position::Left,
    )
    .boxed();
    let corner = container(expand)
        .padding(SP.s)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right);
    let frame = container(stack![drawing, overlay, corner.boxed()])
        .width(Length::Fixed(size.width))
        .height(Length::Fixed(size.height));
    container(frame).center_x(Length::Fill).boxed()
}

/// The segmented-control look from the theme tokens.
fn segment_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = crate::styles::tokens::resolve(tokens);
    r.segmented(TS.body).fill_width(true)
}

/// One form row: the label, then `field`.
fn form_row<'a>(
    label: MessageId,
    field: Element<'a, Message>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        container(
            text(t!(i18n, label))
                .size(TS.body)
                .color(ti(tokens.text_secondary))
        )
        .width(Length::Fixed(TS.body * LABEL_EM)),
        field,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn caption<'a>(label: signex_i18n::Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(label)
        .size(TS.caption)
        .color(crate::styles::tokens::resolve(tokens).ink3)
        .boxed()
}

/// Lines / Dots / Hidden for one grid.
fn display_segments<'a>(
    current: GridDisplay,
    on_pick: fn(GridDisplay) -> GridPropertiesMsg,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let options = [
        (
            GridDisplay::Lines,
            Label::from(t!(i18n, ids::DIALOG_GRID_LINES)),
        ),
        (
            GridDisplay::Dots,
            Label::from(t!(i18n, ids::DIALOG_GRID_DOTS)),
        ),
        (
            GridDisplay::Hidden,
            Label::from(t!(i18n, ids::DIALOG_GRID_HIDDEN)),
        ),
    ];
    container(segmented(
        options,
        current,
        segment_style(tokens),
        move |d| msg(on_pick(d)),
    ))
    .width(Length::Fixed(TS.body * DISPLAY_EM))
    .boxed()
}

/// Step, Multiplier, a divider, Fine display and Coarse display.
fn grid_form<'a>(
    st: &'a GridPropertiesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let step_field = row![
        text_input(t!(i18n, ids::PLACEHOLDER_GRID_STEP), &st.step_mm)
            .style(crate::panels::form_look::text_input_style)
            .font(Font::MONOSPACE)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .width(Length::Fixed(TS.body * STEP_FIELD_EM))
            .on_input(|v| msg(GridPropertiesMsg::SetStep(v)))
            .on_submit(msg(GridPropertiesMsg::Apply)),
        caption(t!(i18n, ids::DIALOG_GRID_STEP_UNIT), tokens),
        caption(t!(i18n, ids::DIALOG_GRID_BOTH_AXES), tokens),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed();

    let multipliers = MULTIPLIERS.map(|m| {
        (
            m,
            Label::from(t!(i18n, ids::DIALOG_GRID_MULTIPLIER_OPTION, "multiplier" => m)),
        )
    });
    let mut multiplier_field = row![
        container(segmented(
            multipliers,
            st.multiplier,
            segment_style(tokens),
            |m| msg(GridPropertiesMsg::SetMultiplier(m)),
        ))
        .width(Length::Fixed(TS.body * MULTIPLIER_EM)),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(value) = coarse_caption(st, i18n) {
        multiplier_field = multiplier_field.push(caption(value, tokens));
    }

    let divider_c = crate::styles::tokens::resolve(tokens).line_soft;
    let divider = container(Space::new().height(crate::styles::HAIRLINE))
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(divider_c)),
            ..container::Style::default()
        });

    column![
        form_row(ids::DIALOG_GRID_STEP, step_field, i18n, tokens),
        form_row(
            ids::DIALOG_GRID_MULTIPLIER,
            multiplier_field.boxed(),
            i18n,
            tokens
        ),
        divider,
        form_row(
            ids::DIALOG_GRID_FINE,
            display_segments(
                st.fine_display,
                GridPropertiesMsg::SetFineDisplay,
                i18n,
                tokens
            ),
            i18n,
            tokens,
        ),
        form_row(
            ids::DIALOG_GRID_COARSE,
            display_segments(
                st.coarse_display,
                GridPropertiesMsg::SetCoarseDisplay,
                i18n,
                tokens
            ),
            i18n,
            tokens,
        ),
    ]
    .spacing(SP.m)
    .boxed()
}

/// "coarse = <step × multiplier>", the number as the step field shows it;
/// `None` while the step is not a number above zero.
pub(crate) fn coarse_caption(
    st: &GridPropertiesState,
    i18n: &I18n,
) -> Option<signex_i18n::Localized> {
    let step = st.draft_step()?;
    let coarse = step * f64::from(st.multiplier.max(1));
    let value = LengthFormat::new(i18n, Unit::Mm).number(coarse);
    Some(t!(i18n, ids::DIALOG_GRID_COARSE_VALUE, "value" => value))
}
