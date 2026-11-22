//! The History panel over a loaded repository (mockup row 39): the header
//! with the branch switcher, the filter chips and search, the commit graph
//! and the selected commit's detail strip. Pure: state in, elements out.

use std::path::Path;

use chrono::Utc;
use iced::widget::{Column, Row, Space, Stack, column, container, mouse_area, opaque, row, svg};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_widgets::branch_switcher::{
    BranchRow, BranchSwitcherIcons, BranchSwitcherMessages, BranchSwitcherState, branch_chip,
    branch_popover, chip_name,
};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::history_graph::{commit_detail, more_button, small_button};
use signex_widgets::theme_ext::TypeScale as GraphScale;
use signex_widgets::{HistoryGraph, HistoryGraphStyle, Label, button, text, text_input, tooltip};

use super::model::{
    HistoryBranchScope, HistoryCommitMsg, HistoryFilterMsg, HistoryPanelMsg, HistoryRepoView,
    HistorySwitcherMsg, RefPromptKind, can_create_branch, document_to_show,
};
use crate::panels::{PanelContext, PanelMsg, form_look, panel_scrollable};
use crate::styles::tokens::resolve;
use crate::styles::{file_kind_icon, ti};

/// Height of the header row; the switcher popover opens just under it.
const HEADER_EM: f32 = 44.0 / 12.0;
const HEADER_GAP_EM: f32 = 8.0 / 12.0;
const SIDE_PAD_EM: f32 = 10.0 / 12.0;
const ROW_GAP_EM: f32 = 5.0 / 12.0;
const ICON_BUTTON_EM: f32 = 22.0 / 12.0;
const ICON_EM: f32 = 14.0 / 12.0;

fn filter_msg(msg: HistoryFilterMsg) -> PanelMsg {
    PanelMsg::History(HistoryPanelMsg::Filter(msg))
}

fn commit_msg(msg: HistoryCommitMsg) -> PanelMsg {
    PanelMsg::History(HistoryPanelMsg::Commit(msg))
}

fn switcher_msg(msg: HistorySwitcherMsg) -> PanelMsg {
    PanelMsg::History(HistoryPanelMsg::Switcher(msg))
}

fn icons(ctx: &PanelContext) -> BranchSwitcherIcons {
    let tid = ctx.theme_id;
    BranchSwitcherIcons {
        branch: crate::icons::icon_chrome_git(tid),
        chevron: crate::icons::icon_chrome_chevron_down(tid),
        check: crate::icons::icon_chrome_check(tid),
        plus: crate::icons::icon_plus(tid),
    }
}

/// "23 commits · 1 branch ahead", or the commits alone with none ahead.
pub(super) fn counts_line(i18n: &I18n, commits: usize, ahead: usize) -> Localized {
    let commit_id = if commits == 1 {
        ids::PANELS_HISTORY_COMMITS_ONE
    } else {
        ids::PANELS_HISTORY_COMMITS_MANY
    };
    let commits = t!(i18n, commit_id, "count" => commits.to_string());
    if ahead == 0 {
        return commits;
    }
    let ahead_id = if ahead == 1 {
        ids::PANELS_HISTORY_BRANCHES_AHEAD_ONE
    } else {
        ids::PANELS_HISTORY_BRANCHES_AHEAD_MANY
    };
    let branches = t!(i18n, ahead_id, "count" => ahead.to_string());
    t!(i18n, ids::PANELS_HISTORY_COUNTS,
        "commits" => commits.into_string(), "branches" => branches.into_string())
}

fn branch_label<'a>(i18n: &I18n, repo: &HistoryRepoView) -> Label<'a> {
    match &repo.current_branch {
        Some(name) => UserText::new(chip_name(name)).into(),
        None => t!(i18n, ids::PANELS_HISTORY_DETACHED).into(),
    }
}

fn tip<'a>(s: &HistoryGraphStyle, label: Localized) -> Element<'a, PanelMsg> {
    let s = *s;
    container(text(label).size(s.caption_size).color(s.ink))
        .padding(Padding::from([s.em(ROW_GAP_EM), s.em(HEADER_GAP_EM)]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(s.panel)),
            border: Border {
                color: s.border,
                width: signex_widgets::theme_ext::HAIRLINE,
                radius: s.radii.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn header<'a>(
    ctx: &'a PanelContext,
    repo: &'a HistoryRepoView,
    s: &HistoryGraphStyle,
) -> Element<'a, PanelMsg> {
    let file = Path::new(&repo.tracked);
    let name = file.file_name().map_or_else(
        || repo.tracked.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    let counts = counts_line(&ctx.i18n, ctx.history.visible.len(), repo.branches_ahead());
    let names = column![
        text(UserText::new(name)).size(s.text_size).color(s.ink),
        text(counts).size(s.caption_size).color(s.ink4),
    ]
    .width(Length::Fill);
    let switch = branch_chip(
        s,
        branch_label(&ctx.i18n, repo),
        &icons(ctx),
        Some(switcher_msg(HistorySwitcherMsg::Toggled)),
    );
    let switch = tooltip(
        switch,
        tip(s, t!(ctx.i18n, ids::PANELS_HISTORY_BRANCH_SWITCH)),
        tooltip::Position::Bottom,
    );
    container(
        row![file_kind_icon::<PanelMsg>(file), names, switch]
            .spacing(s.em(HEADER_GAP_EM))
            .align_y(Alignment::Center),
    )
    .height(s.em(HEADER_EM))
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, s.em(SIDE_PAD_EM)]))
    .boxed()
}

fn search_button<'a>(ctx: &PanelContext, s: &HistoryGraphStyle) -> Element<'a, PanelMsg> {
    let s = *s;
    let side = s.em(ICON_EM);
    let face = container(
        svg(crate::icons::icon_chrome_search(ctx.theme_id))
            .width(side)
            .height(side),
    )
    .center(s.em(ICON_BUTTON_EM));
    let press = button(face)
        .padding(Padding::ZERO)
        .on_press(filter_msg(HistoryFilterMsg::SearchToggled))
        .style(move |_: &Theme, status: button::Status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(s.hover)),
                text_color: s.ink3,
                border: Border {
                    radius: s.radii.small.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });
    tooltip(
        press,
        tip(&s, t!(ctx.i18n, ids::PANELS_HISTORY_SEARCH)),
        tooltip::Position::Bottom,
    )
    .boxed()
}

fn filter_row<'a>(
    ctx: &'a PanelContext,
    repo: &'a HistoryRepoView,
    s: &HistoryGraphStyle,
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let style = ChipStyle {
        fill: r.field,
        edge: ti(ctx.tokens.border),
        hover: ti(ctx.tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let metrics = ChipMetrics::from_text_size(s.text_size);
    let filter = &ctx.history.ui.filter;
    let scope_chip = |label: Label<'a>, scope: HistoryBranchScope| {
        chip(
            label,
            None,
            filter.scope == scope,
            style,
            metrics,
            &ctx.i18n,
            Some(filter_msg(HistoryFilterMsg::ScopePicked(scope))),
        )
    };
    let all = t!(ctx.i18n, ids::PANELS_HISTORY_ALL_BRANCHES);
    container(
        row![
            scope_chip(all.into(), HistoryBranchScope::All),
            scope_chip(branch_label(&ctx.i18n, repo), HistoryBranchScope::Current),
            chip(
                t!(ctx.i18n, ids::PANELS_HISTORY_THIS_FILE),
                None,
                filter.this_file_only,
                style,
                metrics,
                &ctx.i18n,
                Some(filter_msg(HistoryFilterMsg::ThisFileToggled)),
            ),
            Space::new().width(Length::Fill),
            search_button(ctx, s),
        ]
        .spacing(s.em(ROW_GAP_EM))
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([s.em(ROW_GAP_EM), s.em(SIDE_PAD_EM)]))
    .boxed()
}

fn search_field<'a>(ctx: &'a PanelContext, s: &HistoryGraphStyle) -> Element<'a, PanelMsg> {
    container(
        text_input(
            t!(ctx.i18n, ids::PANELS_HISTORY_SEARCH_PLACEHOLDER),
            &ctx.history.ui.filter.search,
        )
        .on_input(|value| filter_msg(HistoryFilterMsg::SearchEdited(value)))
        .style(form_look::text_input_style)
        .size(s.text_size)
        .padding(s.em(ROW_GAP_EM)),
    )
    .padding(Padding::from([0.0, s.em(SIDE_PAD_EM)]))
    .boxed()
}

fn graph<'a>(ctx: &'a PanelContext, s: &HistoryGraphStyle) -> Element<'a, PanelMsg> {
    let state = &ctx.history;
    if state.visible.is_empty() && !state.dirty {
        return container(
            text(t!(ctx.i18n, ids::PANELS_HISTORY_NO_MATCHES))
                .size(s.text_size)
                .color(s.ink4),
        )
        .padding(s.em(SIDE_PAD_EM))
        .boxed();
    }
    let mut graph = HistoryGraph::new(&state.visible, *s, &ctx.i18n, Utc::now())
        .selected(state.ui.selected.as_deref())
        .on_select(|sha| commit_msg(HistoryCommitMsg::Selected(sha)));
    if state.dirty {
        let on_commit = state
            .ui
            .commits_on_save
            .then(|| commit_msg(HistoryCommitMsg::CommitPressed));
        graph = graph.working_tree(on_commit);
    }
    graph.view()
}

/// The buttons of the strip: Show on board / sheet, Restore (or its confirm
/// pair) when the commit holds the file, and the more button.
fn detail_actions<'a>(
    ctx: &'a PanelContext,
    repo: &'a HistoryRepoView,
    sha: &str,
    s: &HistoryGraphStyle,
) -> Vec<Element<'a, PanelMsg>> {
    let state = &ctx.history;
    let i18n: &I18n = &ctx.i18n;
    let show = document_to_show(&state.ui.files, &repo.tracked)
        .map(|path| commit_msg(HistoryCommitMsg::ShowDocumentPressed(path)));
    let mut actions = vec![small_button(
        s,
        t!(i18n, ids::PANELS_HISTORY_SHOW_ON_BOARD),
        show,
    )];
    if repo.restorable.contains(sha) {
        let restore = Some(PanelMsg::HistoryRestoreClicked {
            sha: sha.to_owned(),
        });
        if state.pending_restore.as_deref() == Some(sha) {
            actions.push(small_button(
                s,
                t!(i18n, ids::PANELS_HISTORY_CANCEL),
                Some(PanelMsg::HistoryRestoreCancelled),
            ));
            actions.push(small_button(
                s,
                t!(i18n, ids::PANELS_HISTORY_DISCARD_AND_RESTORE),
                restore,
            ));
        } else {
            actions.push(small_button(
                s,
                t!(i18n, ids::PANELS_HISTORY_RESTORE),
                restore,
            ));
        }
    }
    actions.push(more_button(
        s,
        i18n,
        Some(commit_msg(HistoryCommitMsg::MoreToggled)),
    ));
    actions
}

/// The more menu's rows, or the name field one of them opened.
fn more_row<'a>(ctx: &'a PanelContext, s: &HistoryGraphStyle) -> Option<Element<'a, PanelMsg>> {
    let ui = &ctx.history.ui;
    let i18n: &I18n = &ctx.i18n;
    let inset = Padding::from([0.0, s.em(SIDE_PAD_EM)]);
    if let Some(prompt) = &ui.prompt {
        let placeholder = match prompt.kind {
            RefPromptKind::Branch => ids::PANELS_HISTORY_BRANCH_NAME,
            RefPromptKind::Tag => ids::PANELS_HISTORY_TAG_NAME,
        };
        let field = text_input(t!(i18n, placeholder), &prompt.name)
            .on_input(|value| commit_msg(HistoryCommitMsg::PromptEdited(value)))
            .on_submit(commit_msg(HistoryCommitMsg::PromptSubmitted))
            .style(form_look::text_input_style)
            .size(s.text_size)
            .padding(s.em(ROW_GAP_EM));
        let line = row![
            field,
            small_button(
                s,
                t!(i18n, ids::PANELS_HISTORY_CREATE),
                Some(commit_msg(HistoryCommitMsg::PromptSubmitted)),
            ),
            small_button(
                s,
                t!(i18n, ids::PANELS_HISTORY_CANCEL),
                Some(commit_msg(HistoryCommitMsg::PromptCancelled)),
            ),
        ]
        .spacing(s.em(ROW_GAP_EM))
        .align_y(Alignment::Center);
        return Some(container(line).padding(inset).boxed());
    }
    if !ui.more_open {
        return None;
    }
    let item = |id, msg| small_button(s, t!(i18n, id), Some(commit_msg(msg)));
    let items: Row<Element<'a, PanelMsg>> = Row::with_children([
        item(
            ids::PANELS_HISTORY_NEW_BRANCH_HERE,
            HistoryCommitMsg::NewBranchPressed,
        ),
        item(ids::PANELS_HISTORY_TAG, HistoryCommitMsg::TagPressed),
        item(
            ids::PANELS_HISTORY_COPY_SHA,
            HistoryCommitMsg::CopyShaPressed,
        ),
    ])
    .spacing(s.em(ROW_GAP_EM));
    Some(
        container(items.wrap().vertical_spacing(s.em(ROW_GAP_EM)))
            .padding(inset)
            .boxed(),
    )
}

fn detail<'a>(
    ctx: &'a PanelContext,
    repo: &'a HistoryRepoView,
    s: &HistoryGraphStyle,
) -> Option<Element<'a, PanelMsg>> {
    let state = &ctx.history;
    let sha = state.ui.selected.as_deref()?;
    let commit = repo.commits.iter().find(|c| c.sha == sha)?;
    let actions = detail_actions(ctx, repo, sha, s);
    let strip = commit_detail(s, &ctx.i18n, commit, &state.ui.files, actions);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill).push(strip);
    if let Some(more) = more_row(ctx, s) {
        col = col.push(more);
    }
    Some(col.boxed())
}

/// `body` with the switcher open over it: a clear backdrop closes it, and
/// the card sits under the chip at the right of the header.
fn with_switcher<'a>(
    body: Element<'a, PanelMsg>,
    ctx: &'a PanelContext,
    repo: &'a HistoryRepoView,
    s: &HistoryGraphStyle,
) -> Element<'a, PanelMsg> {
    let ui = &ctx.history.ui;
    let notice: Option<Label<'a>> = ui
        .switch_block
        .as_ref()
        .map(|block| crate::app::project_git::view::block_words(block, &ctx.i18n).into());
    let on_save_and_switch = ui.save_offered.then(|| {
        (
            Label::from(t!(ctx.i18n, ids::PROJECT_VCS_SWITCH_SAVE_AND_SWITCH)),
            switcher_msg(HistorySwitcherMsg::SaveAndSwitchPressed),
        )
    });
    let on_create = can_create_branch(&repo.branches, &ui.switcher_query)
        .then(|| switcher_msg(HistorySwitcherMsg::CreatePressed));
    let card = branch_popover(
        s,
        &ctx.i18n,
        BranchSwitcherState {
            rows: &repo.branches,
            query: &ui.switcher_query,
            notice,
            notice_ink: ti(ctx.tokens.warning),
        },
        &icons(ctx),
        BranchSwitcherMessages {
            on_query: Box::new(|value| switcher_msg(HistorySwitcherMsg::QueryEdited(value))),
            on_pick: Box::new(|branch: &BranchRow| {
                switcher_msg(HistorySwitcherMsg::Picked {
                    name: branch.name.clone(),
                    is_remote: branch.is_remote,
                })
            }),
            on_create,
            on_save_and_switch,
        },
    );
    let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(switcher_msg(HistorySwitcherMsg::Dismissed));
    let anchored = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(Space::new().height(s.em(HEADER_EM)).boxed())
        .push(
            row![
                Space::new().width(Length::Fill),
                opaque(card).capture_scroll(true),
                Space::new().width(s.em(SIDE_PAD_EM)),
            ]
            .boxed(),
        );
    Stack::new()
        .push(body)
        .push(backdrop.boxed())
        .push(anchored.boxed())
        .boxed()
}

/// The whole panel over a loaded repository.
pub(super) fn view<'a>(ctx: &'a PanelContext, repo: &'a HistoryRepoView) -> Element<'a, PanelMsg> {
    let style = HistoryGraphStyle::from_tokens(&ctx.tokens, &GraphScale::DEFAULT);
    let ui = &ctx.history.ui;
    let mut top: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .push(header(ctx, repo, &style))
        .push(filter_row(ctx, repo, &style));
    if ui.search_open {
        top = top.push(search_field(ctx, &style));
    }
    let mut rows: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).push(graph(ctx, &style));
    if let Some(strip) = detail(ctx, repo, &style) {
        rows = rows.push(strip);
    }
    let body = top
        .push(
            panel_scrollable(rows.boxed())
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        )
        .height(Length::Fill)
        .boxed();
    if ui.switcher_open {
        with_switcher(body, ctx, repo, &style)
    } else {
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: Localized) -> String {
        text.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    /// The header line as the mockup words it.
    #[test]
    fn the_counts_read_as_the_mockup_draws_them() {
        let i18n = I18n::en_us();
        assert_eq!(
            plain(counts_line(&i18n, 23, 1)),
            "23 commits · 1 branch ahead"
        );
        assert_eq!(plain(counts_line(&i18n, 1, 0)), "1 commit");
        assert_eq!(
            plain(counts_line(&i18n, 5, 2)),
            "5 commits · 2 branches ahead"
        );
    }
}
