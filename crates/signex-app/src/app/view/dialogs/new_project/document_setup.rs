//! The New PCB and New Schematic setup dialogs (#2232, 2026-11-12): the New
//! Project dialog's card look and fields, for one document of a project.
//! State and update: `app/document_setup/`.

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::text;

use super::{SP, TS, cards, customize, preview, s, user_cards};
use crate::app::new_project::NewProjectMsg;
use crate::app::{DocumentSetupMsg, DocumentSetupState, Message, SetupKind, Signex};
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split, ti, tokens};

fn ds(msg: DocumentSetupMsg) -> Message {
    Message::from(msg)
}

fn edit(msg: NewProjectMsg) -> Message {
    ds(DocumentSetupMsg::Setup(msg))
}

/// Design size, in body-text units, so the card scales with the font.
fn design_size(kind: SetupKind) -> (f32, f32) {
    match kind {
        SetupKind::Pcb => (TS.body * 66.0, TS.body * 54.0),
        SetupKind::Sheet => (TS.body * 52.0, TS.body * 44.0),
    }
}

/// The intro line: which project the document goes into, and that every
/// value stays editable.
fn intro<'a>(
    state: &DocumentSetupState,
    i18n: &I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let id = match state.kind {
        SetupKind::Pcb => ids::SETUP_PCB_INTRO,
        SetupKind::Sheet => ids::SETUP_SHEET_INTRO,
    };
    text(t!(i18n, id, "project" => state.project_name.as_str()))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .boxed()
}

impl Signex {
    pub(in crate::app::view) fn view_document_setup_dialog(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.document_setup.as_ref() else {
            return container(Space::new()).boxed();
        };
        self.wrap_centred_dialog(
            self.view_document_setup_card(state),
            design_size(state.kind),
        )
    }

    fn view_document_setup_card<'a>(
        &'a self,
        state: &'a DocumentSetupState,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let users = &self.ui_state.setup_profiles;
        let draft = &state.draft;
        let (title, create) = match state.kind {
            SetupKind::Pcb => (ids::NEW_PCB_TITLE, ids::SETUP_CREATE_PCB),
            SetupKind::Sheet => (ids::SETUP_SHEET_TITLE, ids::SETUP_CREATE_SHEET),
        };
        let cancel = Some(ds(DocumentSetupMsg::CancelRequested));
        let header = super::super::header::graphite_header_closing(
            s(i18n, title),
            None,
            None,
            cancel.clone(),
            super::super::header::DIALOG_HEADER_H,
            tokens,
        );
        // A sheet takes only the user's profiles: the built-ins differ in
        // board rules alone.
        let (profiles, fields) = match state.kind {
            SetupKind::Pcb => (
                cards::profiles(draft, users, i18n, tokens),
                customize::pcb_rules(draft, i18n, tokens),
            ),
            SetupKind::Sheet => (
                user_cards::section(draft, users, i18n, tokens),
                customize::schematic(draft, i18n, tokens),
            ),
        };
        let left = scrollable(
            container(
                column![
                    intro(state, i18n, tokens),
                    profiles.map(edit),
                    fields.map(edit),
                ]
                .spacing(SP.xl),
            )
            .padding([SP.xl + SP.s, SP.xl + SP.m]),
        )
        .width(Length::Fill);
        let body: Element<'a, Message> = match state.kind {
            SetupKind::Pcb => row![
                left,
                preview::pane(draft, ids::SETUP_PCB_PREVIEW_NOTE, i18n, tokens).map(edit)
            ]
            .height(Length::Fill)
            .boxed(),
            SetupKind::Sheet => container(left).height(Length::Fill).boxed(),
        };
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
                    cancel,
                    tokens,
                ),
                dialog_button(
                    &s(i18n, create),
                    super::super::graphite::call_to_action(tokens),
                    state
                        .can_create()
                        .then(|| ds(DocumentSetupMsg::SubmitRequested)),
                    tokens,
                ),
            ],
        );
        let (w, h) = super::super::widgets::fit_modal_size(
            self.ui_state.window_size,
            design_size(state.kind),
        );
        container(
            column![header, body, footer]
                .width(Length::Fixed(w))
                .height(Length::Fixed(h)),
        )
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}
