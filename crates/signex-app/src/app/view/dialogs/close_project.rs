//! The Close Project confirm with unsaved edits (#2262): the intro counts the
//! unsaved documents and names unsaved project settings apart, the list is
//! the Exit confirm's grouped list (a heading per group with its count, each
//! row with its kind cap and its folder or library; mockup row 16), and info
//! callouts say which documents stay open and why Discard All is withheld.
//! Discard All stands alone at the left of the footer. The intro and footer
//! come from the shared unsaved-prompt chrome, wrapped around the grouped
//! list (mockup row 136).

use std::path::PathBuf;

use super::*;
use iced::widget::{Space, column, container};
use iced::{Element, Length};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::text;

use super::quit_confirm::group_entries;
use super::widgets::modal_title_header;
use crate::app::ProjectCloseChoice;
use crate::app::file_extension::has_extension;
use crate::app::handlers::modal_sizing::CLOSE_LIST_MAX_HEIGHT;
use crate::app::read_only_project::{read_only_note, unsaved_actions};
use crate::app::state::ModalId;
use crate::app::state::sheet_refs::{bounded_names, file_label};
use crate::library::modal_chrome::{UnsavedBody, unsaved_body, unsaved_footer_parts};
use crate::styles::{CalloutTone, dialog_callout, dialog_footer_split, dialog_hint};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

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
        // The Exit confirm's grouped list, scrolling past its cap.
        let groups = group_entries(self.quit_entries(&st.dirty_paths, &st.dirty_rows));
        let list = container(self.unsaved_list(&groups, tokens))
            .height(Length::Fit.max(CLOSE_LIST_MAX_HEIGHT))
            .boxed();

        let mut below = Vec::new();
        // Kept documents stay open and are never discarded; say so, by name.
        if !st.kept_paths.is_empty() {
            below.push(dialog_callout(
                CalloutTone::Info,
                text(kept_line(i18n, &st.kept_paths)).size(TS.body).boxed(),
                tokens,
            ));
        }
        let discard_allowed = discard_blocked.is_none();
        if let Some(reason) = discard_blocked {
            below.push(dialog_callout(
                CalloutTone::Info,
                dialog_hint(reason, tokens),
                tokens,
            ));
        }
        // A read-only project's settings are never written: Save All saves
        // the rest, and the discard drops only that project's (#2904).
        let read_only = self.read_only_offer(&st.dirty_paths, st.dirty_rows.len());
        if let Some(offer) = &read_only {
            let line = text(read_only_note(i18n, offer)).size(TS.body).boxed();
            below.push(dialog_callout(CalloutTone::Warn, line, tokens));
        }
        let body = unsaved_body(
            UnsavedBody {
                title: t!(i18n, ids::DIALOG_CLOSE_PROJECT_QUESTION).into_string(),
                detail: close_detail(i18n, &st.project_name, &st.dirty_paths, st.dirty_rows.len()),
                above: Vec::new(),
                list,
                below,
            },
            tokens,
        );
        let (extras, buttons) = unsaved_footer_parts(
            i18n,
            unsaved_actions(i18n, read_only.as_ref(), discard_allowed, choose),
            tokens,
        );

        let width = self.dialog_width(
            ModalId::ProjectCloseConfirm,
            super::sizes::PROJECT_CLOSE_CONFIRM,
        );
        container(column![header, body, dialog_footer_split(extras, buttons)].width(width))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }
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
