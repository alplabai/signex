//! History graph: a git-style commit graph for the History panel. A lane
//! column on the left (one coloured lane per branch, a dot per commit,
//! fork and merge curves, a ring under merge commits, a dashed lane from the
//! working tree to HEAD) beside compact two-line rows, plus a detail strip
//! for the selected commit.
//!
//! Stateless: the caller owns the commits (newest first), the selected SHA
//! and the messages. The panel header above it (file name, branch switcher,
//! filter chips) is the caller's too.

mod detail;
mod draw;
mod lanes;
mod model;
mod row;
mod style;
#[cfg(test)]
mod tests;

use iced::widget::{Canvas, Column};
use iced::{Element, Length, Padding, Widget};
use signex_i18n::I18n;

/// The time types of [`GraphCommit::time`], for callers without their own
/// chrono dependency (the chrome catalog).
pub use chrono::{DateTime, Duration, Utc};
pub use detail::{commit_detail, commit_detail_picking, more_button};
pub use draw::{GraphGeometry, SegmentShape};
pub use lanes::{DotKind, Edge, EdgeKind, EdgeStyle, GraphLayout, RowLayout, layout};
pub use model::{ChangedFile, FileStatus, GraphCommit, RefPill, author_color_index, initials};
pub use row::{relative_time, small_button};
pub use style::HistoryGraphStyle;

use crate::theme_ext::Spacing;
use draw::GraphProgram;
use row::{RowContext, commit_row, working_tree_row};
use style::GRAPH_INSET_EM;

/// The lane column and the rows, built from the caller's data.
pub struct HistoryGraph<'a, M> {
    commits: &'a [GraphCommit],
    style: HistoryGraphStyle,
    i18n: &'a I18n,
    now: DateTime<Utc>,
    selected: Option<&'a str>,
    has_working_tree: bool,
    on_commit: Option<M>,
    on_select: Option<Box<dyn Fn(String) -> M + 'a>>,
}

impl<'a, M: Clone + 'a> HistoryGraph<'a, M> {
    /// `commits` newest first; `now` is the time ages are measured from
    /// (production passes `Utc::now()`, tests a fixed instant).
    pub fn new(
        commits: &'a [GraphCommit],
        style: HistoryGraphStyle,
        i18n: &'a I18n,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            commits,
            style,
            i18n,
            now,
            selected: None,
            has_working_tree: false,
            on_commit: None,
            on_select: None,
        }
    }

    /// The SHA of the row drawn as selected.
    pub fn selected(mut self, sha: Option<&'a str>) -> Self {
        self.selected = sha;
        self
    }

    /// Show the uncommitted-changes row on top, its lane dashed down to
    /// HEAD. `on_commit` is the Commit… button's message; none disables it.
    pub fn working_tree(mut self, on_commit: Option<M>) -> Self {
        self.has_working_tree = true;
        self.on_commit = on_commit;
        self
    }

    /// Map a click on a commit row to the caller's message, given the
    /// commit's full SHA.
    pub fn on_select(mut self, on_select: impl Fn(String) -> M + 'a) -> Self {
        self.on_select = Some(Box::new(on_select));
        self
    }

    /// The lane layout the view draws.
    pub fn layout(&self) -> GraphLayout {
        layout(self.commits, self.has_working_tree)
    }

    /// The message a click on commit `index` (into the commits) sends.
    pub fn press_message(&self, index: usize) -> Option<M> {
        let commit = self.commits.get(index)?;
        self.on_select.as_ref().map(|f| f(commit.sha.clone()))
    }

    pub fn view(self) -> Element<'a, M> {
        let graph = self.layout();
        let s = self.style;
        let geometry = GraphGeometry::new(s.text_size);
        let ctx = RowContext {
            style: s,
            i18n: self.i18n,
            now: self.now,
        };
        let offset = usize::from(self.has_working_tree);

        let mut rows: Column<Element<'a, M>> = Column::new().width(Length::Fill);
        if self.has_working_tree {
            rows = rows.push(working_tree_row(&ctx, self.on_commit.clone()));
        }
        for (i, commit) in self.commits.iter().enumerate() {
            let lane = graph.rows.get(i + offset).map_or(0, |r| r.lane);
            let is_selected = self.selected == Some(commit.sha.as_str());
            rows = rows.push(commit_row(
                &ctx,
                commit,
                lane,
                is_selected,
                self.press_message(i),
            ));
        }

        let (width, height) = (
            geometry.width(graph.lane_count),
            geometry.height(graph.rows.len()),
        );
        let lanes = Canvas::new(GraphProgram {
            style: s,
            layout: graph,
        })
        .width(width)
        .height(height);

        iced::widget::container(iced::widget::row![lanes, rows])
            .width(Length::Fill)
            .padding(Padding {
                left: s.em(GRAPH_INSET_EM),
                right: Spacing::from_base(s.text_size).m,
                ..Padding::ZERO
            })
            .boxed()
    }
}
