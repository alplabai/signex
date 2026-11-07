//! The "saved, not committed" banner of the Library Browser (#858).
//!
//! Shown while the library holds changes that were written to disk but that
//! no git commit records, with the cause and a "Commit now" retry.

use super::*;
use iced::widget::column;

/// The banner for the library at `library_path`; an empty element when
/// everything it saved is committed.
pub(super) fn view_uncommitted_banner<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let Some(pending) = library_state.uncommitted_in(library_path) else {
        return Space::new().boxed();
    };
    let committing = library_state.is_committing(library_path);
    let count = pending.paths.len();
    let summary = format!(
        "Uncommitted changes: {count} saved file{} not in version control. {}",
        if count == 1 { "" } else { "s" },
        pending.reason
    );
    if crate::styles::is_graphite(tokens) {
        return sidebar_footer(library_path, committing, summary, tokens);
    }
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
    .padding([6.0, 10.0])
    .width(Length::Fill)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.95, 0.65, 0.1, 0.18,
        ))),
        border: Border::default()
            .width(1.0)
            .rounded(3.0)
            .color(iced::Color::from_rgba(0.95, 0.65, 0.1, 0.6)),
        ..iced::widget::container::Style::default()
    })
    .boxed()
}

/// Graphite: the banner sits in the sidebar footer, in the `warn` ink over
/// a `warn_soft` card, with the retry as a quiet outline button.
fn sidebar_footer<'a>(
    library_path: &'a std::path::Path,
    committing: bool,
    summary: String,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (fill, warn) = crate::styles::pill_colors(tokens, crate::styles::PillTone::Warn);
    let line = theme_ext::border_color(tokens);
    let label = if committing {
        "Committing…"
    } else {
        "Commit now"
    };
    let commit_now = button(
        text(label)
            .size(BROWSER_TEXT_SIZE)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([SP.xs, SP.m])
    .on_press_maybe((!committing).then(|| LibraryMessage::BrowserCommitPending {
        library_path: library_path.to_path_buf(),
    }))
    .style(crate::styles::outline_button(tokens));
    container(
        column![
            text(summary).size(BROWSER_HEADER_SIZE).color(warn),
            commit_now,
        ]
        .spacing(SP.s),
    )
    .padding(SP.m)
    .width(Length::Fill)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: line,
            radius: crate::styles::Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    })
    .boxed()
}
