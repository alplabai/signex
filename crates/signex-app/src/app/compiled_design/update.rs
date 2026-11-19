//! The dispatcher of `CompiledDesignMsg`, and starting and landing a
//! compile.

use std::sync::Arc;

use iced::Task;

use super::job::CompileJob;
use super::state::CompiledResult;
use super::{CompileOutcome, CompiledDesignMsg};
use crate::app::project_sheets;
use crate::app::runtime::net_class_styles::plan_projects;
use crate::app::state::ProjectId;
use crate::app::state::project_case::fold_of;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_compiled_design_message(
        &mut self,
        msg: CompiledDesignMsg,
    ) -> Task<Message> {
        use CompiledDesignMsg as M;
        let task = match msg {
            M::CompileRequested => self.start_compile(),
            M::Compiled(outcome) => {
                self.land_compile(&outcome);
                Task::none()
            }
            M::ModePicked(mode) => {
                self.ui_state.compiled_design.mode = mode;
                Task::none()
            }
            M::FindChanged(text) => {
                self.ui_state.compiled_design.find = text;
                Task::none()
            }
            M::RowToggled(row) => {
                let folded = &mut self.ui_state.compiled_design.folded;
                if !folded.remove(&row) {
                    folded.insert(row);
                }
                Task::none()
            }
            M::RowClicked(row) => self.open_compiled_row(row),
            M::ChannelPicked { sheet, channel } => {
                self.pick_channel(sheet, channel);
                Task::none()
            }
            M::RepeatToggled { sheet_symbol, on } => {
                self.toggle_repeat(sheet_symbol, on);
                Task::none()
            }
            M::RepeatFieldTyped {
                sheet_symbol,
                field,
                text,
            } => {
                self.type_repeat_field(sheet_symbol, field, text);
                Task::none()
            }
            M::RepeatFieldSubmitted { sheet_symbol } => {
                self.submit_repeat_fields(sheet_symbol);
                Task::none()
            }
            M::NamingMenuToggled(target) => {
                self.toggle_naming_menu(target);
                Task::none()
            }
            M::NamingMenuClosed => {
                self.ui_state.compiled_design.naming_menu = None;
                Task::none()
            }
            M::NamingPicked { target, format } => {
                self.pick_naming(target, format);
                Task::none()
            }
            M::NamingCustomTyped(text) => {
                if let Some(menu) = self.ui_state.compiled_design.naming_menu.as_mut() {
                    menu.custom = text;
                }
                Task::none()
            }
            M::NamingCustomSubmitted => {
                self.submit_custom_naming();
                Task::none()
            }
            M::IllustrationHovered(hover) => {
                self.ui_state
                    .compiled_design
                    .illustration_hover
                    .update(hover);
                Task::none()
            }
        };
        self.sync_compiled_design_view();
        Task::batch([task, self.finish_update()])
    }

    /// The project a compile acts on: the active document's, else the
    /// active project.
    pub(crate) fn compile_target(&self) -> Option<ProjectId> {
        let document = &self.document_state;
        document
            .active_path
            .as_deref()
            .and_then(|path| document.document_project(path))
            .map(|project| project.id)
            .or(document.active_project)
    }

    /// Start a compile of the target project on a worker. A compile already
    /// running is superseded: its answer is dropped when it lands.
    fn start_compile(&mut self) -> Task<Message> {
        let Some(job) = self.plan_compile() else {
            crate::diagnostics::log_warning(
                "Compile project: open a sheet of a project first; a sheet outside a project \
                 has no compiled design.",
            );
            return Task::none();
        };
        let (project, generation) = (job.project, job.generation);
        self.document_state.compiled_design.pending = Some((project, generation));
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || job.run()),
            move |result| {
                let outcome = result.unwrap_or_else(|reason| CompileOutcome {
                    project,
                    generation,
                    result: Err(reason),
                    warnings: Vec::new(),
                });
                Message::from(CompiledDesignMsg::Compiled(Arc::new(outcome)))
            },
        )
    }

    /// The compile of the target project over a snapshot of its open sheets,
    /// planned the way the net index job plans its stitch.
    fn plan_compile(&mut self) -> Option<CompileJob> {
        let project_id = self.compile_target()?;
        let project = self.document_state.project_by_id(project_id)?;
        let naming = project.data.schematic_design.channel_naming.clone();
        let project_dir = project.dir().to_path_buf();
        let planned = plan_projects(&self.document_state)
            .into_iter()
            .find(|p| p.project == project_id);
        let (open, pages, root, fold) = match planned {
            Some(planned) => (
                project_sheets::open_sheets_snapshot_of(&self.document_state, &planned.open_paths),
                planned.key.pages,
                planned.key.root,
                planned.key.fold,
            ),
            None => (
                Default::default(),
                project_sheets::project_pages(Some(project)),
                project_sheets::project_root_sheet_path(project)?,
                fold_of(Some(project)),
            ),
        };
        let store = &mut self.document_state.compiled_design;
        store.generation += 1;
        Some(CompileJob {
            project: project_id,
            generation: store.generation,
            open,
            pages,
            root,
            fold,
            project_dir,
            naming,
        })
    }

    /// Keep a finished compile. An answer for a closed project or a
    /// superseded request is dropped; a failure is said in Messages.
    fn land_compile(&mut self, outcome: &CompileOutcome) {
        let store = &mut self.document_state.compiled_design;
        if store.pending != Some((outcome.project, outcome.generation)) {
            return;
        }
        store.pending = None;
        if self.document_state.project_by_id(outcome.project).is_none() {
            return;
        }
        for warning in &outcome.warnings {
            crate::diagnostics::log_warning(warning);
        }
        let compiled = match &outcome.result {
            Ok(compiled) => compiled,
            Err(reason) => {
                let i18n = signex_i18n::I18n::en_us_ref();
                let line = signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::PANELS_COMPILED_DESIGN_FAILED,
                    "reason" => reason.as_str()
                );
                crate::diagnostics::log_warning(line.into_string());
                return;
            }
        };
        // The clock is read here, in update; the view shows the stored text.
        let compiled_at = chrono::Local::now().format("%H:%M").to_string();
        let loaded: Vec<ProjectId> = self.document_state.projects.iter().map(|p| p.id).collect();
        let store = &mut self.document_state.compiled_design;
        store.results.retain(|id, _| loaded.contains(id));
        store.results.insert(
            outcome.project,
            CompiledResult {
                design: Arc::new(compiled.design.clone()),
                paths: compiled.paths.clone(),
                compiled_at,
                stale: Default::default(),
            },
        );
        let ui = &mut self.ui_state.compiled_design;
        ui.folded.clear();
        ui.selected = None;
        self.refresh_bound_tables(Some(outcome.project));
    }
}
