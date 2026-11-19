//! The Projects page (mock m1, "Projects page"): every pinned and recent
//! project, pinned first in the user's order and then the recent ones
//! newest first, with no cap. A filter matches the name or folder, and the
//! All / Pinned / Missing switch picks which projects show. A filter that
//! matches nothing says so and offers Clear filter.

use std::path::{Path, PathBuf};

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{Label, SegmentedStyle, segmented};

use super::kit::{
    GLYPH_THEME, Look, Size, action_button, empty_state, filter_field, filter_width, page_heading,
};
use super::projects::{RowKind, Rows, list_or_empty, no_match, project_row};
use super::{Action, ProjectScope, StartScreenMsg, StartScreenState, message};
use crate::app::Message;
use crate::pinned_projects::PinnedProjects;
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the All / Pinned / Missing switch (220 px at the default base).
const SCOPE_W: f32 = TS.body * 55.0 / 3.0;

/// One project the Projects page lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectListing {
    pub path: PathBuf,
    pub pinned: bool,
    pub missing: bool,
}

/// How many projects each switch position lists, before the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScopeCounts {
    pub all: usize,
    pub pinned: usize,
    pub missing: usize,
}

/// Every pinned and recent project, pinned first: what the page lists
/// with no filter and the switch on All.
fn everything(recent: &[PathBuf], pinned: &PinnedProjects) -> Vec<ProjectListing> {
    let pins = pinned.entries().iter().map(|path| ProjectListing {
        path: path.clone(),
        pinned: true,
        missing: pinned.is_missing(path),
    });
    let rest = recent
        .iter()
        .filter(|path| !pinned.contains(path))
        .map(|path| ProjectListing {
            path: path.clone(),
            pinned: false,
            missing: false,
        });
    pins.chain(rest).collect()
}

/// How many projects the page lists in each switch position.
pub(super) fn scope_counts(recent: &[PathBuf], pinned: &PinnedProjects) -> ScopeCounts {
    let all = everything(recent, pinned);
    ScopeCounts {
        all: all.len(),
        pinned: all.iter().filter(|p| p.pinned).count(),
        missing: all.iter().filter(|p| p.missing).count(),
    }
}

/// Whether `path` matches the filter `query`: a case-insensitive substring
/// of its name or folder, folded as the command palette folds.
fn matches(i18n: &I18n, path: &Path, query: &str) -> bool {
    let query = i18n.fold_for_search(query.trim());
    query.is_empty()
        || i18n
            .fold_for_search(&path.display().to_string())
            .contains(&query)
}

/// The projects the page lists for `filter` and `scope`, in page order.
pub fn project_listing(
    i18n: &I18n,
    recent: &[PathBuf],
    pinned: &PinnedProjects,
    filter: &str,
    scope: ProjectScope,
) -> Vec<ProjectListing> {
    everything(recent, pinned)
        .into_iter()
        .filter(|p| match scope {
            ProjectScope::All => true,
            ProjectScope::Pinned => p.pinned,
            ProjectScope::Missing => p.missing,
        })
        .filter(|p| matches(i18n, &p.path, filter))
        .collect()
}

/// The page: heading, toolbar and the list (or its empty state).
pub(super) fn view<'a>(
    rows: Rows<'a>,
    pinned: &'a PinnedProjects,
    state: &'a StartScreenState,
    main_w: f32,
) -> Vec<Element<'a, Message>> {
    let (tokens, i18n) = (rows.tokens, rows.i18n);
    let recent = rows.recent.entries();
    let counts = scope_counts(recent, pinned);
    let scope = segmented(
        [
            (
                ProjectScope::All,
                Label::from(t!(i18n, ids::START_SCOPE_ALL, "count" => counts.all)),
            ),
            (
                ProjectScope::Pinned,
                Label::from(t!(i18n, ids::START_SCOPE_PINNED, "count" => counts.pinned)),
            ),
            (
                ProjectScope::Missing,
                Label::from(t!(i18n, ids::START_SCOPE_MISSING, "count" => counts.missing)),
            ),
        ],
        state.project_scope,
        SegmentedStyle::new(
            resolve(tokens).sunken,
            resolve(tokens).raised,
            resolve(tokens).shadow,
            resolve(tokens).ink3,
            ti(tokens.text),
            TS.body,
        )
        .fill_width(true),
        |scope| message(StartScreenMsg::ProjectScopePicked(scope)),
    );
    let toolbar = row![
        filter_field(
            tokens,
            t!(i18n, ids::START_PROJECTS_FILTER),
            &state.project_filter,
            |text| message(StartScreenMsg::ProjectFilterChanged(text)),
            filter_width(main_w),
        ),
        container(scope).width(SCOPE_W),
        Space::new().width(Length::Fill),
        action_button(
            tokens,
            (Look::Outline, Size::Regular),
            Some(crate::icons::icon_start_folder(GLYPH_THEME)),
            t!(i18n, ids::START_OPEN),
            Some(Action::OpenProject.message()),
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let listed: Vec<Element<'a, Message>> = project_listing(
        i18n,
        recent,
        pinned,
        &state.project_filter,
        state.project_scope,
    )
    .iter()
    .map(|p| project_row(rows, &p.path, RowKind::of(&p.path, pinned), false))
    .collect();
    let query = state.project_filter.trim();
    let list = list_or_empty(tokens, listed, || {
        if !query.is_empty() {
            return no_match(tokens, i18n, ids::START_PROJECTS_NO_MATCH, query);
        }
        let (title, body) = match state.project_scope {
            ProjectScope::All => (ids::START_NO_RECENT_TITLE, ids::START_NO_RECENT_BODY),
            ProjectScope::Pinned => (
                ids::START_PROJECTS_NONE_PINNED,
                ids::START_PROJECTS_NONE_PINNED_BODY,
            ),
            ProjectScope::Missing => (
                ids::START_PROJECTS_NONE_MISSING,
                ids::START_PROJECTS_NONE_MISSING_BODY,
            ),
        };
        empty_state(tokens, i18n, (t!(i18n, title), t!(i18n, body)), false)
    });
    vec![
        page_heading(
            tokens,
            t!(i18n, ids::START_PROJECTS),
            t!(i18n, ids::START_PROJECTS_BODY),
        ),
        column![toolbar, list].spacing(SP.l).boxed(),
    ]
}
