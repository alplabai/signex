use super::{PasteArrayMsg, PasteArrayState};
use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};

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
pub fn view<'a>(state: &'a PasteArrayState, tokens: &ThemeTokens) -> Element<'a, PasteArrayMsg> {
    let text_primary = crate::styles::ti(tokens.text);
    let text_secondary = crate::styles::ti(tokens.text_secondary);
    let verdict = state.input();

    let header = container(
        row![
            text("Paste Array").size(TS.heading).color(text_primary),
            Space::new().width(Length::Fill),
            button(text("Close").size(TS.body).color(text_secondary))
                .on_press(PasteArrayMsg::Closed)
                .style(crate::styles::menu_item(tokens)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SP.m),
    )
    .padding([10, 14])
    .style(crate::styles::toolbar_strip(tokens));

    let field = |label: String, value: &'a str, on_input: fn(String) -> PasteArrayMsg| {
        row![
            text(label).size(TS.body).color(text_primary).width(LABEL_W),
            text_input("", value)
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
            "Copies".to_owned(),
            &state.count,
            PasteArrayMsg::CountChanged
        ),
        field(
            format!("X spacing ({})", state.unit),
            &state.x_spacing,
            PasteArrayMsg::XSpacingChanged
        ),
        field(
            format!("Y spacing ({})", state.unit),
            &state.y_spacing,
            PasteArrayMsg::YSpacingChanged
        ),
        field(
            "Text increment step".to_owned(),
            &state.step,
            PasteArrayMsg::StepChanged
        ),
    ]
    .spacing(SP.m);

    let designators = row![
        text("Designators")
            .size(TS.body)
            .color(text_primary)
            .width(LABEL_W),
        pick_list(
            PasteDesignators::ALL,
            Some(state.designators),
            PasteArrayMsg::DesignatorsChosen,
        )
        .text_size(12),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SP.m);
    let fields = fields.push(designators);

    let note = match &verdict {
        Ok(_) => "Copy k sits k spacings from the original; label numbers count up by k steps."
            .to_owned(),
        Err(reason) => reason.clone(),
    };

    let ok = button(text("OK").size(TS.body).color(if verdict.is_ok() {
        text_primary
    } else {
        text_secondary
    }))
    .on_press_maybe(verdict.is_ok().then_some(PasteArrayMsg::ApplyPressed))
    .style(crate::styles::menu_item(tokens));
    let footer = row![
        text(note).size(TS.caption).color(text_secondary),
        Space::new().width(Length::Fill),
        ok,
        button(text("Cancel").size(TS.body).color(text_secondary))
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
    .into()
}
