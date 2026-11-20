//! The "saved, not committed" banner of the Library Browser (#858).
//!
//! Shown while the library holds changes that were written to disk but that
//! no git commit records, with the cause and a "Commit now" retry. Graphite
//! (mockup row 118): a full-width warn banner under the header.

use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The message "Commit now" sends; `None` while a commit runs.
pub(super) fn commit_now_message(
    library_path: &std::path::Path,
    committing: bool,
) -> Option<LibraryMessage> {
    (!committing).then(|| LibraryMessage::BrowserCommitPending {
        library_path: library_path.to_path_buf(),
    })
}

/// The banner for the library at `library_path`; an empty element when
/// everything it saved is committed.
pub(super) fn view_uncommitted_banner<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(pending) = library_state.uncommitted_in(library_path) else {
        return Space::new().boxed();
    };
    let committing = library_state.is_committing(library_path);
    let summary = t!(
        i18n,
        ids::LIBRARY_BROWSER_UNCOMMITTED,
        "count" => pending.paths.len(),
        "reason" => pending.reason.as_str()
    );
    let label = t!(
        i18n,
        if committing {
            ids::LIBRARY_BROWSER_COMMITTING
        } else {
            ids::LIBRARY_BROWSER_COMMIT_NOW
        }
    );
    let (fill, warn) = crate::styles::pill_colors(tokens, crate::styles::PillTone::Warn);
    let commit_now = crate::library::editor::kit::small_action(
        label,
        commit_now_message(library_path, committing),
        tokens,
    );
    let line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        super::header::tinted_icon(
            crate::icons::icon_chrome_git(signex_types::theme::ThemeId::GraphiteDark),
            BROWSER_TEXT_SIZE,
            warn
        ),
        text(summary).size(BROWSER_TEXT_SIZE).color(warn).boxed(),
        Space::new().width(Length::Fill).boxed(),
        commit_now,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    container(line)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(fill)),
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Commit now" is the existing commit retry, and waits while a commit
    /// runs.
    #[test]
    fn commit_now_retries_the_commit_unless_one_runs() {
        let path = std::path::Path::new("parts.snxlib");
        match commit_now_message(path, false) {
            Some(LibraryMessage::BrowserCommitPending { library_path }) => {
                assert_eq!(library_path, path);
            }
            other => panic!("expected BrowserCommitPending, got {other:?}"),
        }
        assert!(commit_now_message(path, true).is_none());
    }
}
