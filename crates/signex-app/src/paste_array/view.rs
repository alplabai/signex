use super::{PasteArrayMsg, PasteArrayState};
use iced::widget::{Space, Widget, column, container, pick_list, row};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, raw, text, text_input};

use crate::paste_prefs::PasteDesignators;
use iced::{Background, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const DIALOG_W: f32 = 420.0;
const LABEL_W: f32 = 150.0;

/// The dialog as a centred card on a scrim in the main window.
pub fn view<'a>(
    state: &'a PasteArrayState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PasteArrayMsg> {
    let text_primary = crate::styles::ti(tokens.text);
    let text_secondary = crate::styles::ti(tokens.text_secondary);
    let verdict = state.input();

    let header = container(
        row![
            text(t!(i18n, ids::PASTE_ARRAY_TITLE))
                .size(TS.heading)
                .color(text_primary),
            Space::new().width(Length::Fill),
            button(
                text(t!(i18n, ids::DIALOG_BUTTON_CLOSE))
                    .size(TS.body)
                    .color(text_secondary)
            )
            .on_press(PasteArrayMsg::Closed)
            .style(crate::styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SP.m),
    )
    .padding([10, 14])
    .style(crate::styles::toolbar_strip(tokens));

    let field = |label: Localized, value: &'a str, on_input: fn(String) -> PasteArrayMsg| {
        row![
            text(label).size(TS.body).color(text_primary).width(LABEL_W),
            text_input(signex_i18n::UserText::new(String::new()), value)
                .on_input(on_input)
                .on_submit_maybe(verdict.is_ok().then_some(PasteArrayMsg::ApplyPressed))
                .size(TS.body)
                .width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SP.m)
    };

    let fields = column![
        field(
            t!(i18n, ids::PASTE_ARRAY_COPIES),
            &state.count,
            PasteArrayMsg::CountChanged
        ),
        field(
            t!(i18n, ids::PASTE_ARRAY_X_SPACING, "unit" => state.unit.to_string()),
            &state.x_spacing,
            PasteArrayMsg::XSpacingChanged
        ),
        field(
            t!(i18n, ids::PASTE_ARRAY_Y_SPACING, "unit" => state.unit.to_string()),
            &state.y_spacing,
            PasteArrayMsg::YSpacingChanged
        ),
        field(
            t!(i18n, ids::PASTE_ARRAY_STEP),
            &state.step,
            PasteArrayMsg::StepChanged
        ),
    ]
    .spacing(SP.m);

    let designators = row![
        text(t!(i18n, ids::PASTE_ARRAY_DESIGNATORS))
            .size(TS.body)
            .color(text_primary)
            .width(LABEL_W),
        // The mode names come from `PasteDesignators`'s Display (#2059).
        pick_list(
            Some(state.designators),
            PasteDesignators::ALL,
            |mode: &PasteDesignators| mode.to_string(),
        )
        .graphite()
        .on_select(PasteArrayMsg::DesignatorsChosen),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SP.m);
    let fields = fields.push(designators.boxed());

    let note: Element<'a, PasteArrayMsg> = match &verdict {
        Ok(_) => text(t!(i18n, ids::PASTE_ARRAY_HINT))
            .size(TS.caption)
            .color(text_secondary)
            .boxed(),
        // raw: the input check words its own refusal, in English (#2061).
        Err(reason) => text(raw(reason.clone()))
            .size(TS.caption)
            .color(text_secondary)
            .boxed(),
    };

    let ok = button(text(t!(i18n, ids::DIALOG_BUTTON_OK)).size(TS.body).color(
        if verdict.is_ok() {
            text_primary
        } else {
            text_secondary
        },
    ))
    .on_press_maybe(verdict.is_ok().then_some(PasteArrayMsg::ApplyPressed))
    .style(crate::styles::menu_item(tokens));
    let footer = row![
        note,
        Space::new().width(Length::Fill),
        ok,
        button(
            text(t!(i18n, ids::DIALOG_BUTTON_CANCEL))
                .size(TS.body)
                .color(text_secondary)
        )
        .on_press(PasteArrayMsg::Closed)
        .style(crate::styles::menu_item(tokens)),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SP.m);

    let dialog = container(
        column![
            header,
            container(fields).padding([SP.l, 14.0]),
            container(footer).padding([10, 14]),
        ]
        .width(Length::Fixed(DIALOG_W)),
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
    .boxed()
}
