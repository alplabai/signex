//! The "Create Sheet" dialog (#1275): where to write the file of a sheet
//! symbol whose file does not exist yet. A confirm-class dialog: it opens in
//! its own window and locks the main window (docs/DETACHED_MODALS.md).
//!
//! Two states: a new file (Create is the Enter action), and a typed name
//! that already exists ("Use existing file" is offered beside a disabled
//! Create, and Enter does nothing, so picking another name stays the safe
//! default). Design: docs/HIERARCHY.md, "Creating the child sheet".

use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_i18n::RenderTranslatable;
use signex_i18n::{Localized, UserText, ids, t};
use signex_widgets::{text, text_input};

use super::widgets::modal_title_header;
use super::*;
use crate::app::contracts::CreateChildSheetConfirmState;
use crate::styles::{
    CalloutTone, DialogButtonKind, dialog_body, dialog_button, dialog_callout, dialog_caption,
    dialog_field_label, dialog_footer_split,
};

fn msg(m: ProjectMsg) -> Message {
    Message::Project(m)
}

impl Signex {
    pub(in crate::app::view) fn view_create_child_sheet_confirm(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(
            self.view_create_child_sheet_body(),
            super::sizes::CREATE_CHILD_SHEET,
        )
    }

    /// The dialog body: the card in the main window, or the content of its
    /// own window (no header; the native title bar has the title).
    pub(in crate::app::view) fn view_create_child_sheet_body(&self) -> Element<'_, Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let later = msg(ProjectMsg::CreateChildSheetAnswered { create: false });

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_TITLE).into_string(),
            later.clone(),
            tokens,
        );
        let modal = super::super::super::state::ModalId::CreateChildSheet;
        let header = self.modal_header(header_content, modal);

        let question = text(t!(
            i18n,
            ids::DIALOG_CREATE_CHILD_SHEET_QUESTION,
            "name" => confirm.name.as_str()
        ))
        .size(TS.body)
        .color(crate::styles::tokens::resolve(tokens).ink3)
        .boxed();
        let intro = crate::styles::dialog_intro_with(
            crate::styles::DialogIcon::NewSheet,
            t!(
                i18n,
                ids::DIALOG_CREATE_CHILD_SHEET_INTRO,
                "name" => confirm.name.as_str()
            )
            .into_string(),
            Some(question),
            tokens,
        );

        let mut body = column![intro, self.create_child_sheet_file_row(confirm)].spacing(SP.m);
        if let Some(error) = &confirm.error {
            body = body.push(callout(CalloutTone::Error, error.localized(i18n), tokens));
        }
        if let Some(warning) = &confirm.reuse_warning {
            body = body.push(callout(CalloutTone::Warn, warning.localized(i18n), tokens));
        }
        if confirm.error.is_none() && !confirm.exists {
            body = body.push(dialog_caption(
                t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_FILE_NOTE),
                tokens,
            ));
        }

        // In the main window the card is its design width and as tall as its
        // content; in its own window it fills that window.
        let width = self.dialog_width(modal, super::sizes::CREATE_CHILD_SHEET);
        let height = if self.modal_detached(modal) {
            Length::Fill
        } else {
            Length::Shrink
        };
        container(
            column![
                header,
                dialog_body(body),
                dialog_footer_split(
                    vec![dialog_caption(
                        t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_LATER_HINT),
                        tokens
                    )],
                    footer_buttons(confirm, later, i18n, tokens),
                ),
            ]
            .width(Length::Fill)
            .height(height),
        )
        .width(width)
        .height(height)
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// The "File" label with the folder the path is relative to on the
    /// right (the full folder in its tooltip), over the path field and
    /// Browse.... The field submits Create only while the typed name is new.
    fn create_child_sheet_file_row<'a>(
        &'a self,
        confirm: &'a CreateChildSheetConfirmState,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let parent_dir = confirm.parent.parent().unwrap_or(std::path::Path::new(""));
        let full_folder = parent_dir.display().to_string();
        let fold = crate::app::state::project_case::fold_of(
            self.document_state.document_project(&confirm.parent),
        );
        let folder = match crate::app::handlers::canvas::relative_within(
            &confirm.project_dir,
            parent_dir,
            fold,
        ) {
            Some(rel) if rel.as_os_str().is_empty() => {
                t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_RELATIVE_TO_PROJECT)
            }
            Some(rel) => t!(
                i18n,
                ids::DIALOG_CREATE_CHILD_SHEET_RELATIVE_TO,
                "folder" => format!("{}/", rel.display())
            ),
            None => t!(
                i18n,
                ids::DIALOG_CREATE_CHILD_SHEET_RELATIVE_TO,
                "folder" => full_folder.clone()
            ),
        };
        let label_line = row![
            dialog_field_label(t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_FILE_LABEL), tokens),
            Space::new().width(Length::Fill),
            signex_widgets::tooltip(
                dialog_caption(folder, tokens),
                text(UserText::new(full_folder)).size(TS.caption),
                iced::widget::tooltip::Position::Bottom,
            ),
        ]
        .align_y(iced::Alignment::Center);

        let mut field = text_input(
            t!(i18n, ids::PLACEHOLDER_CHILD_SHEET_PATH),
            &confirm.path_text,
        )
        .style(crate::panels::form_look::text_input_style)
        .on_input(|t| msg(ProjectMsg::CreateChildSheetPathChanged(t)))
        .font(super::graphite::mono())
        .padding([6, 8])
        .size(TS.label)
        .width(Length::Fill);
        // A name that exists: Enter does nothing and focus stays here, so
        // the user picks another name or chooses "Use existing file".
        if !confirm.exists {
            field = field.on_submit(msg(ProjectMsg::CreateChildSheetAnswered { create: true }));
        }
        column![
            label_line,
            row![
                field,
                dialog_button(
                    t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_BROWSE),
                    DialogButtonKind::Secondary,
                    Some(msg(ProjectMsg::CreateChildSheetBrowse)),
                    tokens,
                ),
            ]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
        ]
        .boxed()
    }
}

/// Later, then "Use existing file" when the typed name exists, then Create
/// (disabled while the typed name exists).
fn footer_buttons<'a>(
    confirm: &CreateChildSheetConfirmState,
    later: Message,
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Vec<Element<'a, Message>> {
    let mut buttons = vec![dialog_button(
        t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_LATER),
        DialogButtonKind::Cancel,
        Some(later),
        tokens,
    )];
    if confirm.exists {
        buttons.push(dialog_button(
            t!(i18n, ids::DIALOG_CREATE_CHILD_SHEET_OPEN_EXISTING),
            DialogButtonKind::Secondary,
            Some(msg(ProjectMsg::CreateChildSheetOpenExisting)),
            tokens,
        ));
    }
    let create =
        (!confirm.exists).then(|| msg(ProjectMsg::CreateChildSheetAnswered { create: true }));
    buttons.push(dialog_button(
        t!(i18n, ids::DIALOG_BUTTON_CREATE),
        DialogButtonKind::Primary,
        create,
        tokens,
    ));
    buttons
}

/// A callout under the field holding `message` in the tone's ink.
fn callout<'a>(
    tone: CalloutTone,
    message: Localized,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    dialog_callout(tone, text(message).size(TS.body).boxed(), tokens)
}
