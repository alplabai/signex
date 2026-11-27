//! The Libraries page (mockup row 1, "Libraries page"): the libraries loaded for
//! every project, Stock first, in a table of name and path, scope,
//! component count and version control, each with Open (the library's
//! Library Browser tab; Stock opens read-only, as it is mounted). The
//! toolbar filters the table and offers Add library… (the Components
//! panel's Add ▸ Global) and Open Library Browser.
//!
//! Version control (#2591) reads `—` for Stock, `git`, `off` (a link that
//! opens Enable Version Control) or "needs attention" with its reason
//! inline; the state is read off the UI thread into
//! [`StartScreenState::library_vcs`]. A Global or Installed row has a ⋯
//! menu with Remove from Signex (#2592).

use std::path::{Path, PathBuf};

use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_library::library_vcs::{LibraryVcsState, VcsAttention};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::kit::{
    GLYPH_THEME, Look, Size, action_button, command, empty_state, filter_field, filter_width,
    list_card, mono, page_heading, pill, section_header, weighted,
};
use super::projects::no_match;
use super::{StartScreenMsg, StartScreenState, message};
use crate::app::Message;
use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsMountSource;
use crate::styles::{PillTone, Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the Scope column.
const SCOPE_W: f32 = TS.body * 7.0;
/// Width of the Components column.
const COUNT_W: f32 = TS.body * 8.0;
/// Width of the Version control column; a reason wraps inside it.
const VCS_W: f32 = TS.body * 11.0;
/// Width of the row actions column (Open and the ⋯ button).
const ACTIONS_W: f32 = TS.body * 7.0;

/// A loaded library as the Start screen lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedLibrary {
    pub name: String,
    /// The `.snxlib` file.
    pub path: PathBuf,
    pub components: usize,
    pub scope: ComponentsMountSource,
    /// The folder's version control as last read; `None` before the first
    /// read lands, and always for Stock, which shows `—`.
    pub vcs: Option<LibraryVcsState>,
}

/// The folder a library file's version control lives in: the one holding
/// the `.snxlib`, as Enable Version Control scopes it.
pub fn library_folder(path: &Path) -> Option<&Path> {
    path.parent()
}

/// The mounted libraries, Stock first and then in the Components panel's
/// source order, with the version control `state` holds for each. Nothing
/// is open on the Start screen, so no library counts as a project's.
pub fn loaded_libraries(
    library: &crate::library::LibraryState,
    state: &StartScreenState,
) -> Vec<LoadedLibrary> {
    let mut out: Vec<LoadedLibrary> = library
        .open_libraries
        .iter()
        .map(|lib| {
            let scope = library.mount_source_for(&lib.root, &[]);
            let vcs = (scope != ComponentsMountSource::Stock)
                .then(|| library_folder(&lib.root))
                .flatten()
                .and_then(|dir| state.library_vcs.get(dir).cloned());
            LoadedLibrary {
                name: lib.display_name.clone(),
                path: lib.root.clone(),
                components: lib.cached_components.len(),
                scope,
                vcs,
            }
        })
        .collect();
    let rank = |scope: ComponentsMountSource| {
        ComponentsMountSource::ORDER
            .iter()
            .position(|s| *s == scope)
            .unwrap_or(usize::MAX)
    };
    out.sort_by_key(|lib| rank(lib.scope));
    out
}

/// Whether `library` matches the filter `query`: a case-insensitive
/// substring of its name or path.
pub(super) fn matches(i18n: &I18n, library: &LoadedLibrary, query: &str) -> bool {
    let query = i18n.fold_for_search(query.trim());
    query.is_empty()
        || i18n.fold_for_search(&library.name).contains(&query)
        || i18n
            .fold_for_search(&library.path.display().to_string())
            .contains(&query)
}

/// The page: heading, toolbar and the table (or its empty state).
pub(super) fn view<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    libraries: &[LoadedLibrary],
    state: &'a StartScreenState,
    main_w: f32,
) -> Vec<Element<'a, Message>> {
    let toolbar = row![
        filter_field(
            tokens,
            t!(i18n, ids::START_LIBRARIES_FILTER),
            &state.library_filter,
            |text| message(StartScreenMsg::LibraryFilterChanged(text)),
            filter_width(main_w),
        ),
        Space::new().width(Length::Fill),
        action_button(
            tokens,
            (Look::Outline, Size::Regular),
            Some(crate::icons::icon_plus(GLYPH_THEME)),
            t!(i18n, ids::START_ADD_LIBRARY),
            Some(Message::Library(LibraryMessage::ComponentsPanelAddLibrary(
                ComponentsMountSource::Global
            ))),
        ),
        action_button(
            tokens,
            (Look::Primary, Size::Regular),
            None,
            t!(i18n, ids::START_OPEN_LIBRARY_BROWSER),
            command("open_library_browser"),
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let query = state.library_filter.trim();
    let shown: Vec<&LoadedLibrary> = libraries
        .iter()
        .filter(|lib| matches(i18n, lib, query))
        .collect();
    let table = if shown.is_empty() {
        let empty = if query.is_empty() {
            empty_state(
                tokens,
                i18n,
                (
                    t!(i18n, ids::START_LIBRARIES_NONE),
                    t!(i18n, ids::START_LIBRARIES_NONE_BODY),
                ),
                false,
            )
        } else {
            no_match(tokens, i18n, ids::START_LIBRARIES_NO_MATCH, query)
        };
        list_card(tokens, vec![empty])
    } else {
        let mut rows = vec![header(tokens, i18n)];
        rows.extend(shown.into_iter().map(|lib| library_row(tokens, i18n, lib)));
        list_card(tokens, rows)
    };
    vec![
        page_heading(
            tokens,
            t!(i18n, ids::START_LIBRARIES),
            t!(i18n, ids::START_LIBRARIES_BODY),
        ),
        column![toolbar, table].spacing(SP.l).boxed(),
    ]
}

/// The five cells of a table row, in column widths.
fn cells<'a>(
    library: Element<'a, Message>,
    scope: Element<'a, Message>,
    count: Element<'a, Message>,
    vcs: Element<'a, Message>,
    actions: Element<'a, Message>,
) -> Row<Element<'a, Message>> {
    row![
        container(library).width(Length::Fill),
        container(scope).width(SCOPE_W),
        container(count)
            .width(COUNT_W)
            .align_x(iced::alignment::Horizontal::Right),
        container(vcs).width(VCS_W),
        container(actions)
            .width(ACTIONS_W)
            .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(SP.l)
    .align_y(Alignment::Center)
}

/// The column headings on the sunken band at the top of the table.
fn header<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, Message> {
    let band = resolve(tokens).sunken;
    let corner = Radii::GRAPHITE.card - 1.0;
    container(cells(
        section_header(tokens, t!(i18n, ids::START_LIBRARIES_COLUMN_LIBRARY)),
        section_header(tokens, t!(i18n, ids::START_LIBRARIES_COLUMN_SCOPE)),
        section_header(tokens, t!(i18n, ids::START_LIBRARIES_COLUMN_COMPONENTS)),
        section_header(
            tokens,
            t!(i18n, ids::START_LIBRARIES_COLUMN_VERSION_CONTROL),
        ),
        Space::new().boxed(),
    ))
    .padding([SP.s + SP.xs / 2.0, SP.l])
    .width(Length::Fill)
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(band)),
        border: Border {
            radius: iced::border::Radius {
                top_left: corner,
                top_right: corner,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

fn library_row<'a>(tokens: &ThemeTokens, i18n: &I18n, lib: &LoadedLibrary) -> Element<'a, Message> {
    let stock = lib.scope == ComponentsMountSource::Stock;
    let caption: Element<'a, Message> = if stock {
        caption_text(tokens, t!(i18n, ids::START_LIBRARIES_STOCK_CAPTION))
    } else {
        caption_text(tokens, UserText::new(lib.path.display().to_string()))
    };
    let name = column![
        text(UserText::new(lib.name.clone()))
            .size(TS.body)
            .font(weighted(iced::font::Weight::Medium))
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
        caption,
    ]
    .spacing(SP.xs / 2.0)
    .width(Length::Fill);
    let tone = if lib.scope == ComponentsMountSource::Global {
        PillTone::Accent
    } else {
        PillTone::Neutral
    };
    let components = i64::try_from(lib.components).unwrap_or(i64::MAX);
    let count = text(t!(i18n, ids::START_COUNT, "count" => i18n.count(components)))
        .size(TS.body)
        .font(mono())
        .color(ti(tokens.text_secondary))
        .wrapping(iced::widget::text::Wrapping::None);
    let open = action_button(
        tokens,
        (Look::Outline, Size::Small),
        None,
        t!(i18n, ids::START_OPEN_PROJECT),
        Some(Message::Library(LibraryMessage::OpenLibraryBrowser(
            lib.path.clone(),
        ))),
    );
    let mut actions = row![open].spacing(SP.xs).align_y(Alignment::Center);
    if is_removable(lib.scope) {
        actions = actions.push(action_button(
            tokens,
            (Look::Ghost, Size::Small),
            None,
            t!(i18n, ids::START_LIBRARY_ROW_MENU),
            Some(message(StartScreenMsg::LibraryMenuOpened(lib.path.clone()))),
        ));
    }
    container(cells(
        name.boxed(),
        pill(tokens, t!(i18n, lib.scope.message()), tone),
        count.boxed(),
        vcs_cell(tokens, i18n, lib),
        actions.boxed(),
    ))
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// Whether Remove from Signex applies to a library of `scope` (#2592):
/// Global and Installed libraries are registered with Signex; Stock ships
/// with it and a project's own come from its `.snxprj`.
pub fn is_removable(scope: ComponentsMountSource) -> bool {
    matches!(
        scope,
        ComponentsMountSource::Global | ComponentsMountSource::Installed
    )
}

/// The Version control cell (#2591): `—` for Stock, a `git` pill, `off` as
/// a link to Enable Version Control, or a "needs attention" pill over its
/// reason. Empty until the state is read.
fn vcs_cell<'a>(tokens: &ThemeTokens, i18n: &I18n, lib: &LoadedLibrary) -> Element<'a, Message> {
    if lib.scope == ComponentsMountSource::Stock {
        return quiet_text(tokens, t!(i18n, ids::START_LIBRARIES_VCS_NONE));
    }
    match &lib.vcs {
        None => Space::new().boxed(),
        Some(LibraryVcsState::Git) => {
            pill(tokens, t!(i18n, ids::START_LIBRARIES_VCS_GIT), PillTone::Ok)
        }
        Some(LibraryVcsState::Off) => action_button(
            tokens,
            (Look::Ghost, Size::Small),
            None,
            t!(i18n, ids::START_LIBRARIES_VCS_OFF),
            Some(message(StartScreenMsg::LibraryVcsEnableClicked(
                lib.path.clone(),
            ))),
        ),
        Some(LibraryVcsState::NeedsAttention(why)) => column![
            pill(
                tokens,
                t!(i18n, ids::START_LIBRARIES_VCS_ATTENTION),
                PillTone::Warn,
            ),
            text(attention_reason(i18n, why))
                .size(TS.caption)
                .color(resolve(tokens).ink4)
                .width(Length::Fill),
        ]
        .spacing(SP.xs / 2.0)
        .boxed(),
    }
}

/// The reason a library's repository needs attention, as the cell says it.
pub(super) fn attention_reason(i18n: &I18n, why: &VcsAttention) -> Localized {
    match why {
        VcsAttention::GitNotFound => t!(i18n, ids::START_LIBRARIES_VCS_GIT_MISSING),
        VcsAttention::LfsNotFound => t!(i18n, ids::START_LIBRARIES_VCS_LFS_MISSING),
        VcsAttention::Unreadable(reason) => t!(
            i18n,
            ids::START_LIBRARIES_VCS_UNREADABLE,
            "reason" => reason.as_str()
        ),
        VcsAttention::Uncommitted(count) => t!(
            i18n,
            ids::START_LIBRARIES_VCS_UNCOMMITTED,
            "count" => *count
        ),
    }
}

/// A short line in the secondary ink, as the mock's `.caption` cells.
fn quiet_text<'a>(tokens: &ThemeTokens, line: Localized) -> Element<'a, Message> {
    text(line)
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// A mono caption line that ends in an ellipsis when long.
fn caption_text<'a>(
    tokens: &ThemeTokens,
    line: impl Into<signex_widgets::Label<'a>>,
) -> Element<'a, Message> {
    text(line)
        .size(TS.caption)
        .font(mono())
        .color(resolve(tokens).ink4)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
pub(super) fn sample(name: &str, scope: ComponentsMountSource, components: usize) -> LoadedLibrary {
    LoadedLibrary {
        name: name.to_owned(),
        path: PathBuf::from(format!("{name}.snxlib")),
        components,
        scope,
        vcs: None,
    }
}
