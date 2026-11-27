//! Start screen — what the centre region shows when no document is on
//! screen. Two distinct states (#331):
//!
//! * nothing loaded at all → [`view`]: a navigation column (New project,
//!   Open…, the Home / Projects / Libraries pages, Keyboard Shortcuts) and
//!   the page it selects;
//! * a project is loaded but no document is picked → [`hint_view`], a
//!   quieter pointer at the project tree with New sheet, New PCB and
//!   Project Options….
//!
//! Every action reuses an existing message: the actions the menu messages,
//! a recent entry File ▸ Open Recent's `OpenRecentProject` (a vanished
//! project is dropped and reported by the same handler), the hint's buttons
//! the command catalog, the Libraries page the library messages. Pages,
//! filters, pins and the row menu send [`StartScreenMsg`] (#2305).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use iced::Element;
use signex_i18n::I18n;
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::menu_bar::MenuMessage;
use crate::pinned_projects::PinnedProjects;
use crate::recent_projects::RecentProjects;

mod extension_slot;
mod graphite;
mod hint;
mod kit;
mod libraries_page;
mod nav_strip;
mod projects;
mod projects_page;

pub use hint::{HintFacts, hint_view};
pub use libraries_page::{LoadedLibrary, is_removable, library_folder, loaded_libraries};
pub use projects::opened_label;
pub use projects_page::{ProjectListing, project_listing};

/// How many recent projects Home lists; the Projects page lists them all
/// (File ▸ Open Recent keeps more).
pub const MAX_START_RECENT: usize = 5;

/// Which centre-region placeholder applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Nothing loaded: show the start screen.
    Start,
    /// Project loaded, no document selected: show the quiet hint.
    ProjectHint,
}

impl Surface {
    pub fn select(has_active_project: bool) -> Self {
        if has_active_project {
            Self::ProjectHint
        } else {
            Self::Start
        }
    }
}

/// One clickable entry on the start screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewProject,
    OpenProject,
    /// The Home page's reference-design card (and Help ▸ Open Sample
    /// Project); the navigation has no row for it (decided 2026-11-19).
    OpenSampleProject,
    KeyboardShortcuts,
}

impl Action {
    pub const ALL: [Action; 4] = [
        Action::NewProject,
        Action::OpenProject,
        Action::OpenSampleProject,
        Action::KeyboardShortcuts,
    ];

    /// The existing, working message this action dispatches.
    pub fn message(self) -> Message {
        Message::Menu(match self {
            Action::NewProject => MenuMessage::NewProject,
            Action::OpenProject => MenuMessage::OpenProject,
            Action::OpenSampleProject => MenuMessage::OpenSampleProject,
            Action::KeyboardShortcuts => MenuMessage::OpenKeyboardShortcuts,
        })
    }
}

/// The page the main column shows; the navigation row of the current one
/// is lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartPage {
    #[default]
    Home,
    /// Every recent and pinned project, filtered.
    Projects,
    /// The loaded libraries.
    Libraries,
}

/// The Projects page switch: which projects it lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectScope {
    #[default]
    All,
    Pinned,
    /// Pinned projects whose file is not on disk.
    Missing,
}

/// The Start screen's own state: the page and the filters of the two list
/// pages. Opening a project or a tab sends it back to Home, unfiltered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartScreenState {
    pub page: StartPage,
    pub project_filter: String,
    pub project_scope: ProjectScope,
    pub library_filter: String,
    /// The bundled sample project's file, whose thumbnail the reference
    /// card shows; read once at boot, `None` without a config directory.
    pub sample_project: Option<PathBuf>,
    /// The time the last-opened column is read against. `update` stamps it
    /// when the screen shows and on [`StartScreenMsg::ClockTicked`]; it is
    /// `None` while the screen is off, so `view` never reads the clock.
    pub now: Option<DateTime<Utc>>,
    /// The version-control state of each library folder, read off the UI
    /// thread when the Libraries page opens and after Enable Version
    /// Control finishes (#2591). A folder not read yet has no entry.
    pub library_vcs: BTreeMap<PathBuf, signex_library::library_vcs::LibraryVcsState>,
    /// Bumped by every read of [`Self::library_vcs`]; an older read's
    /// answer is dropped.
    pub library_vcs_generation: u64,
    /// The library file whose Remove from Signex (#2592) waits for its
    /// Close Library to unmount it; cleared when that close is cancelled.
    pub removing_library: Option<PathBuf>,
}

impl StartScreenState {
    /// The state a launch starts in: Home, with the sample project's path.
    pub fn at_boot(now: DateTime<Utc>) -> Self {
        Self {
            sample_project: crate::sample_project::sample_root()
                .map(|root| crate::sample_project::project_path(&root)),
            now: Some(now),
            ..Self::default()
        }
    }

    /// Whether the screen is on Home with no filter or scope set.
    pub fn is_home(&self) -> bool {
        self.page == StartPage::Home
            && self.project_filter.is_empty()
            && self.project_scope == ProjectScope::All
            && self.library_filter.is_empty()
    }

    /// Back to Home with every filter cleared; the sample path stays.
    pub fn go_home(&mut self) {
        self.page = StartPage::Home;
        self.project_filter.clear();
        self.project_scope = ProjectScope::All;
        self.library_filter.clear();
    }
}

/// The recent projects the start screen shows: newest first, capped.
pub fn visible_recent(recent: &[PathBuf]) -> &[PathBuf] {
    &recent[..recent.len().min(MAX_START_RECENT)]
}

/// The two project lists Home shows (#2305).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSections<'a> {
    /// Pinned Projects, in the user's order; the section is hidden when
    /// this is empty.
    pub pinned: &'a [PathBuf],
    /// Recent Projects without the pinned ones, newest first, capped.
    pub recent: Vec<&'a PathBuf>,
    /// How many recent projects are not pinned, the capped-off ones too.
    pub recent_total: usize,
    /// Whether Recent Projects shows: hidden when every recent project is
    /// pinned, shown (with its empty card) when there are none at all.
    pub show_recent: bool,
}

impl ProjectSections<'_> {
    /// Whether Recent Projects lists fewer than there are, so its header
    /// links to the Projects page.
    pub fn has_more_recent(&self) -> bool {
        self.recent_total > self.recent.len()
    }
}

/// Split `recent` and `pinned` into Home's sections: a pinned project is
/// listed under Pinned Projects only.
pub fn project_sections<'a>(
    recent: &'a [PathBuf],
    pinned: &'a PinnedProjects,
) -> ProjectSections<'a> {
    let unpinned: Vec<&PathBuf> = recent
        .iter()
        .filter(|path| !pinned.contains(path))
        .collect();
    let recent_total = unpinned.len();
    ProjectSections {
        pinned: pinned.entries(),
        show_recent: recent.is_empty() || !unpinned.is_empty(),
        recent: unpinned.into_iter().take(MAX_START_RECENT).collect(),
        recent_total,
    }
}

/// The existing File ▸ Open Recent message for one entry.
pub fn recent_message(path: &Path) -> Message {
    Message::Menu(MenuMessage::OpenRecentProject(path.to_path_buf()))
}

/// Wrap a Start screen event in its namespace.
pub fn message(msg: StartScreenMsg) -> Message {
    Message::Project(crate::app::ProjectMsg::StartScreen(msg))
}

/// Start screen events (#2305). Routed to
/// `Signex::dispatch_start_screen_message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartScreenMsg {
    /// Pin was pressed on a Recent Projects row (hover button or menu).
    ProjectPinned(PathBuf),
    /// Open was picked in a Recent Projects row's menu: the menu closes and
    /// the project opens as File ▸ Open Recent opens it.
    RecentProjectClicked(PathBuf),
    /// Unpin, or Remove on a pinned project missing on disk, was pressed.
    ProjectUnpinned(PathBuf),
    /// A Pinned Projects row (or its menu's Open) was clicked.
    PinnedProjectClicked(PathBuf),
    /// A project row was right-clicked: open its menu at the pointer.
    RowRightClicked(PathBuf),
    /// The row menu was dismissed without a pick.
    RowMenuDismissed,
    /// The start-up check of the pins finished off the UI thread: the pins
    /// whose file is not on disk, or why the check could not run.
    PinsChecked(Result<Vec<PathBuf>, String>),
    /// A navigation row, the Libraries card or "Show all" picked a page.
    PageSelected(StartPage),
    /// The Projects page filter text changed.
    ProjectFilterChanged(String),
    /// The Projects page switch (All / Pinned / Missing) was picked.
    ProjectScopePicked(ProjectScope),
    /// The Libraries page filter text changed.
    LibraryFilterChanged(String),
    /// Clear filter was pressed on the current page's empty state.
    FilterCleared,
    /// The minute tick fired while the screen shows; carries the time the
    /// last-opened column is read against.
    ClockTicked(DateTime<Utc>),
    /// The off-thread read of the libraries' version control finished
    /// (#2591): each library folder with its state, or why the read failed.
    LibraryVcsRead {
        generation: u64,
        result: Result<Vec<(PathBuf, signex_library::library_vcs::LibraryVcsState)>, String>,
    },
    /// "off" was pressed in a library row's Version control cell: open
    /// Enable Version Control on that library file.
    LibraryVcsEnableClicked(PathBuf),
    /// A library row's ⋯ button was pressed: open its menu at the pointer.
    LibraryMenuOpened(PathBuf),
    /// Remove from Signex was picked in a library row's menu: ask first.
    LibraryRemoveRequested(PathBuf),
    /// The Remove from Signex confirm was accepted for this library file.
    LibraryRemoveConfirmed(PathBuf),
    /// A removed library's entry left `global_libraries.toml` on a worker:
    /// the list now saved, or why the write failed.
    GlobalLibraryUnregistered(
        Result<Vec<crate::panels::components_panel::global_prefs::GlobalLibraryEntry>, String>,
    ),
}

/// How often the last-opened column ("today 14:05", "yesterday") is
/// re-read against the clock while the Start screen shows.
pub const CLOCK_TICK: std::time::Duration = std::time::Duration::from_secs(60);

/// The minute tick behind [`StartScreenMsg::ClockTicked`]; none while the
/// screen is off, so an open document costs nothing.
pub fn clock_subscription(shown: bool) -> iced::Subscription<Message> {
    if shown {
        iced::time::every(CLOCK_TICK).map(|_| message(StartScreenMsg::ClockTicked(Utc::now())))
    } else {
        iced::Subscription::none()
    }
}

/// The open menu of a Start screen row: a project row's right-click menu,
/// or a library row's ⋯ menu (#2592).
#[derive(Debug, Clone, PartialEq)]
pub struct StartRowMenu {
    pub path: PathBuf,
    pub kind: StartRowKind,
    pub x: f32,
    pub y: f32,
}

/// Which list a [`StartRowMenu`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartRowKind {
    Project,
    Library,
}

fn recent_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// What the Start screen draws, read from the model by the caller.
pub struct StartScreenData<'a> {
    /// Label of the key bound to Open… (empty when unbound).
    pub open_shortcut: String,
    /// Label of the key bound to the shortcut list (empty when unbound).
    pub shortcuts_shortcut: String,
    pub recent: &'a RecentProjects,
    pub pinned: &'a PinnedProjects,
    pub libraries: Vec<LoadedLibrary>,
    /// The cached project renders the row tiles show.
    pub thumbnails: &'a crate::project_thumbnail::ProjectThumbnails,
    pub state: &'a StartScreenState,
    /// The clock the last-opened times are read against
    /// ([`StartScreenState::now`], stamped by `update`).
    pub now: DateTime<Utc>,
    /// The installed extensions, whose sections fill the Home page's
    /// extension slot (#2495). None in the Community binary.
    pub extensions: &'a crate::extension::ExtensionHost,
}

pub fn view<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    data: StartScreenData<'a>,
) -> Element<'a, Message> {
    graphite::view(tokens, i18n, data)
}

/// Where the middle of the collapsed navigation strip is, given the left
/// edge of the laid-out hero title (#2346).
#[cfg(test)]
pub(crate) fn strip_centre_from_hero(hero_left: f32) -> f32 {
    hero_left - graphite::MAIN_PAD_X - nav_strip::STRIP_W / 2.0
}

#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod page_tests;
#[cfg(test)]
mod tests;
