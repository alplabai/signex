//! The Exit confirm ("Quit" on macOS) when unsaved edits remain: the
//! question, one fact line, and the unsaved list grouped by project, then
//! files outside every project, then the libraries (drafts and Component
//! Preview rows). Cancel / Discard All / Save All.

use std::path::{Path, PathBuf};

use super::*;
use iced::widget::{Column, Space, column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, Localized, MessageId, Translatable, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::graphite_list::{LIST_ROW_H, group_header, kind_cap, kind_code, list_box, list_row};
use crate::app::state::ModalId;
use crate::library::state::EditorAddress;
use crate::styles::{
    DialogButtonKind, DialogIcon, chrome_px, dialog_body, dialog_button, dialog_footer_split,
};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The widest a row's folder or library name gets before it is cut, so the
/// file name keeps room (240 px at the 12 px base).
const META_MAX_W: f32 = chrome_px(240.0 * 11.0 / 12.0);

/// The group an unsaved entry is listed under. The order of the variants is
/// the order of the groups: projects (in project order), then files outside
/// every project, then the libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum QuitGroupKey {
    /// The open project at this index of `DocumentState::projects`.
    Project(usize),
    /// A file no open project owns.
    Other,
    /// A symbol or footprint draft, or a Component Preview row.
    Libraries,
}

/// One unsaved entry as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuitEntry {
    pub group: QuitGroupKey,
    /// The kind code (SCH, PCB, CMP, ...); `None` for another kind of file.
    pub kind: Option<MessageId>,
    /// File name, a draft's path in its library, or a row's part number.
    pub name: String,
    /// The muted right column: the folder of a file, the library of a
    /// draft or a row.
    pub meta: String,
    /// The library a draft or a row belongs to, counted once in the fact
    /// line.
    pub library: Option<PathBuf>,
}

/// One group of the list: its key and its entries, in list order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuitGroup {
    pub key: QuitGroupKey,
    pub entries: Vec<QuitEntry>,
}

/// `entries` grouped: one group per key that has entries, in key order;
/// the entries of a group keep the order they were given in.
pub(super) fn group_entries(mut entries: Vec<QuitEntry>) -> Vec<QuitGroup> {
    entries.sort_by_key(|entry| entry.group);
    let mut groups: Vec<QuitGroup> = Vec::new();
    for entry in entries {
        match groups.last_mut() {
            Some(group) if group.key == entry.group => group.entries.push(entry),
            _ => groups.push(QuitGroup {
                key: entry.group,
                entries: vec![entry],
            }),
        }
    }
    groups
}

/// What the fact line counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QuitSummary {
    /// Every listed entry.
    pub count: usize,
    /// Component Preview rows are among them: "items", not "documents".
    pub has_rows: bool,
    /// Projects with unsaved documents.
    pub projects: usize,
    /// Libraries with unsaved drafts or rows.
    pub libraries: usize,
}

impl QuitSummary {
    pub(super) fn of(groups: &[QuitGroup], rows: usize) -> Self {
        let mut libraries: Vec<&Path> = groups
            .iter()
            .flat_map(|group| &group.entries)
            .filter_map(|entry| entry.library.as_deref())
            .collect();
        libraries.sort();
        libraries.dedup();
        Self {
            count: groups.iter().map(|group| group.entries.len()).sum(),
            has_rows: rows > 0,
            projects: groups
                .iter()
                .filter(|group| matches!(group.key, QuitGroupKey::Project(_)))
                .count(),
            libraries: libraries.len(),
        }
    }

    /// "5 unsaved items in 1 project and 1 library." The project and the
    /// library clause are each left out when they count none.
    pub(super) fn line(&self, i18n: &I18n) -> Localized {
        let kind = if self.has_rows { "items" } else { "documents" };
        let scope = match (self.projects > 0, self.libraries > 0) {
            (true, true) => "both",
            (true, false) => "projects",
            (false, true) => "libraries",
            (false, false) => "none",
        };
        let unsaved = Translatable::new(ids::DIALOG_QUIT_UNSAVED_COUNT)
            .arg("count", self.count)
            .arg("kind", kind);
        let projects =
            Translatable::new(ids::DIALOG_QUIT_PROJECT_COUNT).arg("count", self.projects);
        let libraries =
            Translatable::new(ids::DIALOG_QUIT_LIBRARY_COUNT).arg("count", self.libraries);
        t!(
            i18n,
            ids::DIALOG_QUIT_SUMMARY,
            "scope" => scope,
            "unsaved" => unsaved,
            "projects" => projects,
            "libraries" => libraries
        )
    }
}

/// The folder of `path`, as the muted column shows it.
fn folder_of(path: &Path) -> String {
    path.parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default()
}

/// The file name of `path`, or the whole path when it has none.
fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Signex {
    /// Every entry of the quit confirm, classified: drafts and rows under the
    /// libraries, a file under the open project that owns it (its sheets,
    /// its board, the project file itself), any other file under Other.
    pub(super) fn quit_entries(&self, paths: &[PathBuf], rows: &[EditorAddress]) -> Vec<QuitEntry> {
        let roots = self.draft_library_roots();
        let drafts: std::collections::HashSet<PathBuf> =
            self.document_state.unsaved_drafts().into_iter().collect();
        let files = paths.iter().map(|path| {
            if drafts.contains(path) {
                self.quit_draft_entry(path, &roots)
            } else {
                self.quit_file_entry(path)
            }
        });
        let rows = rows.iter().map(|address| self.quit_row_entry(address));
        files.chain(rows).collect()
    }

    /// A symbol or footprint draft: listed under the libraries, by its path
    /// in the library that owns it.
    fn quit_draft_entry(&self, path: &Path, roots: &[PathBuf]) -> QuitEntry {
        let fold = self.draft_fold(path);
        let library = crate::app::state::scope::owning_libraries(path, roots, fold)
            .first()
            .map(|lib| (*lib).clone());
        let dir = library.as_deref().and_then(Path::parent);
        QuitEntry {
            group: QuitGroupKey::Libraries,
            kind: kind_code(path),
            name: crate::library::close_prompt::dirty_path_label(path, dir),
            meta: library
                .as_deref()
                .map_or_else(|| folder_of(path), |lib| self.library.library_label(lib)),
            library: library.or_else(|| path.parent().map(Path::to_path_buf)),
        }
    }

    /// A file: under the open project that owns it (its sheets, its board,
    /// the project file itself), else under Other.
    fn quit_file_entry(&self, path: &Path) -> QuitEntry {
        let projects = &self.document_state.projects;
        let owner = self
            .document_state
            .document_project(path)
            .map(|project| project.id)
            .or_else(|| {
                projects
                    .iter()
                    .find(|project| project.path == *path)
                    .map(|project| project.id)
            });
        let group = owner
            .and_then(|id| projects.iter().position(|project| project.id == id))
            .map_or(QuitGroupKey::Other, QuitGroupKey::Project);
        QuitEntry {
            group,
            kind: kind_code(path),
            name: file_name_of(path),
            meta: folder_of(path),
            library: None,
        }
    }

    /// A Component Preview row: under the libraries, by its part number.
    fn quit_row_entry(&self, address: &EditorAddress) -> QuitEntry {
        QuitEntry {
            group: QuitGroupKey::Libraries,
            kind: Some(ids::PANELS_TAB_KIND_COMPONENT),
            name: self.library.editors.get(address).map_or_else(
                || address.row_id.to_string(),
                |editor| editor.row.internal_pn.as_str().to_string(),
            ),
            meta: self.library.library_label(&address.library_path),
            library: Some(address.library_path.clone()),
        }
    }

    /// `(group headings, rows)` of the quit confirm's list, for its window
    /// height (`modal_sizing`).
    pub(crate) fn quit_list_shape(&self, st: &crate::app::AppQuitConfirmState) -> (usize, usize) {
        let groups = group_entries(self.quit_entries(&st.dirty_paths, &st.dirty_rows));
        let rows = groups.iter().map(|group| group.entries.len()).sum();
        (groups.len(), rows)
    }

    /// The heading of a group: the project's name, or the catalog's.
    fn quit_group_title(&self, key: QuitGroupKey) -> String {
        let i18n = self.ui_state.i18n.as_ref();
        match key {
            QuitGroupKey::Project(index) => self
                .document_state
                .projects
                .get(index)
                .map(|project| project.data.name.clone())
                .unwrap_or_default(),
            QuitGroupKey::Other => t!(i18n, ids::DIALOG_QUIT_GROUP_OTHER).into_string(),
            QuitGroupKey::Libraries => t!(i18n, ids::DIALOG_QUIT_GROUP_LIBRARIES).into_string(),
        }
    }

    /// The list box: each group's heading, then its rows.
    fn quit_list<'a>(&self, groups: &[QuitGroup], tokens: &ThemeTokens) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let ink = crate::styles::ti(tokens.text);
        let muted = crate::styles::tokens::resolve(tokens).ink4;
        let mut items: Vec<Element<'a, Message>> = Vec::new();
        for group in groups {
            items.push(group_header(
                i18n,
                &self.quit_group_title(group.key),
                group.entries.len(),
                tokens,
            ));
            for entry in &group.entries {
                let mut cells: Vec<Element<'a, Message>> = Vec::new();
                if let Some(code) = entry.kind {
                    cells.push(kind_cap(i18n, code, tokens));
                }
                cells.push(
                    container(
                        text(UserText::new(entry.name.clone()))
                            .size(TS.body)
                            .color(ink)
                            .wrapping(iced::widget::text::Wrapping::None),
                    )
                    .width(Length::Fill)
                    .clip(true)
                    .boxed(),
                );
                cells.push(
                    container(
                        text(UserText::new(entry.meta.clone()))
                            .size(TS.caption)
                            .color(muted)
                            .wrapping(iced::widget::text::Wrapping::None),
                    )
                    .max_width(META_MAX_W)
                    .clip(true)
                    .boxed(),
                );
                items.push(list_row(cells, LIST_ROW_H));
            }
        }
        list_box(items, tokens)
    }

    pub(in crate::app::view) fn view_app_quit_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_app_quit_confirm_body();
        self.wrap_centred_dialog(dialog, super::sizes::APP_QUIT_CONFIRM)
    }

    pub(in crate::app::view) fn view_app_quit_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.app_quit_confirm else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();

        // macOS says "Quit", other systems "Exit".
        let macos = cfg!(target_os = "macos");
        let pick = |quit: MessageId, exit: MessageId| if macos { quit } else { exit };
        let choose = |choice| Message::Project(ProjectMsg::AppQuitConfirm(choice));
        let header_content: Element<'_, Message> = super::widgets::modal_title_header(
            t!(
                i18n,
                pick(
                    ids::DIALOG_QUIT_UNSAVED_TITLE,
                    ids::DIALOG_EXIT_UNSAVED_TITLE
                )
            )
            .into_string(),
            choose(ProjectCloseChoice::Cancel),
            tokens,
        );
        let header = self.modal_header(header_content, ModalId::AppQuitConfirm);

        let groups = group_entries(self.quit_entries(&st.dirty_paths, &st.dirty_rows));
        let summary = QuitSummary::of(&groups, st.dirty_rows.len());

        let lead = text(t!(
            i18n,
            pick(ids::DIALOG_QUIT_QUESTION, ids::DIALOG_EXIT_QUESTION)
        ))
        .size(TS.label)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .color(crate::styles::ti(tokens.text));
        let fact = text(summary.line(i18n))
            .size(TS.body)
            .color(crate::styles::ti(tokens.text_secondary));
        let mut body: Column<Element<'_, Message>> =
            column![column![lead.boxed(), fact.boxed()].spacing(SP.s).boxed()].spacing(SP.m);
        // The list changed since the user last saw it: say so above it.
        if self.ui_state.quit_confirm_refreshed {
            body = body.push(super::graphite::callout(
                DialogIcon::Warn,
                t!(i18n, ids::DIALOG_QUIT_LIST_CHANGED),
                tokens,
            ));
        }
        body = body.push(
            container(self.quit_list(&groups, tokens))
                .height(Length::Fit.max(
                    crate::app::handlers::modal_sizing::LIST_MAX_HEIGHT
                        + crate::app::handlers::modal_sizing::LIST_BOX_CHROME,
                ))
                .boxed(),
        );

        // Cancel at the left; Discard All, then Save All, at the right.
        let footer = dialog_footer_split(
            vec![dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(choose(ProjectCloseChoice::Cancel)),
                tokens,
            )],
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_UNSAVED_DISCARD_ALL),
                    DialogButtonKind::Danger,
                    Some(choose(ProjectCloseChoice::DiscardAll)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_SAVE_ALL),
                    DialogButtonKind::Primary,
                    Some(choose(ProjectCloseChoice::SaveAll)),
                    tokens,
                ),
            ],
        );
        container(
            column![header, dialog_body(body), footer]
                .width(self.dialog_width(ModalId::AppQuitConfirm, super::sizes::APP_QUIT_CONFIRM)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

#[cfg(test)]
#[path = "quit_confirm_tests.rs"]
mod tests;
