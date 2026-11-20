//! History graph samples: the mockup's power.snxsch history (working tree,
//! a merge on main at HEAD, two ripple-fix commits in lane 1, a tag on an
//! old commit) with one selected row and its detail strip, drawn in both
//! Graphite themes side by side at the panel's 340 px width.

use iced::widget::{Row, Space, column, container};
use iced::{Background, Element, Length, Theme, Widget};
use signex_i18n::{UserText, ids, t};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::history_graph::{
    ChangedFile, DateTime, Duration, FileStatus, GraphCommit, HistoryGraph, HistoryGraphStyle,
    RefPill, Utc, commit_detail, more_button, small_button,
};
use signex_widgets::theme_ext::TypeScale;

use crate::catalog::Message;
use crate::{locale, theme};

/// The panel width the mockup draws the History panel at, in px at the
/// default base size.
const PANEL_WIDTH: f32 = 340.0;

/// The fixed "now" the sample ages are measured from, so the page never
/// changes between frames.
fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_795_000_000, 0).unwrap_or_default()
}

fn branch(name: &str, is_current: bool) -> RefPill {
    RefPill::Branch {
        name: name.into(),
        is_current,
    }
}

/// One sample commit by `author`, made `age` before [`now`].
fn sample(
    sha: &str,
    parents: &[&str],
    refs: Vec<RefPill>,
    author: &str,
    subject: &str,
    age: Duration,
) -> GraphCommit {
    GraphCommit {
        sha: sha.into(),
        parents: parents.iter().map(|p| (*p).to_string()).collect(),
        refs,
        author_name: author.into(),
        author_email: format!("{author}@example.com"),
        time: now() - age,
        subject: subject.into(),
    }
}

pub(crate) fn sample_commits() -> Vec<GraphCommit> {
    let tag = RefPill::Tag {
        name: "v1.0".into(),
    };
    vec![
        sample(
            "a3bbcc6",
            &["9e01f4d", "7f21c90"],
            vec![branch("main", true), RefPill::Head],
            "j.berg",
            "Merge ripple-fix into main",
            Duration::minutes(12),
        ),
        sample(
            "7f21c90",
            &["e44d1a2"],
            vec![branch("ripple-fix", false)],
            "j.berg",
            "Raise L1 to 4.7 µH for ripple margin",
            Duration::hours(1),
        ),
        sample(
            "e44d1a2",
            &["5d7e210"],
            Vec::new(),
            "a.yilmaz",
            "Add output cap C12",
            Duration::hours(3),
        ),
        sample(
            "9e01f4d",
            &["5d7e210"],
            Vec::new(),
            "a.yilmaz",
            "Swap Q2 to a lower-Rds(on) part",
            Duration::hours(5),
        ),
        sample(
            "5d7e210",
            &["c41aa02"],
            Vec::new(),
            "j.berg",
            "Add soft-start cap C7",
            Duration::days(2),
        ),
        sample(
            "c41aa02",
            &["0b3f9e1"],
            vec![tag],
            "m.demir",
            "Split power stage into its own sheet",
            Duration::days(5),
        ),
        sample(
            "0b3f9e1",
            &[],
            Vec::new(),
            "j.berg",
            "Initial import",
            Duration::days(35),
        ),
    ]
}

/// The changed files the detail strip lists for `sha`.
fn sample_files(sha: &str) -> Vec<ChangedFile> {
    let file = |status, path: &str| ChangedFile {
        status,
        path: path.into(),
    };
    if sha == "9e01f4d" {
        vec![
            file(FileStatus::Modified, "power.snxsch"),
            file(FileStatus::Modified, "dcdc-buck.snxpcb"),
            file(FileStatus::Added, "assets/Q2.step"),
        ]
    } else {
        vec![file(FileStatus::Modified, "power.snxsch")]
    }
}

/// One theme's panel: the graph, then the selected commit's detail strip.
fn panel<'a>(id: ThemeId, commits: &'a [GraphCommit], selected: &'a str) -> Element<'a, Message> {
    let tokens = theme_tokens(id);
    let style = HistoryGraphStyle::from_tokens(&tokens, &TypeScale::DEFAULT);
    let i18n = locale::en_us();
    let graph = HistoryGraph::new(commits, style, i18n, now())
        .working_tree(Some(Message::HistorySampleCommitPressed))
        .selected(Some(selected))
        .on_select(Message::HistoryCommitSelected)
        .view();
    let detail = commits.iter().find(|c| c.sha == selected).map(|c| {
        let actions = vec![
            small_button(
                &style,
                t!(i18n, ids::PANELS_HISTORY_SHOW_DIFFERENCES),
                Some(Message::HistorySampleCommitPressed),
            ),
            small_button(
                &style,
                t!(i18n, ids::PANELS_HISTORY_SHOW_ON_BOARD),
                Some(Message::HistorySampleCommitPressed),
            ),
            small_button(
                &style,
                t!(i18n, ids::PANELS_HISTORY_RESTORE),
                Some(Message::HistorySampleCommitPressed),
            ),
            more_button(&style, i18n, Some(Message::HistorySampleCommitPressed)),
        ];
        commit_detail(&style, i18n, c, &sample_files(&c.sha), actions)
    });
    let panel_bg = theme::color(tokens.panel_bg);
    let mut body = column![
        signex_widgets::text(UserText::new(id.label()))
            .size(TypeScale::DEFAULT.caption)
            .color(theme::color(tokens.text_secondary)),
        graph,
    ];
    if let Some(detail) = detail {
        body = body.push(detail);
    }
    container(body)
        .width(PANEL_WIDTH)
        .padding(TypeScale::DEFAULT.caption)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            ..container::Style::default()
        })
        .boxed()
}

pub(crate) fn view<'a>(commits: &'a [GraphCommit], selected: &'a str) -> Element<'a, Message> {
    Row::new()
        .push(panel(ThemeId::GraphiteLight, commits, selected))
        .push(Space::new().width(TypeScale::DEFAULT.body).boxed())
        .push(panel(ThemeId::GraphiteDark, commits, selected))
        .width(Length::Shrink)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_selection_is_in_the_sample() {
        let commits = sample_commits();
        assert!(commits.iter().any(|c| c.sha == "9e01f4d"));
        assert_eq!(commits.iter().filter(|c| c.is_head()).count(), 1);
        assert_eq!(commits.iter().filter(|c| c.is_merge()).count(), 1);
    }

    #[test]
    fn the_sample_draws_two_lanes() {
        let g = signex_widgets::history_graph::layout(&sample_commits(), true);
        assert_eq!(g.lane_count, 2);
        assert_eq!(g.rows.len(), 8);
    }
}
