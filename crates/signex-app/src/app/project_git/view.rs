//! The branch button and the switcher list (mockup rows 35 and 39). Both
//! speak [`ProjectGitMsg`], so the Project tab and the History panel embed
//! the same widgets and map them into their own message.

use std::sync::LazyLock;

use iced::widget::{Column, Row, column, container, svg};
use iced::{Alignment, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_library::adapters::local_git::project_repo::{BranchFacts, SwitchRefusal};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text};

use super::{AuthorProblem, BRANCH_GLYPH, ProjectGitMsg, RepoView, SwitchBlock};
use crate::app::state::ProjectId;
use crate::styles::{PillTone, Spacing, TypeScale, pill_colors, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Side of the branch glyph, beside body text.
const GLYPH: f32 = TS.body * 7.0 / 6.0;

static BRANCH: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(BRANCH_GLYPH));

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn branch_glyph<'a>(ink: Color) -> Element<'a, ProjectGitMsg> {
    svg(BRANCH.clone())
        .width(Length::Fixed(GLYPH))
        .height(Length::Fixed(GLYPH))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The branch shown on the button.
fn branch_words(repo: Option<&RepoView>, i18n: &I18n) -> Label<'static> {
    match repo.and_then(|repo| repo.facts.as_ref()) {
        Some(facts) => match &facts.branch {
            Some(name) => UserText::new(name.clone()).into(),
            None => t!(i18n, ids::PROJECT_VCS_BRANCH_DETACHED).into(),
        },
        None => t!(i18n, ids::PROJECT_VCS_STATE_CHECKING).into(),
    }
}

/// The button naming the branch checked out; a press opens the switcher.
pub(crate) fn branch_button<'a>(
    project_id: ProjectId,
    repo: Option<&RepoView>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, ProjectGitMsg> {
    let ink = ti(tokens.text);
    let face: Row<Element<'a, ProjectGitMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(branch_glyph(resolve(tokens).ink3))
        .push(
            text(branch_words(repo, i18n))
                .size(TS.body)
                .font(mono())
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    button(face)
        .padding([SP.xs, SP.m])
        .on_press(ProjectGitMsg::SwitcherToggled { project_id })
        .style(crate::styles::outline_button(tokens))
        .boxed()
}

/// How far `branch` is from the current one, in words.
pub(super) fn distance_words(branch: &BranchFacts, i18n: &I18n) -> Option<Localized> {
    match (branch.ahead, branch.behind) {
        (0, 0) => None,
        (ahead, 0) => Some(t!(i18n, ids::PROJECT_VCS_BRANCH_AHEAD, "count" => ahead)),
        (0, behind) => Some(t!(i18n, ids::PROJECT_VCS_BRANCH_BEHIND, "count" => behind)),
        (ahead, behind) => Some(t!(
            i18n,
            ids::PROJECT_VCS_BRANCH_AHEAD_BEHIND,
            "ahead" => ahead,
            "behind" => behind
        )),
    }
}

fn branch_row<'a>(
    project_id: ProjectId,
    branch: &BranchFacts,
    is_busy: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, ProjectGitMsg> {
    let r = resolve(tokens);
    let mut line: Row<Element<'a, ProjectGitMsg>> =
        Row::new().spacing(SP.m).align_y(Alignment::Center).push(
            text(UserText::new(branch.name.clone()))
                .size(TS.body)
                .font(mono())
                .color(ti(tokens.text))
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
        );
    if branch.is_current {
        let ink = pill_colors(tokens, PillTone::Ok).1;
        line = line.push(
            text(t!(i18n, ids::PROJECT_VCS_BRANCH_CURRENT))
                .size(TS.caption)
                .color(ink)
                .boxed(),
        );
    } else if let Some(words) = distance_words(branch, i18n) {
        line = line.push(text(words).size(TS.caption).color(r.ink4).boxed());
    }
    let pick = (!is_busy).then(|| ProjectGitMsg::BranchPicked {
        project_id,
        name: branch.name.clone(),
    });
    button(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.s])
        .on_press_maybe(pick)
        .style(crate::styles::ghost_button(tokens))
        .boxed()
}

/// Why the last switch did not happen, in the catalog's words.
pub(super) fn block_words(block: &SwitchBlock, i18n: &I18n) -> Localized {
    match block {
        SwitchBlock::UnsavedEdits(file) => {
            t!(i18n, ids::PROJECT_VCS_SWITCH_UNSAVED, "file" => file.as_str())
        }
        SwitchBlock::WorkRunning => t!(i18n, ids::PROJECT_VCS_SWITCH_BUSY),
        SwitchBlock::Refused(refusal) => match refusal {
            SwitchRefusal::UnknownBranch => t!(i18n, ids::PROJECT_VCS_SWITCH_UNKNOWN),
            SwitchRefusal::NoCommits => t!(i18n, ids::PROJECT_VCS_SWITCH_NO_COMMITS),
            SwitchRefusal::ProjectFileDiffers => t!(i18n, ids::PROJECT_VCS_SWITCH_PROJECT_FILE),
            SwitchRefusal::LfsFileDiffers(file) => {
                t!(i18n, ids::PROJECT_VCS_SWITCH_LFS, "file" => file.as_str())
            }
            SwitchRefusal::WouldOverwrite => t!(i18n, ids::PROJECT_VCS_SWITCH_OVERWRITE),
            SwitchRefusal::Failed(why) => {
                t!(i18n, ids::PROJECT_VCS_SWITCH_FAILED, "why" => why.as_str())
            }
        },
    }
}

/// Why the typed author was not written, in the catalog's words.
pub(crate) fn author_problem_words(problem: &AuthorProblem, i18n: &I18n) -> Localized {
    match problem {
        AuthorProblem::Unreadable => t!(i18n, ids::PROJECT_VCS_AUTHOR_INVALID),
        AuthorProblem::WriteFailed(why) => {
            t!(i18n, ids::PROJECT_VCS_AUTHOR_FAILED, "why" => why.as_str())
        }
    }
}

/// The open switcher: the local branches, and why the last pick was refused.
pub(crate) fn switcher<'a>(
    project_id: ProjectId,
    repo: Option<&RepoView>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, ProjectGitMsg> {
    let r = resolve(tokens);
    let is_busy = repo.is_some_and(|repo| repo.switching.is_some());
    let mut rows: Column<Element<'a, ProjectGitMsg>> =
        Column::new().spacing(SP.xs).width(Length::Fill);
    for branch in repo
        .and_then(|repo| repo.facts.as_ref())
        .map_or(&[][..], |facts| facts.branches.as_slice())
    {
        rows = rows.push(branch_row(project_id, branch, is_busy, i18n, tokens));
    }
    let mut body: Column<Element<'a, ProjectGitMsg>> = column![
        text(t!(i18n, ids::PROJECT_VCS_SWITCHER_HEADING))
            .size(TS.caption)
            .color(r.ink3),
        rows,
    ]
    .spacing(SP.s)
    .width(Length::Fill);
    if let Some(block) = repo.and_then(|repo| repo.block.as_ref()) {
        body = body.push(
            text(block_words(block, i18n))
                .size(TS.caption)
                .color(ti(tokens.warning))
                .boxed(),
        );
    }
    container(body)
        .width(Length::Fill)
        .padding(SP.m)
        .style(crate::styles::context_menu(tokens))
        .boxed()
}
