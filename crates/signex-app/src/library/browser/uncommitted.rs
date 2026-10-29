//! The "saved, not committed" banner of the Library Browser (#858).
//!
//! Shown while the library holds changes that were written to disk but that
//! no git commit records, with the cause and a "Commit now" retry.

use super::*;

/// The banner for the library at `library_path`; an empty element when
/// everything it saved is committed.
pub(super) fn view_uncommitted_banner<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let Some(pending) = library_state.uncommitted_in(library_path) else {
        return Space::new().into();
    };
    let committing = library_state.is_committing(library_path);
    let count = pending.paths.len();
    let summary = format!(
        "Uncommitted changes: {count} saved file{} not in version control. {}",
        if count == 1 { "" } else { "s" },
        pending.reason
    );
    let warn = theme_ext::text_primary(tokens);
    let commit_now = button(
        text(if committing {
            "Committing…"
        } else {
            "Commit now"
        })
        .size(BROWSER_TEXT_SIZE)
        .color(warn),
    )
    .padding([SP.s, 10.0])
    .on_press_maybe((!committing).then(|| LibraryMessage::BrowserCommitPending {
        library_path: library_path.to_path_buf(),
    }));
    container(
        row![
            text(summary).size(BROWSER_TEXT_SIZE).color(warn),
            Space::new().width(Length::Fill),
            commit_now,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 10])
    .width(Length::Fill)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.95, 0.65, 0.1, 0.18,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: iced::Color::from_rgba(0.95, 0.65, 0.1, 0.6),
        },
        ..iced::widget::container::Style::default()
    })
    .into()
}
