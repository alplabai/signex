//! The Variant Manager as a page tab of its project (mockup row 32): one tab
//! per project, `<project> — Variants`, saving into the `.snxprj`. Opening
//! it again focuses it. Its parts are read while it is open.

use iced::widget::container;
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use crate::app::state::ProjectId;
use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::styles::TypeScale;

impl Signex {
    /// The project's Variant Manager tab is open (shown or not).
    pub(crate) fn variant_manager_is_open(&self, id: ProjectId) -> bool {
        self.document_state
            .tabs
            .iter()
            .any(|tab| tab.kind.as_variant_manager() == Some(id))
    }

    /// Focus `id`'s Variant Manager tab, or open it, and read its parts: a
    /// new tab always reads them, a focused one when the last read no
    /// longer holds.
    pub(crate) fn open_variant_manager_tab(&mut self, id: ProjectId) -> iced::Task<Message> {
        let Some(project) = self.document_state.project_by_id(id) else {
            return iced::Task::none();
        };
        let path = project.path.clone();
        let existing = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.kind.as_variant_manager() == Some(id));
        let idx = match existing {
            Some(idx) => idx,
            None => {
                let kind = TabKind::VariantManager(id);
                // A page tab: "<project> — Variants" (docs/PROJECT_TAB.md,
                // "Page tabs").
                let title = self.page_tab_title(&kind).unwrap_or_default();
                self.document_state.project_tabs.entry(id).or_default();
                self.document_state.tabs.push(TabInfo {
                    title,
                    path,
                    project_id: Some(id),
                    kind,
                });
                self.document_state.tabs.len() - 1
            }
        };
        if idx != self.document_state.active_tab || existing.is_none() {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
        self.read_variant_parts(id, existing.is_none())
    }

    /// The project whose Variant Manager tab the main window shows. Like the
    /// Project tab, it is not undocked.
    pub(crate) fn variant_manager_tab_in_window(&self, is_main: bool) -> Option<ProjectId> {
        if !is_main {
            return None;
        }
        let docs = &self.document_state;
        docs.tabs
            .get(docs.active_tab)
            .and_then(|tab| tab.kind.as_variant_manager())
    }

    /// The Variant Manager tab of `project_id`. A tab whose project is gone
    /// renders a short note instead.
    pub(crate) fn view_variant_manager_tab(&self, project_id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return container(
                text(t!(self.ui_state.i18n, ids::RULES_NO_PROJECT))
                    .size(TypeScale::default().title)
                    .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed();
        };
        let tab = self.document_state.project_tabs.get(&project_id);
        let body = super::shell(
            project,
            tab,
            self.document_state.sheet_edit_epoch,
            self.ui_state.i18n.as_ref(),
            tokens,
        );
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed()
    }
}
