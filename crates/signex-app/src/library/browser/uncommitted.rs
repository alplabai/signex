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
    sidebar_footer(library_path, committing, summary, label, tokens)
}

/// Graphite: the banner sits in the sidebar footer, in the `warn` ink over
/// a `warn_soft` card, with the retry as a quiet outline button.
fn sidebar_footer<'a>(
    library_path: &'a std::path::Path,
    committing: bool,
    summary: signex_i18n::Localized,
    label: signex_i18n::Localized,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (fill, warn) = crate::styles::pill_colors(tokens, crate::styles::PillTone::Warn);
    let line = theme_ext::border_color(tokens);
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
