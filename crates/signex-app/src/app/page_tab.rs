//! Page tabs: document tabs that show one of a project's pages rather than
//! a drawing (Project, Rules and Layer Stack today; BOM Studio, Export Job
//! and the other #2523 kinds join here). Two rules hold for every one:
//!
//! - the title is `<project name> — <page name>` ("sensor-node — Layer
//!   Stack"), read in the current language;
//! - the tab is dirty exactly when the file it saves into, its owner, has
//!   unsaved work (#661). Project and Rules save into the `.snxprj` on
//!   `TabInfo.path`. A page tab carries its owner's path as `TabInfo.path`
//!   and never a path of its own, so the dirty check is that path's
//!   (`tab_bar::tab_is_dirty`).
//!
//! Each page kind draws its own body. See docs/PROJECT_TAB.md, "Page tabs".

use signex_i18n::{Arg, I18n, MessageId, ids, t};

use crate::app::state::ProjectId;
use crate::app::{LoadedProject, Signex, TabKind};

/// The page name of a page tab; `None` for a drawing, an editor or a
/// library tab. A new page kind adds its arm here.
pub(crate) fn page_name(kind: &TabKind) -> Option<MessageId> {
    match kind {
        TabKind::PolygonManager => Some(ids::POLYGON_MANAGER_PAGE),
        TabKind::Project(_) => Some(ids::CHROME_PAGE_TAB_PROJECT),
        TabKind::Rules(_) => Some(ids::RULES_TAB_TITLE),
        TabKind::LayerStack => Some(ids::PCB_STACK_TITLE),
        TabKind::PinTable => Some(ids::EDITORS_PIN_TABLE_PAGE),
        TabKind::VariantManager(_) => Some(ids::VARIANT_MANAGER_TAB_TITLE),
        TabKind::DesignChecklist(_) => Some(ids::CHECKLIST_TAB_TITLE),
        TabKind::Schematic
        | TabKind::Pcb
        | TabKind::ComponentEditor(_)
        | TabKind::SymbolEditor(_)
        | TabKind::FootprintEditor(_)
        | TabKind::LibraryBrowser(_) => None,
        // Titled by its package, not a project (`app/gerber_viewer/tab`);
        // an Export Job by its file (`app/output_job/document`).
        TabKind::GerberViewer | TabKind::OutputJob => None,
    }
}

/// The project a page tab shows a page of. A Layer Stack tab shows a page of
/// its board, which may have no project: `Signex::board_page_title` names it.
pub(crate) fn page_project(kind: &TabKind) -> Option<ProjectId> {
    page_name(kind)?;
    kind.as_project()
        .or_else(|| kind.as_rules())
        .or_else(|| kind.as_variant_manager())
        .or_else(|| kind.as_design_checklist())
}

/// `<project> — <page>` in the language of `i18n`, without bidi marks so
/// the strip can measure and cut it.
pub(crate) fn page_title(i18n: &I18n, project_name: &str, page: MessageId) -> String {
    let page = t!(i18n, page).into_string();
    i18n.message_plain(
        ids::CHROME_PAGE_TAB_TITLE,
        &[
            ("project", Arg::from(project_name)),
            ("page", Arg::from(page)),
        ],
    )
}

/// The name a page tab shows for `project`: its own name, or the file stem
/// of its `.snxprj` when the name is blank.
pub(crate) fn project_display_name(project: &LoadedProject) -> String {
    let name = project.data.name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    project
        .path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The title of a page tab of `kind` among `projects`; `None` for any other
/// tab, or a page tab whose project has closed.
pub(crate) fn tab_title(kind: &TabKind, projects: &[LoadedProject], i18n: &I18n) -> Option<String> {
    let page = page_name(kind)?;
    let id = page_project(kind)?;
    let project = projects.iter().find(|p| p.id == id)?;
    Some(page_title(i18n, &project_display_name(project), page))
}

impl Signex {
    /// The title a page tab of `kind` gets now; `None` for any other tab.
    pub(crate) fn page_tab_title(&self, kind: &TabKind) -> Option<String> {
        tab_title(kind, &self.document_state.projects, &self.ui_state.i18n)
    }

    /// Mint every page tab's title again: after a project is renamed, so
    /// the stored titles (the tab's context menu, a drag ghost) follow.
    pub(crate) fn refresh_page_tab_titles(&mut self) {
        let titles: Vec<(usize, String)> = self
            .document_state
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, tab)| {
                self.page_tab_title(&tab.kind)
                    .or_else(|| {
                        let page = page_name(&tab.kind).filter(|_| tab.kind.is_board_page())?;
                        Some(self.board_page_title(&tab.path, tab.project_id, page))
                    })
                    .or_else(|| {
                        tab.kind
                            .is_pin_table()
                            .then(|| self.pin_table_title(&tab.path))
                    })
                    .map(|title| (i, title))
            })
            .collect();
        for (i, title) in titles {
            self.document_state.tabs[i].title = title;
        }
    }
}

#[cfg(test)]
#[path = "page_tab_tests.rs"]
mod tests;
