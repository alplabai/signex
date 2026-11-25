//! The differences Show differences… opens under the detail strip (mockup
//! row 39): the file with its added and removed counts and Close, then each
//! run of changed lines. Pure: state in, elements out.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, container, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_library::repo_diff::{DiffLine, DiffLineKind, FileDiff, FileDiffBody, MAX_DIFF_LINES};
use signex_widgets::history_graph::small_button;
use signex_widgets::theme_ext::{HAIRLINE, Spacing};
use signex_widgets::{HistoryGraphStyle, text};

use super::model::{HistoryCommitMsg, HistoryDiff, HistoryPanelMsg};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Gap between the header, the runs and their lines.
const GAP_EM: f32 = 6.0 / 12.0;
/// Width of the + / − mark in front of a line.
const MARK_EM: f32 = 1.0;
/// Space around a line inside its tinted band.
const LINE_PAD_EM: f32 = 4.0 / 12.0;
/// Space above the block, and its padding (vertical, horizontal).
const MARGIN_EM: f32 = 4.0 / 12.0;
const PAD_EM: (f32, f32) = (8.0 / 12.0, 10.0 / 12.0);

type Rows<'a> = Column<Element<'a, PanelMsg>>;

/// How a tab of a stored row shows: the columns stay apart without tab stops.
const TAB_SHOWN: &str = "   ";

fn close_msg() -> PanelMsg {
    PanelMsg::History(HistoryPanelMsg::Commit(HistoryCommitMsg::DifferencesClosed))
}

/// A stored line as the list shows it.
pub(super) fn shown_line(text: &str) -> String {
    text.replace('\t', TAB_SHOWN)
}

/// The fills and mark inks of added and removed lines.
#[derive(Clone, Copy)]
struct LineLook {
    added_fill: Color,
    removed_fill: Color,
    added_ink: Color,
    removed_ink: Color,
}

impl LineLook {
    fn of(ctx: &PanelContext) -> Self {
        let r = resolve(&ctx.tokens);
        Self {
            added_fill: r.ok_soft,
            removed_fill: r.err_soft,
            added_ink: ti(ctx.tokens.success),
            removed_ink: ti(ctx.tokens.error),
        }
    }
}

fn note<'a>(s: &HistoryGraphStyle, words: Localized) -> Element<'a, PanelMsg> {
    text(words).size(s.caption_size).color(s.ink4).boxed()
}

fn header<'a>(i18n: &I18n, diff: &'a HistoryDiff, s: &HistoryGraphStyle) -> Element<'a, PanelMsg> {
    let path = text(UserText::new(diff.path.clone()))
        .size(s.caption_size)
        .font(Font::MONOSPACE)
        .color(s.ink)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .width(Length::Fill);
    let mut line = row![path].spacing(s.em(GAP_EM)).align_y(Alignment::Center);
    if let Some(Ok(file)) = &diff.result {
        let counts = t!(i18n, ids::PANELS_HISTORY_DIFF_COUNTS,
            "added" => file.added.to_string(), "removed" => file.removed.to_string());
        line = line.push(
            text(counts)
                .size(s.caption_size)
                .font(Font::MONOSPACE)
                .color(s.ink4)
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }
    line.push(small_button(
        s,
        t!(i18n, ids::PANELS_HISTORY_DIFF_CLOSE),
        Some(close_msg()),
    ))
    .boxed()
}

fn diff_line<'a>(
    s: &HistoryGraphStyle,
    i18n: &I18n,
    look: LineLook,
    line: &DiffLine,
) -> Element<'a, PanelMsg> {
    let (mark, fill, ink) = match line.kind {
        DiffLineKind::Added => (
            Some(ids::PANELS_HISTORY_DIFF_ADDED_SIGN),
            Some(look.added_fill),
            s.ink,
        ),
        DiffLineKind::Removed => (
            Some(ids::PANELS_HISTORY_DIFF_REMOVED_SIGN),
            Some(look.removed_fill),
            s.ink,
        ),
        DiffLineKind::Context => (None, None, s.ink3),
    };
    let mark_ink = if line.kind == DiffLineKind::Added {
        look.added_ink
    } else {
        look.removed_ink
    };
    let mark: Element<'a, PanelMsg> = match mark {
        Some(id) => text(t!(i18n, id))
            .size(s.caption_size)
            .font(Font::MONOSPACE)
            .color(mark_ink)
            .boxed(),
        None => Space::new().boxed(),
    };
    let body = row![
        container(mark).width(s.em(MARK_EM)),
        text(UserText::new(shown_line(&line.text)))
            .size(s.caption_size)
            .font(Font::MONOSPACE)
            .color(ink)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
    ];
    let radius = s.radii.small;
    container(body)
        .width(Length::Fill)
        .padding(Padding::from([0.0, s.em(LINE_PAD_EM)]))
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The runs of changed lines, or why there are none.
fn body<'a>(
    ctx: &PanelContext,
    file: &FileDiff,
    s: &HistoryGraphStyle,
    mut col: Rows<'a>,
) -> Rows<'a> {
    let i18n: &I18n = &ctx.i18n;
    let (hunks, truncated) = match &file.body {
        FileDiffBody::Binary => {
            return col.push(note(s, t!(i18n, ids::PANELS_HISTORY_DIFF_BINARY)));
        }
        FileDiffBody::Text { hunks, truncated } => (hunks, *truncated),
    };
    if hunks.is_empty() {
        return col.push(note(s, t!(i18n, ids::PANELS_HISTORY_DIFF_NO_LINES)));
    }
    let look = LineLook::of(ctx);
    for hunk in hunks {
        let start = t!(i18n, ids::PANELS_HISTORY_DIFF_HUNK,
            "old" => hunk.old_start.to_string(), "new" => hunk.new_start.to_string());
        let lines: Rows<'a> =
            Column::with_children(hunk.lines.iter().map(|line| diff_line(s, i18n, look, line)));
        col = col.push(
            Column::new()
                .spacing(s.em(GAP_EM) / 2.0)
                .push(note(s, start))
                .push(lines.boxed())
                .boxed(),
        );
    }
    if truncated {
        let count = MAX_DIFF_LINES.to_string();
        col = col.push(note(
            s,
            t!(i18n, ids::PANELS_HISTORY_DIFF_TRUNCATED, "count" => count),
        ));
    }
    col
}

/// The differences block of `diff`.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    diff: &'a HistoryDiff,
    s: &HistoryGraphStyle,
) -> Element<'a, PanelMsg> {
    let s = *s;
    let i18n: &I18n = &ctx.i18n;
    let col: Rows<'a> = Column::new()
        .width(Length::Fill)
        .spacing(s.em(GAP_EM))
        .push(header(i18n, diff, &s));
    let col = match &diff.result {
        None => col.push(note(&s, t!(i18n, ids::PANELS_HISTORY_DIFF_LOADING))),
        Some(Err(reason)) => col.push(note(
            &s,
            t!(i18n, ids::PANELS_HISTORY_DIFF_FAILED, "reason" => reason.clone()),
        )),
        Some(Ok(file)) => body(ctx, file, &s, col),
    };
    let block = container(col)
        .width(Length::Fill)
        .padding(Padding::from([s.em(PAD_EM.0), s.em(PAD_EM.1)]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(s.sunken)),
            text_color: Some(s.ink),
            border: Border {
                color: s.border,
                width: HAIRLINE,
                radius: s.radii.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(block)
        .width(Length::Fill)
        .padding(Padding {
            top: s.em(MARGIN_EM),
            bottom: s.em(MARGIN_EM),
            ..Padding::from([0.0, Spacing::from_base(s.text_size).m])
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{TimeZone, Utc};
    use signex_library::repo_diff::DiffHunk;
    use signex_widgets::history_graph::{ChangedFile, FileStatus, GraphCommit};

    use super::*;
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    use crate::panels::history::{HistoryRenderMode, HistoryRepoView, view_history};

    /// The panel as the mockup draws it: 340 px, tall enough to show all.
    const WINDOW: iced::Size = iced::Size::new(340.0, 4000.0);
    const SHA: &str = "9e01f4d3c2b1a0f9e8d7c6b5a4f3e2d1c0b9a8f7";
    const LONG: &str =
        "PART\tQ2\tSi2302CDS\t42100000\t31750000\t0\tN-channel MOSFET, lower Rds(on), SOT-23";

    fn commit() -> GraphCommit {
        GraphCommit {
            sha: SHA.into(),
            parents: Vec::new(),
            refs: Vec::new(),
            author_name: "a.yilmaz".into(),
            author_email: "a@example.com".into(),
            time: Utc.timestamp_opt(1_795_000_000, 0).unwrap(),
            subject: "Swap Q2 to a lower-Rds(on) part".into(),
        }
    }

    fn line(kind: DiffLineKind, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_line: None,
            new_line: None,
            text: text.into(),
        }
    }

    fn loaded() -> FileDiff {
        FileDiff {
            path: "power.snxsch".into(),
            added: 1,
            removed: 1,
            body: FileDiffBody::Text {
                hunks: vec![DiffHunk {
                    old_start: 12,
                    new_start: 12,
                    lines: vec![
                        line(DiffLineKind::Context, "WIRE\tw1"),
                        line(DiffLineKind::Removed, "PART\tQ2\tAO3400"),
                        line(DiffLineKind::Added, LONG),
                    ],
                }],
                truncated: false,
            },
        }
    }

    /// A History panel on the mockup's commit, its three files listed, and
    /// `diff` open under the strip.
    fn context(diff: Option<HistoryDiff>) -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        let history = &mut ctx.history;
        history.mode = HistoryRenderMode::Ready;
        history.repo = Some(Arc::new(HistoryRepoView {
            tracked: "power.snxsch".into(),
            commits: vec![commit()],
            restorable: [SHA.to_owned()].into_iter().collect(),
            ..HistoryRepoView::default()
        }));
        history.visible = Arc::new(vec![commit()]);
        history.ui.selected = Some(SHA.into());
        let file = |status, path: &str| ChangedFile {
            status,
            path: path.into(),
        };
        history.ui.files = vec![
            file(FileStatus::Modified, "power.snxsch"),
            file(FileStatus::Modified, "dcdc-buck.snxpcb"),
            file(FileStatus::Added, "assets/Q2.step"),
        ];
        history.ui.diff = diff;
        ctx
    }

    fn open(result: Option<Result<Arc<FileDiff>, String>>) -> Option<HistoryDiff> {
        Some(HistoryDiff {
            sha: SHA.into(),
            path: "power.snxsch".into(),
            result,
        })
    }

    fn texts(ctx: &PanelContext) -> Vec<String> {
        lay_out_texts(view_history(ctx), WINDOW)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    fn commit_msgs(ctx: &PanelContext, label: &str) -> Vec<HistoryCommitMsg> {
        messages_on_click(view_history(ctx), WINDOW, label)
            .into_iter()
            .filter_map(|msg| match msg {
                PanelMsg::History(HistoryPanelMsg::Commit(msg)) => Some(msg),
                _ => None,
            })
            .collect()
    }

    /// Mockup row 39: the strip's first action is Show differences…, and a
    /// press asks for the differences.
    #[test]
    fn the_strip_offers_show_differences_first() {
        let ctx = context(None);
        let shown = texts(&ctx);
        let first = shown
            .iter()
            .position(|t| t == "Show differences…")
            .expect("Show differences… is drawn");
        let board = shown
            .iter()
            .position(|t| t == "Show on board / sheet")
            .expect("Show on board / sheet is drawn");
        assert!(first < board, "{shown:?}");
        let pressed = commit_msgs(&ctx, "Show differences…");
        assert!(
            matches!(pressed[..], [HistoryCommitMsg::ShowDifferencesPressed]),
            "{pressed:?}"
        );
    }

    #[test]
    fn a_file_row_press_picks_that_file() {
        let pressed = commit_msgs(&context(None), "dcdc-buck.snxpcb");
        assert!(
            matches!(&pressed[..], [HistoryCommitMsg::FilePicked(path)] if path == "dcdc-buck.snxpcb"),
            "{pressed:?}"
        );
    }

    #[test]
    fn open_differences_list_the_changed_lines_with_their_counts() {
        let ctx = context(open(Some(Ok(Arc::new(loaded())))));
        let shown = texts(&ctx);
        for expected in [
            "+1 −1",
            "Line 12 before, line 12 after",
            "+",
            "−",
            "PART   Q2   AO3400",
            "WIRE   w1",
            shown_line(LONG).as_str(),
        ] {
            assert!(
                shown.iter().any(|t| t == expected),
                "{expected:?}: {shown:?}"
            );
        }
        let pressed = commit_msgs(&ctx, "Close");
        assert!(
            matches!(pressed[..], [HistoryCommitMsg::DifferencesClosed]),
            "{pressed:?}"
        );
    }

    /// A stored row is longer than the panel is wide: it wraps inside the
    /// panel instead of running past its edge.
    #[test]
    fn a_long_line_wraps_inside_the_panel() {
        let ctx = context(open(Some(Ok(Arc::new(loaded())))));
        let laid = lay_out_texts(view_history(&ctx), WINDOW);
        let long = shown_line(LONG);
        let row = laid
            .iter()
            .find(|t| t.text == long)
            .expect("the long line is drawn");
        assert!(row.bounds.x + row.bounds.width <= WINDOW.width, "{row:?}");
        let mark = laid.iter().find(|t| t.text == "+").expect("its mark");
        assert!(row.bounds.height > mark.bounds.height, "it wraps: {row:?}");
    }

    #[test]
    fn reading_binary_and_failed_differences_say_so() {
        let said = |result| texts(&context(open(result)));
        assert!(said(None).iter().any(|t| t == "Reading the differences…"));
        let binary = FileDiff {
            body: FileDiffBody::Binary,
            ..loaded()
        };
        assert!(
            said(Some(Ok(Arc::new(binary))))
                .iter()
                .any(|t| t == "Not a text file, so there are no lines to compare.")
        );
        assert!(
            said(Some(Err("object not found".into())))
                .iter()
                .any(|t| t == "Differences could not be read: object not found")
        );
    }

    /// Differences of another commit stay closed under this one's strip.
    #[test]
    fn differences_of_another_commit_are_not_shown() {
        let other = HistoryDiff {
            sha: "0b3f9e1".into(),
            ..open(Some(Ok(Arc::new(loaded())))).unwrap()
        };
        let shown = texts(&context(Some(other)));
        assert!(!shown.iter().any(|t| t == "Close"), "{shown:?}");
    }
}
