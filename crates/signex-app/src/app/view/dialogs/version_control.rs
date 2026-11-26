//! The Enable Version Control dialog (mockup row 26): which files the first commit
//! takes, its message, Git LFS for 3D models and, for a project, whether
//! every save is committed.

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::{ToggleSwitchStyle, checkbox, raw, text, text_input, toggle_switch};

use super::widgets::modal_title_header;
use crate::app::contracts::{CommitAuthor, TrackKind};
use crate::app::{EnableVersionControlState, TrackItem, VersionControlScope};
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_caption, dialog_field_label,
    dialog_footer, dialog_inline_error, dialog_intro_with, dialog_row, dialog_rows,
    dialog_section_label, file_kind_icon,
};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

use crate::app::handlers::modal_sizing::TRACK_LIST_MAX_HEIGHT;

/// Side of a row's checkbox: the title step, the mock's 14 px at the 12 px
/// body (14.2), following the base size.
const ROW_CHECKBOX: f32 = TS.title;
/// Gap between an option's control and its text.
const OPTION_GAP: f32 = 8.0;

/// The catalog name of a row's kind.
pub(in crate::app::view) fn track_kind_label(kind: TrackKind) -> MessageId {
    match kind {
        TrackKind::Project => ids::DIALOG_VERSION_CONTROL_KIND_PROJECT,
        TrackKind::Schematic => ids::DIALOG_VERSION_CONTROL_KIND_SCHEMATIC,
        TrackKind::Pcb => ids::DIALOG_VERSION_CONTROL_KIND_PCB,
        TrackKind::Library => ids::DIALOG_VERSION_CONTROL_KIND_LIBRARY,
        TrackKind::Config => ids::DIALOG_VERSION_CONTROL_KIND_CONFIG,
        TrackKind::Components => ids::DIALOG_VERSION_CONTROL_KIND_COMPONENTS,
        TrackKind::Folder => ids::DIALOG_VERSION_CONTROL_KIND_FOLDER,
    }
}

/// The row's path as the list shows it: a folder ends in `/`.
pub(in crate::app::view) fn track_display_path(item: &TrackItem) -> String {
    let path = item.relative.replace('\\', "/");
    if item.is_directory && !path.ends_with('/') {
        format!("{path}/")
    } else {
        path
    }
}

/// Enable is offered once the rows are read, while no run is in flight, the
/// first commit message is not blank and the commit author is known (git's,
/// or both fields filled; mockup row 26).
pub(in crate::app::view) fn can_enable(st: &EnableVersionControlState) -> bool {
    !st.working && !st.scanning && !st.commit_message.trim().is_empty() && st.author.is_ready()
}

/// The name and email fields, shown only while git has no author.
fn author_fields<'a>(
    st: &'a EnableVersionControlState,
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Option<Element<'a, Message>> {
    let CommitAuthor::Missing { name, email } = &st.author else {
        return None;
    };
    let field = |value: &'a str, on: fn(String) -> EnableVersionControlMsg| {
        let mut input = text_input(UserText::new(String::new()), value)
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body)
            .padding([SP.s, SP.m])
            .width(Length::Fill);
        if !st.working {
            input = input
                .on_input(move |s| Message::EnableVersionControl(on(s)))
                .on_submit(Message::EnableVersionControl(
                    EnableVersionControlMsg::Confirm,
                ));
        }
        input.boxed()
    };
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    let note = text(t!(i18n, ids::DIALOG_VERSION_CONTROL_AUTHOR_NOTE))
        .size(TS.caption)
        .color(ink3);
    let column = column![
        note,
        dialog_field_label(t!(i18n, ids::DIALOG_VERSION_CONTROL_AUTHOR_NAME), tokens),
        field(name.as_str(), EnableVersionControlMsg::AuthorNameChanged),
        dialog_field_label(t!(i18n, ids::DIALOG_VERSION_CONTROL_AUTHOR_EMAIL), tokens),
        field(email.as_str(), EnableVersionControlMsg::AuthorEmailChanged),
    ]
    .spacing(SP.xs);
    Some(column.boxed())
}

impl Signex {
    pub(in crate::app::view) fn view_enable_version_control_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_enable_version_control_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::EnableVersionControl,
            dialog,
            super::sizes::ENABLE_VERSION_CONTROL,
        )
    }

    pub(in crate::app::view) fn view_enable_version_control_dialog_body(
        &self,
    ) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.enable_version_control else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let modal = crate::app::state::ModalId::EnableVersionControl;
        let vc = |m: EnableVersionControlMsg| Message::EnableVersionControl(m);

        let header = self.modal_header(
            modal_title_header(
                t!(i18n, ids::DIALOG_TITLE_ENABLE_VERSION_CONTROL).into_string(),
                vc(EnableVersionControlMsg::Close),
                tokens,
            ),
            modal,
        );

        let mut body = column![intro(st, i18n, tokens)].spacing(SP.l);
        if st.scanning {
            body = body.push(dialog_caption(
                t!(i18n, ids::DIALOG_VERSION_CONTROL_SCANNING),
                tokens,
            ));
        } else if !st.items.is_empty() {
            body = body.push(track_list(st, i18n, tokens));
        }
        let mut message = text_input(UserText::new(String::new()), st.commit_message.as_str())
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body)
            .padding([SP.s, SP.m])
            .width(Length::Fill);
        if !st.working {
            message = message
                .on_input(move |s| vc(EnableVersionControlMsg::CommitMessageChanged(s)))
                .on_submit(vc(EnableVersionControlMsg::Confirm));
        }
        body = body.push(
            column![
                dialog_field_label(t!(i18n, ids::DIALOG_VERSION_CONTROL_COMMIT_MESSAGE), tokens),
                message,
            ]
            .boxed(),
        );
        if let Some(fields) = author_fields(st, i18n, tokens) {
            body = body.push(fields);
        }
        let lfs = checkbox(st.use_lfs)
            .style(crate::panels::form_look::checkbox_style)
            .size(ROW_CHECKBOX)
            .on_toggle(move |_| vc(EnableVersionControlMsg::ToggleLfs))
            .boxed();
        body = body.push(option_row(
            lfs,
            t!(i18n, ids::DIALOG_VERSION_CONTROL_LFS),
            t!(i18n, ids::DIALOG_VERSION_CONTROL_LFS_HELP),
            tokens,
        ));
        if st.scope == VersionControlScope::Project {
            let r = crate::styles::tokens::resolve(tokens);
            let look = ToggleSwitchStyle::new(
                crate::styles::tokens::accent(tokens),
                r.line_strong,
                crate::styles::tokens::on_accent(tokens),
            );
            let switch = toggle_switch(
                st.commit_every_save,
                look,
                Some(vc(EnableVersionControlMsg::CommitEverySaveToggled)),
            );
            body = body.push(option_row(
                switch,
                t!(i18n, ids::DIALOG_VERSION_CONTROL_COMMIT_EVERY_SAVE),
                t!(i18n, ids::DIALOG_VERSION_CONTROL_COMMIT_EVERY_SAVE_HELP),
                tokens,
            ));
        }
        if let Some(err) = st.error.as_ref() {
            // raw: the setup error is worded by the version-control job (#2061).
            let err = text(raw(err.clone())).size(TS.body).boxed();
            body = body.push(dialog_inline_error(err, tokens));
        }

        let enable_label = if st.working {
            ids::DIALOG_VERSION_CONTROL_ENABLING
        } else {
            ids::DIALOG_VERSION_CONTROL_ENABLE
        };
        let dialog = container(
            column![
                header,
                dialog_body(body),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(vc(EnableVersionControlMsg::Close)),
                        tokens
                    ),
                    dialog_button(
                        t!(i18n, enable_label),
                        DialogButtonKind::Primary,
                        can_enable(st).then_some(vc(EnableVersionControlMsg::Confirm)),
                        tokens
                    ),
                ]),
            ]
            .width(self.dialog_width(modal, super::sizes::ENABLE_VERSION_CONTROL)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// The intro: the folder that gets a repository (in the mono face, as in
/// Rename), then what the first commit takes and what is committed after it.
fn intro<'a>(
    st: &'a EnableVersionControlState,
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let (title_id, sentence) = match st.scope {
        VersionControlScope::Project => (
            ids::DIALOG_VERSION_CONTROL_PROJECT,
            t!(
                i18n,
                ids::DIALOG_VERSION_CONTROL_INTRO_PROJECT,
                "every" => if st.commit_every_save { "on" } else { "off" }
            ),
        ),
        VersionControlScope::Library => (
            ids::DIALOG_VERSION_CONTROL_LIBRARY,
            t!(i18n, ids::DIALOG_VERSION_CONTROL_INTRO_LIBRARY),
        ),
    };
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    let path_line = text(UserText::new(st.project_dir.display().to_string()))
        .size(TS.caption)
        .font(super::graphite::mono())
        .color(ink3);
    let secondary = column![path_line, text(sentence).size(TS.body).color(ink3)].spacing(SP.xs);
    dialog_intro_with(
        DialogIcon::VersionControl,
        t!(i18n, title_id, "name" => st.project_name.as_str()).into_string(),
        Some(secondary.boxed()),
        tokens,
    )
}

/// The heading with the ticked count and Select all, then the bordered file
/// list (scrolls past [`TRACK_LIST_MAX_HEIGHT`]).
fn track_list<'a>(
    st: &'a EnableVersionControlState,
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let total = st.items.len();
    let ticked = st.items.iter().filter(|i| i.tracked).count();
    let all_ticked = ticked == total;
    let count_id = if all_ticked {
        ids::DIALOG_VERSION_CONTROL_TICKED
    } else {
        ids::DIALOG_VERSION_CONTROL_TICKED_MORE
    };
    let mut count = row![dialog_caption(
        t!(i18n, count_id, "ticked" => ticked, "total" => total),
        tokens
    )]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    if !all_ticked {
        count = count.push(select_all_link(i18n, tokens));
    }
    let heading = row![
        dialog_section_label(t!(i18n, ids::DIALOG_VERSION_CONTROL_TRACK_HEADING), tokens),
        Space::new().width(Length::Fill),
        count,
    ]
    .align_y(Alignment::Center);

    let r = crate::styles::tokens::resolve(tokens);
    let ink = crate::styles::ti(tokens.text);
    let mono = super::graphite::mono();
    let rows: Vec<Element<'a, Message>> = st
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let check = checkbox(item.tracked)
                .style(crate::panels::form_look::checkbox_style)
                .size(ROW_CHECKBOX)
                .on_toggle(move |_| {
                    Message::EnableVersionControl(EnableVersionControlMsg::ToggleItem(idx))
                })
                .boxed();
            let shown = track_display_path(item);
            let icon = file_kind_icon(std::path::Path::new(&shown));
            let name = text(UserText::new(shown))
                .size(TS.body)
                .font(mono)
                .color(if item.tracked { ink } else { r.ink3 })
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .boxed();
            let kind = dialog_caption(t!(i18n, track_kind_label(item.kind)), tokens);
            dialog_row(vec![check, icon, name, kind])
        })
        .collect();
    column![
        heading,
        dialog_rows(None, rows, Some(TRACK_LIST_MAX_HEIGHT), tokens),
    ]
    .spacing(SP.s + 2.0)
    .boxed()
}

/// "Select all": an accent text link that ticks every row.
fn select_all_link<'a>(
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let ink = crate::styles::tokens::resolve(tokens).accent_text;
    signex_widgets::button(
        text(t!(i18n, ids::DIALOG_VERSION_CONTROL_SELECT_ALL))
            .size(TS.caption)
            .color(ink),
    )
    .padding(0)
    .style(move |_: &iced::Theme, _| iced::widget::button::Style {
        background: None,
        text_color: ink,
        ..iced::widget::button::Style::default()
    })
    .on_press(Message::EnableVersionControl(
        EnableVersionControlMsg::AllTicked,
    ))
    .boxed()
}

/// An option: its control, then a medium-weight title over a caption help
/// line.
fn option_row<'a>(
    control: Element<'a, Message>,
    title: signex_i18n::Localized,
    help: signex_i18n::Localized,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    let words = column![
        text(title)
            .size(TS.body)
            .font(crate::styles::medium_font())
            .color(crate::styles::ti(tokens.text)),
        text(help).size(TS.caption).color(ink3),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    row![container(control).padding([1.0, 0.0]), words]
        .spacing(OPTION_GAP)
        .align_y(Alignment::Start)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn item(relative: &str, kind: TrackKind, is_directory: bool, tracked: bool) -> TrackItem {
        TrackItem {
            absolute: PathBuf::from("/p").join(relative),
            relative: relative.to_string(),
            kind,
            is_directory,
            tracked,
        }
    }

    fn dialog(scope: VersionControlScope, items: Vec<TrackItem>) -> EnableVersionControlState {
        EnableVersionControlState {
            scope,
            project_path: PathBuf::from("/p/dcdc-buck.snxprj"),
            project_dir: PathBuf::from("/p"),
            project_name: "dcdc-buck".to_string(),
            items,
            use_lfs: false,
            commit_message: "Initial version of dcdc-buck".to_string(),
            commit_every_save: true,
            error: None,
            working: false,
            scanning: false,
            author: CommitAuthor::Resolved,
        }
    }

    /// Every kind has its own catalog name, and it renders.
    #[test]
    fn every_row_kind_has_a_catalog_name() {
        let i18n = signex_i18n::I18n::en_us_ref();
        let kinds = [
            TrackKind::Project,
            TrackKind::Schematic,
            TrackKind::Pcb,
            TrackKind::Library,
            TrackKind::Config,
            TrackKind::Components,
            TrackKind::Folder,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(track_kind_label(*a), track_kind_label(*b), "{a:?} / {b:?}");
            }
        }
        assert_eq!(
            t!(i18n, track_kind_label(TrackKind::Pcb)).into_string(),
            "PCB"
        );
    }

    /// A folder row ends in a slash, so its icon and its name read as a
    /// folder; a file row is shown as listed.
    #[test]
    fn a_folder_row_ends_in_a_slash() {
        assert_eq!(
            track_display_path(&item("assets", TrackKind::Folder, true, true)),
            "assets/"
        );
        assert_eq!(
            track_display_path(&item("lib\\a.snxlib", TrackKind::Library, true, true)),
            "lib/a.snxlib/"
        );
        assert_eq!(
            track_display_path(&item("power.snxsch", TrackKind::Schematic, false, true)),
            "power.snxsch"
        );
    }

    /// Enable waits for the rows, for a run in flight and for a commit
    /// message.
    #[test]
    fn enable_needs_the_rows_no_run_and_a_message() {
        let mut st = dialog(VersionControlScope::Project, Vec::new());
        assert!(can_enable(&st));
        st.commit_message = "   ".to_string();
        assert!(!can_enable(&st), "a blank message withholds Enable");
        st.commit_message = "x".to_string();
        st.scanning = true;
        assert!(!can_enable(&st));
        st.scanning = false;
        st.working = true;
        assert!(!can_enable(&st));
    }

    /// Mockup row 26: Enable waits for git's answer and, when git has no
    /// author, for both the name and the email.
    #[test]
    fn enable_needs_a_known_author() {
        let mut st = dialog(VersionControlScope::Project, Vec::new());
        st.author = CommitAuthor::Probing;
        assert!(!can_enable(&st), "git has not answered yet");
        let missing = |name: &str, email: &str| CommitAuthor::Missing {
            name: name.to_string(),
            email: email.to_string(),
        };
        st.author = missing("", "");
        assert!(!can_enable(&st));
        st.author = missing("Ada", "  ");
        assert!(!can_enable(&st), "a blank email withholds Enable");
        st.author = missing("Ada", "ada@example.org");
        assert!(can_enable(&st));
    }

    /// Mockup row 26: the name and email fields show only while git has no
    /// author.
    #[test]
    fn the_author_fields_show_only_when_git_has_none() {
        let texts = |author: CommitAuthor| {
            let (mut app, _) = Signex::new();
            let mut st = dialog(VersionControlScope::Project, Vec::new());
            st.author = author;
            app.ui_state.enable_version_control = Some(st);
            crate::layout_probe::lay_out_texts(
                app.view_enable_version_control_dialog_body(),
                iced::Size::new(1200.0, 900.0),
            )
            .into_iter()
            .map(|t| t.text)
            .collect::<Vec<String>>()
        };
        let missing = texts(CommitAuthor::Missing {
            name: String::new(),
            email: String::new(),
        });
        for want in ["Your name", "Your email"] {
            assert!(missing.iter().any(|s| s == want), "{want:?} in {missing:?}");
        }
        for author in [CommitAuthor::Resolved, CommitAuthor::Probing] {
            let shown = texts(author);
            assert!(!shown.iter().any(|s| s == "Your name"), "{shown:?}");
        }
    }

    /// The dialog builds in both Graphite themes, for both scopes, with the
    /// list partly ticked, while scanning and with an error.
    #[test]
    fn the_dialog_builds_in_every_state() {
        use signex_types::theme::ThemeId;
        for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            for scope in [VersionControlScope::Project, VersionControlScope::Library] {
                let (mut app, _) = Signex::new();
                app.document_state.panel_ctx.tokens = app.tokens_for(theme);
                let mut st = dialog(
                    scope,
                    vec![
                        item("dcdc-buck.snxprj", TrackKind::Project, false, true),
                        item("outputs", TrackKind::Folder, true, false),
                    ],
                );
                app.ui_state.enable_version_control = Some(st.clone());
                let _ = app.view_enable_version_control_dialog_body();
                st.scanning = true;
                st.error = Some("refused".to_string());
                app.ui_state.enable_version_control = Some(st);
                let _ = app.view_enable_version_control_dialog_body();
            }
        }
    }

    /// The laid-out dialog shows the count, Select all while a row is
    /// unticked, the commit message and, for a project, Commit every save.
    #[test]
    fn the_dialog_shows_the_count_select_all_and_the_options() {
        let (mut app, _) = Signex::new();
        app.ui_state.enable_version_control = Some(dialog(
            VersionControlScope::Project,
            vec![
                item("dcdc-buck.snxprj", TrackKind::Project, false, true),
                item("outputs", TrackKind::Folder, true, false),
            ],
        ));
        let texts = crate::layout_probe::lay_out_texts(
            app.view_enable_version_control_dialog_body(),
            iced::Size::new(1200.0, 900.0),
        );
        let shown: Vec<String> = texts
            .iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        for want in [
            "1 of 2 ticked ·",
            "Select all",
            "First commit message",
            "Commit every save",
            "outputs/",
            "Folder",
        ] {
            assert!(shown.iter().any(|s| s == want), "{want:?} in {shown:?}");
        }

        let mut all = dialog(
            VersionControlScope::Library,
            vec![item("library.toml", TrackKind::Config, false, true)],
        );
        all.project_name = "parts".to_string();
        app.ui_state.enable_version_control = Some(all);
        let texts = crate::layout_probe::lay_out_texts(
            app.view_enable_version_control_dialog_body(),
            iced::Size::new(1200.0, 900.0),
        );
        let shown: Vec<String> = texts
            .iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert!(shown.iter().any(|s| s == "1 of 1 ticked"), "{shown:?}");
        assert!(!shown.iter().any(|s| s == "Select all"), "all ticked");
        assert!(
            !shown.iter().any(|s| s == "Commit every save"),
            "a library has no Commit every save switch"
        );
    }
}
