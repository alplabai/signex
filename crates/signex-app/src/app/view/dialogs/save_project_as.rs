//! File ▸ Save Project As… (#2908): the copy's name and location (the New
//! Project fields), the `.snxprj` it lands on, what the copy takes, and
//! Cancel / Save copy. State and rules: `app/project_save_as/`.

use iced::widget::{Column, column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use crate::app::project_save_as::{NAME_INPUT_ID, SaveProjectAsMsg, SaveProjectAsState};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Design size (width, height) of the card.
const SIZE: (f32, f32) = (640.0, 460.0);

fn sa(msg: SaveProjectAsMsg) -> Message {
    Message::from(msg)
}

impl Signex {
    pub(in crate::app::view) fn save_project_as_overlay(&self) -> Option<Element<'_, Message>> {
        let state = self.ui_state.save_project_as.as_deref()?;
        Some(self.wrap_centred_dialog(self.view_save_project_as_card(state), SIZE))
    }

    fn view_save_project_as_card<'a>(
        &'a self,
        state: &'a SaveProjectAsState,
    ) -> Element<'a, Message> {
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let saving = state.saving.is_some();
        let cancel = (!saving).then(|| sa(SaveProjectAsMsg::CancelRequested));
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::DIALOG_SAVE_PROJECT_AS_TITLE).into_string(),
            Some(
                t!(
                    i18n,
                    ids::DIALOG_SAVE_PROJECT_AS_SUBTITLE,
                    "project" => state.project_name.as_str()
                )
                .into_string(),
            ),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = container(fields(state, i18n, tokens))
            .padding([SP.xl, SP.xl + SP.m])
            .width(Length::Fill)
            .height(Length::Fill);
        // Once the dialog names keys the copy cannot keep, the primary button
        // agrees to go without them; until then nothing is written.
        let (save_label, save_msg) = if saving {
            (
                ids::DIALOG_SAVE_PROJECT_AS_SAVING,
                SaveProjectAsMsg::SubmitRequested,
            )
        } else if state.not_kept.is_some() {
            (
                ids::DIALOG_SAVE_PROJECT_AS_SAVE_WITHOUT,
                SaveProjectAsMsg::KeysNotKeptAccepted,
            )
        } else {
            (
                ids::DIALOG_SAVE_PROJECT_AS_SAVE,
                SaveProjectAsMsg::SubmitRequested,
            )
        };
        let footer = dialog_footer_split(
            Vec::new(),
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, save_label),
                    super::graphite::call_to_action(tokens),
                    state.can_save().then(|| sa(save_msg)),
                    tokens,
                ),
            ],
        );
        let width = super::widgets::modal_card_width(self.ui_state.window_size, SIZE.0);
        container(column![header, body, footer].width(Length::Fixed(width)))
            .height(Length::Fixed(SIZE.1))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }
}

fn caption<'a>(i18n: &I18n, id: MessageId, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .boxed()
}

/// Name, location with Browse, the path line, what the copy takes, and why
/// Save is off.
fn fields<'a>(
    state: &'a SaveProjectAsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let name = text_input(
        t!(i18n, ids::DIALOG_NEW_PROJECT_NAME_PLACEHOLDER),
        &state.name,
    )
    .id(NAME_INPUT_ID.clone())
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| sa(SaveProjectAsMsg::NameEdited(v)))
    .on_submit(sa(SaveProjectAsMsg::SubmitRequested))
    .size(TS.label)
    .padding(SP.m)
    .width(Length::Fill);
    let location = text_input(
        t!(i18n, ids::DIALOG_NEW_PROJECT_LOCATION_PLACEHOLDER),
        &state.location,
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| sa(SaveProjectAsMsg::LocationEdited(v)))
    .on_submit(sa(SaveProjectAsMsg::SubmitRequested))
    .size(TS.label)
    .padding(SP.m)
    .width(Length::Fill);
    let browse =
        signex_widgets::button(text(t!(i18n, ids::DIALOG_NEW_PROJECT_BROWSE)).size(TS.label))
            .padding([SP.m, SP.l])
            .on_press(sa(SaveProjectAsMsg::BrowseRequested))
            .style(crate::styles::outline_button(tokens));
    let line =
        |label: signex_widgets::Label<'static>, color: iced::Color| -> Element<'a, Message> {
            text(label)
                .size(TS.body)
                .color(color)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Length::Fill)
                .boxed()
        };
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = column![
        column![
            caption(i18n, ids::DIALOG_NEW_PROJECT_NAME_LABEL, tokens),
            name
        ]
        .spacing(SP.m),
        column![
            caption(i18n, ids::DIALOG_NEW_PROJECT_LOCATION_LABEL, tokens),
            row![location, browse].spacing(SP.m)
        ]
        .spacing(SP.m),
    ]
    .spacing(SP.l);
    if state.name_is_empty() {
        col = col.push(line(
            t!(i18n, ids::DIALOG_NEW_PROJECT_ERR_NAME_EMPTY).into(),
            muted,
        ));
    } else if let Some(path) = state.project_file() {
        col = col.push(line(
            UserText::new(path.display().to_string()).into(),
            muted,
        ));
    }
    col = col.push(line(
        t!(i18n, ids::DIALOG_SAVE_PROJECT_AS_NOTE).into(),
        muted,
    ));
    let warn = ti(tokens.warning);
    if let Some(format) = &state.read_only_format {
        let note = t!(
            i18n,
            ids::DIALOG_SAVE_PROJECT_AS_READ_ONLY,
            "project" => state.project_name.as_str(),
            "format" => format.as_str()
        );
        col = col.push(line(note.into(), warn));
    }
    if state.unsaved > 0 {
        // A note, not a warning: the copy includes the edits (Caner, 2026-11-25).
        let note = t!(i18n, ids::DIALOG_SAVE_PROJECT_AS_UNSAVED, "count" => state.unsaved);
        col = col.push(line(note.into(), muted));
    }
    if let Some(keys) = &state.not_kept {
        let note = t!(
            i18n,
            ids::DIALOG_SAVE_PROJECT_AS_NOT_KEPT,
            "count" => keys.len(),
            "keys" => keys.join(", ")
        );
        col = col.push(line(note.into(), warn));
    }
    let danger = tokens::danger(tokens);
    if let Some(why) = &state.error {
        let failed = t!(i18n, ids::DIALOG_SAVE_PROJECT_AS_FAILED, "why" => why.as_str());
        col = col.push(line(failed.into(), danger));
    }
    if let Some(reason) = state
        .blocked_by()
        .and_then(|blocker| super::new_project::blocker_line(blocker, i18n))
    {
        col = col.push(line(reason, danger));
    }
    col.boxed()
}
