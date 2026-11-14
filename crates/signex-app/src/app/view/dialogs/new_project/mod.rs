//! The New Project setup dialog (#2232): name and place, profile cards, the
//! folded-away Customize section and the stack-up
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

pub(crate) mod cards;
pub(crate) mod customize;
mod preview;
#[cfg(test)]
mod tests;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the preview column: about 24 em of body text.
const PREVIEW_W: f32 = TS.body * 24.0;

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
        self.wrap_centred_dialog(self.view_new_project_card(false), sizes::NEW_PROJECT)
    }

    /// The dialog in its own OS window: the window's title bar replaces the
    /// header, and the card fills the window.
    pub(in crate::app::view) fn view_new_project_window(&self) -> Element<'_, Message> {
        self.view_new_project_card(true)
    }

    /// The body always `Fill`s between the header and the footer, so only it
    /// scrolls. `windowed`: header slot empty and the card fills the window;
    /// otherwise a fixed-size card in the main window.
    fn view_new_project_card(&self, windowed: bool) -> Element<'_, Message> {
        let Some(state) = self.ui_state.new_project.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let creating = state.creating.is_some();
        let cancel = (!creating).then(|| np(NewProjectMsg::CancelRequested));
        let header = if windowed {
            super::widgets::os_title_bar_slot()
        } else {
            super::header::graphite_header_closing(
                s(i18n, ids::DIALOG_NEW_PROJECT_TITLE),
                None,
                None,
                cancel.clone(),
                super::header::DIALOG_HEADER_H,
                tokens,
            )
        };
        let left = scrollable(
            container(
                column![
                    fields(state, i18n, tokens),
                    cards::profiles(state, i18n, tokens).map(np),
                    customize::section(state, i18n, tokens).map(np),
                ]
                .spacing(SP.xl + SP.s),
            )
            .padding([SP.xl + SP.s, SP.xl + SP.m]),
        )
        .width(Length::Fill);
        let body = row![left, preview::pane(state, i18n, tokens).map(np)].height(Length::Fill);
        let hint = text(t!(i18n, ids::DIALOG_NEW_PROJECT_FOOTER_HINT))
            .size(TS.body)
            .color(tokens::muted_text(tokens))
            .boxed();
        let footer = dialog_footer_split(
            vec![hint],
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_NEW_PROJECT_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(
                        i18n,
                        if creating {
                            ids::DIALOG_NEW_PROJECT_CREATING
                        } else {
                            ids::DIALOG_NEW_PROJECT_CREATE
                        }
                    ),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_create()
                        .then(|| np(NewProjectMsg::SubmitRequested)),
                    tokens,
                ),
            ],
        );
        let (width, height) = if windowed {
            (Length::Fill, Length::Fill)
        } else {
            // The fallback card has the height the wrapper centres on, so the
            // body scrolls inside it and the footer stays on the card.
            let (w, h) =
                super::widgets::fit_modal_size(self.ui_state.window_size, sizes::NEW_PROJECT);
            (Length::Fixed(w), Length::Fixed(h))
        };
        container(column![header, body, footer].width(width).height(height))
            .width(width)
            .height(height)
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
    let name = text_input(
        t!(i18n, ids::DIALOG_NEW_PROJECT_NAME_PLACEHOLDER),
        &state.name,
    )
    .id(crate::app::new_project::NAME_INPUT_ID.clone())
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| np(NewProjectMsg::NameEdited(v)))
    .on_submit(np(NewProjectMsg::SubmitRequested))
    .size(TS.label)
    .padding(SP.m)
    .width(Length::Fill);
    let location = text_input(
        t!(i18n, ids::DIALOG_NEW_PROJECT_LOCATION_PLACEHOLDER),
        &state.location,
    )
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
        .labelled(make_folder_label(state, i18n))
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
    if let Some(why) = &state.create_error {
        col = col.push(
            text(t!(
                i18n,
                ids::DIALOG_NEW_PROJECT_ERR_CREATE_FAILED,
                "why" => why.as_str()
            ))
            .size(TS.body)
            .color(tokens::danger(tokens))
            .boxed(),
        );
    }
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

/// The folder checkbox's label: `Create "<name>" folder` with the name as
/// typed, or a neutral wording while the name is empty.
pub(crate) fn make_folder_label(state: &NewProjectState, i18n: &I18n) -> Localized {
    match state.folder_label_name() {
        "" => t!(i18n, ids::DIALOG_NEW_PROJECT_MAKE_FOLDER_UNNAMED),
        name => t!(i18n, ids::DIALOG_NEW_PROJECT_MAKE_FOLDER, "name" => name),
    }
}

/// The inline reason Create is off. A blank name is not nagged about.
fn blocker_text(state: &NewProjectState, i18n: &I18n) -> Option<Label<'static>> {
    Some(match state.blocked_by()? {
        Blocker::Name(NameProblem::Empty) => {
            t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_NAME_EMPTY).into()
        }
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
fn labelled_row<'a, M: 'a>(
    caption: Localized,
    control: Element<'a, M>,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, M> {
    row![
        text(caption).size(TS.label).color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        control
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}
