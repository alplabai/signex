//! The New Project setup dialog (#2232): name and place, profile cards, the
//! Signex AI mode, the folded-away Customize section and the stack-up
//! preview. Draft and rules: `app/new_project/`.

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_widgets::{CheckboxLabel as _, Label, checkbox, text, text_input};

use super::sizes;
use crate::app::new_project::{
    Blocker, NameProblem, NewProjectMsg, NewProjectState, TargetProblem,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

mod cards;
mod customize;
mod preview;
#[cfg(test)]
mod tests;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the preview column.
const PREVIEW_W: f32 = 236.0 * 11.0 / 12.0;

fn np(msg: NewProjectMsg) -> Message {
    Message::from(msg)
}

/// A catalog message as the string the dialog button takes.
fn s(i18n: &I18n, id: signex_i18n::MessageId) -> String {
    t!(i18n, id).into_string()
}

fn label<'a>(
    i18n: &I18n,
    id: signex_i18n::MessageId,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .boxed()
}

impl Signex {
    pub(in crate::app::view) fn view_new_project_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_new_project_card(), sizes::NEW_PROJECT)
    }

    fn view_new_project_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.new_project.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let header = super::header::graphite_header(
            s(i18n, ids::DIALOG_NEW_PROJECT_TITLE),
            None,
            None,
            np(NewProjectMsg::CancelRequested),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let left = scrollable(
            container(
                column![
                    fields(state, i18n, tokens),
                    cards::profiles(state, i18n, tokens),
                    cards::ai_modes(state, i18n, tokens),
                    customize::section(state, i18n, tokens),
                ]
                .spacing(SP.xl + SP.s),
            )
            .padding([SP.xl + SP.s, SP.xl + SP.m]),
        )
        .width(Length::Fill);
        let body = row![left, preview::pane(state, i18n, tokens)].height(Length::Shrink);
        let hint = text(t!(i18n, ids::DIALOG_NEW_PROJECT_FOOTER_HINT))
            .size(TS.body)
            .color(tokens::muted_text(tokens))
            .boxed();
        let footer = dialog_footer_split(
            vec![hint],
            vec![
                dialog_button(
                    &s(i18n, ids::DIALOG_NEW_PROJECT_CANCEL),
                    DialogButtonKind::Cancel,
                    Some(np(NewProjectMsg::CancelRequested)),
                    tokens,
                ),
                dialog_button(
                    &s(i18n, ids::DIALOG_NEW_PROJECT_CREATE),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_create()
                        .then(|| np(NewProjectMsg::SubmitRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::NEW_PROJECT.0),
        )))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// Project name, location with Browse, the folder checkbox and the reason
/// Create is off.
fn fields<'a>(
    state: &'a NewProjectState,
    i18n: &I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let name = text_input(UserText::new(""), &state.name)
        .id(crate::app::new_project::NAME_INPUT_ID.clone())
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| np(NewProjectMsg::NameEdited(v)))
        .on_submit(np(NewProjectMsg::SubmitRequested))
        .size(TS.label)
        .padding(SP.m)
        .width(Length::Fill);
    let location = text_input(UserText::new(""), &state.location)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| np(NewProjectMsg::LocationEdited(v)))
        .on_submit(np(NewProjectMsg::SubmitRequested))
        .size(TS.label)
        .padding(SP.m)
        .width(Length::Fill);
    let browse =
        signex_widgets::button(text(t!(i18n, ids::DIALOG_NEW_PROJECT_BROWSE)).size(TS.label))
            .padding([SP.m, SP.l])
            .on_press(np(NewProjectMsg::BrowseRequested))
            .style(crate::styles::outline_button(tokens));
    let make_folder = checkbox(state.make_folder)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.title)
        .labelled(t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_MAKE_FOLDER,
            "name" => state.folder_label_name()
        ))
        .text_size(TS.label)
        .on_toggle(|on| np(NewProjectMsg::MakeFolderToggled(on)));
    let mut col = column![
        row![
            column![
                label(i18n, ids::DIALOG_NEW_PROJECT_NAME_LABEL, tokens),
                name
            ]
            .spacing(SP.m)
            .width(Length::FillPortion(1)),
            column![
                label(i18n, ids::DIALOG_NEW_PROJECT_LOCATION_LABEL, tokens),
                row![location, browse].spacing(SP.m)
            ]
            .spacing(SP.m)
            .width(Length::FillPortion(1)),
        ]
        .spacing(SP.l + SP.s),
        make_folder,
    ]
    .spacing(SP.m);
    if let Some(reason) = blocker_text(state, i18n) {
        col = col.push(
            text(reason)
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.boxed()
}

/// The inline reason Create is off. A blank name is not nagged about.
fn blocker_text(state: &NewProjectState, i18n: &I18n) -> Option<Label<'static>> {
    Some(match state.blocked_by()? {
        Blocker::Name(NameProblem::Empty) => return None,
        Blocker::Name(NameProblem::Invalid) => {
            t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_NAME_INVALID).into()
        }
        Blocker::Target(TargetProblem::LocationEmpty) => {
            t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_LOCATION_EMPTY).into()
        }
        Blocker::Target(TargetProblem::LocationMissing) => {
            t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_LOCATION_MISSING).into()
        }
        Blocker::Target(TargetProblem::Exists(path)) => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_EXISTS,
            "path" => path.display().to_string()
        )
        .into(),
        Blocker::Target(TargetProblem::FolderNotEmpty(path)) => t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_ERR_FOLDER_NOT_EMPTY,
            "path" => path.display().to_string()
        )
        .into(),
        Blocker::Target(TargetProblem::Other(why)) => UserText::new(why).into(),
    })
}

/// A field label and its control on one line, the control right-aligned.
fn labelled_row<'a>(
    caption: Localized,
    control: Element<'a, Message>,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    row![
        text(caption).size(TS.label).color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        control
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}
