//! The Project tab shell: a vertical section list beside the selected
//! section. Pure: `&Signex -> Element`, no mutation.

use iced::widget::{Space, Stack, column, container, mouse_area, row};
use iced::{Element, Length, Widget};
use signex_i18n::ids;
use signex_widgets::dimension_label::DimensionLabelMessage;
use signex_widgets::{raw, text};

use super::preview_ui::{self, PreviewPopup, ProjectPreviewMsg, preview_message};
use super::section_list::section_list;
use super::{
    ProjectSection, ProjectTabMsg, RowDragMsg, erc_annotation, fonts, general, libraries,
    parameters, requirements, schematic_defaults, setup, sheets, variants, version_control,
};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::styles::TypeScale;

impl Signex {
    /// The project whose Project tab `window` shows. Only the main window
    /// shows one: a Project tab is not undocked (see the module docs).
    pub(crate) fn project_tab_in_window(&self, is_main: bool) -> Option<ProjectId> {
        let document = &self.document_state;
        if !is_main {
            return None;
        }
        document
            .tabs
            .get(document.active_tab)
            .and_then(|tab| tab.kind.as_project())
    }

    /// The Project tab for `project_id`, in the main window or an undocked
    /// one. A tab whose project is gone renders a short note instead.
    pub(crate) fn view_project_tab(&self, project_id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return container(
                text(raw("This project is no longer open."))
                    .size(TypeScale::default().title)
                    .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed();
        };
        let tab = self.document_state.project_tabs.get(&project_id);
        let selected = tab.map(|tab| tab.section).unwrap_or_default();
        let setup_draft = tab.and_then(|tab| tab.setup_draft.as_ref());
        let row_drag = tab.and_then(|tab| tab.row_drag.as_ref());
        let users = &self.ui_state.setup_profiles;
        let content = match selected {
            ProjectSection::General => general::view(&self.ui_state.i18n, project, tokens),
            ProjectSection::Parameters => parameters::view(project, tab, i18n, tokens),
            ProjectSection::Sheets => sheets::view(
                project,
                &self.project_sheet_rows(project),
                row_drag,
                setup_draft,
                users,
                &self.document_state.panel_ctx.canvas_colors,
                i18n,
                tokens,
            ),
            ProjectSection::Variants => variants::view(
                project,
                tab,
                self.document_state.sheet_edit_epoch,
                i18n,
                tokens,
            ),
            ProjectSection::Libraries => libraries::view(
                project_id,
                &self.project_library_rows(project),
                row_drag,
                i18n,
                tokens,
            ),
            ProjectSection::ErcAnnotation => {
                let facts = self.erc_section_facts(project);
                erc_annotation::view(project, &facts, i18n, tokens)
            }
            ProjectSection::VersionControl => {
                let facts = self.version_control_facts(project);
                let git = version_control::GitView {
                    state: &self.ui_state.project_git,
                    now: self.library.clock,
                };
                version_control::view(project, &facts, git, i18n, tokens)
            }
            ProjectSection::SchematicDefaults => schematic_defaults::view(
                project,
                tab.map(|tab| &tab.default_drafts),
                &self.ui_state.project_preview,
                &self.document_state.panel_ctx.canvas_colors,
                i18n,
                tokens,
            ),
            ProjectSection::Setup => setup::view(
                project,
                setup_draft,
                tab.and_then(|tab| tab.journal_lines_draft.as_deref()),
                users,
                i18n,
                tokens,
            ),
            ProjectSection::Requirements => {
                requirements::view(project, tab.map(|tab| &tab.requirements), i18n, tokens)
            }
            ProjectSection::Fonts => fonts::view(project, tab.map(|tab| &tab.fonts), i18n, tokens),
        };
        // The Variant Manager fills the tab; the other sections sit on top.
        let body = if selected == ProjectSection::Variants {
            content
        } else {
            column![content, Space::new().height(Length::Fill)].boxed()
        };
        let page = row![
            section_list(
                project,
                &self.section_facts(project),
                selected,
                i18n,
                tokens
            ),
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(crate::styles::panel_region(tokens)),
        ];
        let page: Element<'_, Message> = match read_only_banner(project, i18n, tokens) {
            Some(banner) => column![banner, page].boxed(),
            None => page.boxed(),
        };
        // A handle let go past the drawing still ends its drag.
        let ui = &self.ui_state.project_preview;
        let is_held = ui.labels_of(project_id).dragging_index().is_some();
        let released = is_held.then(|| {
            let msg = ProjectPreviewMsg::Labels(DimensionLabelMessage::PointerReleased);
            preview_message(project_id, msg)
        });
        // A row held by its grip drops wherever the button is let go.
        let row_released = row_drag.is_some().then(|| {
            general::tab_message(ProjectTabMsg::RowDragged {
                project_id,
                msg: RowDragMsg::Released,
            })
        });
        let released = row_released.or(released);
        let mut area = mouse_area(page);
        if let Some(released) = released {
            area = area.on_release(released);
        }
        let mut layers = Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(area.boxed());
        if let Some(popup) = self.preview_window(project_id, setup_draft) {
            layers = layers.push(popup);
        }
        layers.boxed()
    }

    /// The larger preview window open over `project_id`'s tab.
    fn preview_window<'a>(
        &'a self,
        project_id: ProjectId,
        setup_draft: Option<&crate::app::NewProjectState>,
    ) -> Option<Element<'a, Message>> {
        let project = self.document_state.project_by_id(project_id)?;
        let ui = &self.ui_state.project_preview;
        let tokens = &self.document_state.panel_ctx.tokens;
        let canvas = &self.document_state.panel_ctx.canvas_colors;
        let i18n = self.ui_state.i18n.as_ref();
        let (hint, drawing) = match ui.popup_of(project_id)? {
            PreviewPopup::Sheet => (
                ids::PROJECT_PREVIEW_HINT_SHEET,
                sheets::popup_drawing(
                    project,
                    setup_draft,
                    &self.ui_state.setup_profiles,
                    canvas,
                    i18n,
                    tokens,
                ),
            ),
            PreviewPopup::SchematicDefaults => (
                ids::PROJECT_PREVIEW_HINT_VALUES,
                schematic_defaults::popup_drawing(project, ui, canvas, i18n, tokens),
            ),
        };
        Some(preview_ui::window(project_id, hint, drawing, i18n, tokens))
    }
}

/// The warn banner over a project read from a newer `.snxprj` minor: its
/// edits cannot be saved in this build (#2904).
fn read_only_banner<'a>(
    project: &crate::app::state::LoadedProject,
    i18n: &signex_i18n::I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Option<Element<'a, Message>> {
    let format = project.data.unknown.newer_minor_source.as_deref()?;
    let line = crate::app::read_only_project::banner_line(i18n, format);
    let body = text(line).size(TypeScale::default().body).boxed();
    let callout = crate::styles::dialog_callout(crate::styles::CalloutTone::Warn, body, tokens);
    Some(container(callout).padding(BANNER_PADDING).boxed())
}

/// Space around the read-only banner, in logical pixels.
const BANNER_PADDING: f32 = 8.0;
