//! The Close Project confirm with unsaved edits (#2262): the intro counts the
//! unsaved documents and names unsaved project settings apart, the list shows
//! one row per entry (kind icon, name, kind, dirty dot), and info callouts say
//! which documents stay open and why Discard All is withheld. Discard All
//! stands alone at the left of the footer.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::*;
use iced::widget::{Space, column, container};
use iced::{Element, Length};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::widgets::modal_title_header;
use crate::app::ProjectCloseChoice;
use crate::app::file_extension::has_extension;
use crate::app::handlers::modal_sizing::CLOSE_LIST_MAX_HEIGHT;
use crate::app::state::ModalId;
use crate::app::state::sheet_refs::{bounded_names, file_label};
use crate::styles::{
    CalloutTone, DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_callout,
    dialog_caption, dialog_footer_split, dialog_hint, dialog_intro, dialog_row, dialog_rows,
    dirty_dot, file_kind_icon, ti,
};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_project_close_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_project_close_confirm_body();
        self.wrap_centred_dialog(dialog, super::sizes::PROJECT_CLOSE_CONFIRM)
    }

    pub(in crate::app::view) fn view_project_close_confirm_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.project_close_confirm else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let choose = |choice| Message::Project(ProjectMsg::CloseConfirm(choice));

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_CLOSE_PROJECT_UNSAVED_TITLE).into_string(),
            choose(ProjectCloseChoice::Cancel),
            tokens,
        );
        let header = self.modal_header(header_content, ModalId::ProjectCloseConfirm);

        // An unreadable sheet hides its children: no Discard All, and why.
        let discard_blocked = st.sheet_refs.discard_blocked_reason();

        let mut body = column![
            dialog_intro(
                DialogIcon::Warn,
                t!(i18n, ids::DIALOG_CLOSE_PROJECT_QUESTION).into_string(),
                Some(close_detail(
                    i18n,
                    &st.project_name,
                    &st.dirty_paths,
                    st.dirty_rows.len(),
                )),
                tokens,
            ),
            dialog_rows(
                None,
                self.close_rows(i18n, &st.dirty_paths, &st.dirty_rows, tokens),
                Some(CLOSE_LIST_MAX_HEIGHT),
                tokens,
            ),
        ]
        .spacing(SP.m);
        // Kept documents stay open and are never discarded; say so, by name.
        if !st.kept_paths.is_empty() {
            body = body.push(dialog_callout(
                CalloutTone::Info,
                text(kept_line(i18n, &st.kept_paths)).size(TS.body).boxed(),
                tokens,
            ));
        }
        let mut extras = Vec::new();
        match discard_blocked {
            Some(reason) => {
                body = body.push(dialog_callout(
                    CalloutTone::Info,
                    dialog_hint(reason, tokens),
                    tokens,
                ));
            }
            None => extras.push(dialog_button(
                t!(i18n, ids::DIALOG_UNSAVED_DISCARD_ALL),
                DialogButtonKind::Danger,
                Some(choose(ProjectCloseChoice::DiscardAll)),
                tokens,
            )),
        }
        let buttons = vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(choose(ProjectCloseChoice::Cancel)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_SAVE_ALL),
                DialogButtonKind::Primary,
                Some(choose(ProjectCloseChoice::SaveAll)),
                tokens,
            ),
        ];

        let width = self.dialog_width(
            ModalId::ProjectCloseConfirm,
            super::sizes::PROJECT_CLOSE_CONFIRM,
        );
        container(
            column![
                header,
                dialog_body(body),
                dialog_footer_split(extras, buttons)
            ]
            .width(width),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// One row per unsaved entry: the files first, then Component Preview
    /// rows by part number. Names only: paths would overflow the modal and
    /// the project already implies the folder; a symbol or footprint draft
    /// is named relative to its library's folder, so same-named drafts differ.
    fn close_rows(
        &self,
        i18n: &I18n,
        paths: &[PathBuf],
        rows: &[crate::library::state::EditorAddress],
        tokens: &ThemeTokens,
    ) -> Vec<Element<'_, Message>> {
        let roots = self.draft_library_roots();
        let drafts: HashSet<PathBuf> = self.document_state.unsaved_drafts().into_iter().collect();
        let files = paths.iter().map(|path| {
            let dir = drafts.contains(path).then(|| {
                let fold = self.draft_fold(path);
                // Libraries sharing a folder share its relative label.
                crate::app::state::scope::owning_libraries(path, &roots, fold)
                    .first()
                    .and_then(|lib| lib.parent())
                    .map(Path::to_path_buf)
            });
            let name =
                crate::library::close_prompt::dirty_path_label(path, dir.flatten().as_deref());
            close_row(path, name, row_kind(path), i18n, tokens)
        });
        let components = rows.iter().map(|address| {
            close_row(
                &address.library_path,
                self.library.editor_label(address),
                ids::DIALOG_CLOSE_PROJECT_KIND_COMPONENT,
                i18n,
                tokens,
            )
        });
        files.chain(components).collect()
    }
}

/// A list row: the file's kind icon, its name (cut with an ellipsis), what
/// it is, and the dirty dot.
fn close_row<'a>(
    icon_path: &Path,
    name: String,
    kind: MessageId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    dialog_row(vec![
        file_kind_icon(icon_path),
        text(UserText::new(name))
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill)
            .boxed(),
        dialog_caption(t!(i18n, kind), tokens),
        dirty_dot(tokens),
    ])
}

/// What an unsaved file is, by its extension.
fn row_kind(path: &Path) -> MessageId {
    const KINDS: [(&str, MessageId); 6] = [
        ("snxsch", ids::DIALOG_CLOSE_PROJECT_KIND_SCHEMATIC),
        ("snxpcb", ids::DIALOG_CLOSE_PROJECT_KIND_PCB),
        ("snxprj", ids::DIALOG_CLOSE_PROJECT_KIND_SETTINGS),
        ("snxlib", ids::DIALOG_CLOSE_PROJECT_KIND_LIBRARY),
        ("snxsym", ids::DIALOG_CLOSE_PROJECT_KIND_SYMBOL),
        ("snxfpt", ids::DIALOG_CLOSE_PROJECT_KIND_FOOTPRINT),
    ];
    KINDS
        .iter()
        .find(|(extension, _)| has_extension(path, extension))
        .map_or(ids::DIALOG_CLOSE_PROJECT_KIND_FILE, |(_, kind)| *kind)
}

/// The intro's second line: every unsaved row is counted, the project's own
/// settings (its `.snxprj`) named apart. Library rows make the count "items".
fn close_detail(i18n: &I18n, project: &str, paths: &[PathBuf], rows: usize) -> String {
    let is_settings = |p: &&PathBuf| has_extension(p, "snxprj");
    let settings = paths.iter().any(|p| is_settings(&p));
    let documents = paths.len() - paths.iter().filter(is_settings).count();
    if settings && documents == 0 && rows == 0 {
        return t!(i18n, ids::DIALOG_CLOSE_PROJECT_DETAIL_SETTINGS_ONLY, "project" => project)
            .into_string();
    }
    let id = if settings {
        ids::DIALOG_CLOSE_PROJECT_DETAIL_SETTINGS
    } else {
        ids::DIALOG_CLOSE_PROJECT_DETAIL
    };
    super::confirms::unsaved_line(i18n, id, documents, rows, Some(project))
}

/// The documents that stay open: another open project uses them, or that
/// cannot be ruled out (`CloseScan::kept`).
fn kept_line(i18n: &I18n, kept: &[PathBuf]) -> Localized {
    let names: Vec<String> = kept.iter().map(|p| file_label(p)).collect();
    t!(
        i18n,
        ids::DIALOG_CLOSE_PROJECT_KEPT,
        "count" => kept.len(),
        "names" => bounded_names(&names)
    )
}

#[cfg(test)]
#[path = "close_project_tests.rs"]
mod tests;
