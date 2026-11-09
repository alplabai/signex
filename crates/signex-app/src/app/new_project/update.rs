//! The New Project dialog's update: opening it, its edits, and Create.

use std::path::PathBuf;

use iced::Task;
use iced::widget::operation;

use super::state::NewProjectState;
use super::target::{TargetStatus, check_target};
use super::{NewProjectMsg, check_name};
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::handlers::document_files::new_project::{NewProjectLayout, NewProjectPlan};
use crate::app::{Message, Signex};

/// Widget id of the name field, focused when the dialog opens.
pub(crate) static NAME_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.new_project.name"));

/// Name a fresh dialog offers.
const SUGGESTED_NAME: &str = "Untitled";

impl Signex {
    /// Where a new project goes by default: beside the open project's folder,
    /// else `Signex` in the user's documents folder.
    fn default_project_location(&self) -> String {
        let beside_open = self
            .document_state
            .active_project
            .and_then(|id| self.document_state.project_by_id(id))
            .and_then(|p| p.path.parent())
            .and_then(std::path::Path::parent)
            .map(PathBuf::from);
        let base = beside_open.or_else(|| {
            dirs::document_dir()
                .or_else(dirs::home_dir)
                .map(|d| d.join("Signex"))
        });
        base.map(|p| p.display().to_string()).unwrap_or_default()
    }

    /// File > New Project: open the setup dialog. Writes nothing.
    pub(crate) fn open_new_project_dialog(&mut self) -> Task<Message> {
        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;
        if self.ui_state.new_project.is_some() {
            return operation::focus(NAME_INPUT_ID.clone());
        }
        let mut state = NewProjectState::new(self.default_project_location());
        state.name = SUGGESTED_NAME.to_string();
        self.ui_state.new_project = Some(state);
        Task::batch([
            operation::focus(NAME_INPUT_ID.clone()),
            operation::text_input::select_all(NAME_INPUT_ID.clone()),
            self.recheck_new_project_target(),
        ])
    }

    /// Ask the blocking pool whether the described project fits on disk.
    fn recheck_new_project_target(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.new_project.as_mut() else {
            return Task::none();
        };
        state.generation += 1;
        state.target = TargetStatus::Unchecked;
        if !state.wants_target_check() {
            return Task::none();
        }
        let generation = state.generation;
        let (name, location, make_folder) = (
            state.name.clone(),
            state.location.clone(),
            state.make_folder,
        );
        Task::perform(
            on_blocking_pool(move || Ok(check_target(&name, &location, make_folder))),
            move |result| {
                let status = result
                    .unwrap_or_else(|why| TargetStatus::Problem(super::TargetProblem::Other(why)));
                Message::from(NewProjectMsg::TargetChecked { generation, status })
            },
        )
    }

    pub(crate) fn dispatch_new_project_message(&mut self, msg: NewProjectMsg) -> Task<Message> {
        use NewProjectMsg as M;
        if self.ui_state.new_project.is_none() && !matches!(msg, M::CancelRequested) {
            return Task::none();
        }
        match msg {
            M::NameEdited(name) => {
                self.with_new_project(|s| s.name = name);
                self.recheck_new_project_target()
            }
            M::LocationEdited(location) => {
                self.with_new_project(|s| s.location = location);
                self.recheck_new_project_target()
            }
            M::LocationPicked(Some(path)) => {
                self.with_new_project(|s| s.location = path.display().to_string());
                self.recheck_new_project_target()
            }
            M::LocationPicked(None) => Task::none(),
            M::BrowseRequested => self.browse_new_project_location(),
            M::MakeFolderToggled(on) => {
                self.with_new_project(|s| s.make_folder = on);
                self.recheck_new_project_target()
            }
            M::ImportRequested => import_rule_file(&self.ui_state.io.picker, Message::from),
            M::TargetChecked { generation, status } => {
                self.with_new_project(|s| {
                    if s.generation == generation {
                        s.target = status;
                    }
                });
                Task::none()
            }
            M::SubmitRequested => self.create_project_from_dialog(),
            M::CancelRequested => {
                self.ui_state.new_project = None;
                Task::none()
            }
            setup => {
                self.with_new_project(|s| {
                    s.apply_setup(setup);
                });
                Task::none()
            }
        }
    }

    fn with_new_project(&mut self, edit: impl FnOnce(&mut NewProjectState)) {
        if let Some(state) = self.ui_state.new_project.as_mut() {
            edit(state);
        }
    }

    fn browse_new_project_location(&mut self) -> Task<Message> {
        let picker = self.ui_state.io.picker.clone();
        let start = self
            .ui_state
            .new_project
            .as_ref()
            .and_then(NewProjectState::location_path);
        Task::perform(
            async move {
                let mut dialog = picker.dialog().set_title("New Project location");
                if let Some(dir) = start.filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                dialog.pick_folder().await
            },
            |picked| Message::from(NewProjectMsg::LocationPicked(picked)),
        )
    }

    /// Create: close the dialog and write the project off-thread. Ignored
    /// while Create is unavailable (an unchecked target, a bad value).
    fn create_project_from_dialog(&mut self) -> Task<Message> {
        let Some(state) = self.ui_state.new_project.as_ref() else {
            return Task::none();
        };
        if !state.can_create() {
            return Task::none();
        }
        let (Some(location), Ok(name)) = (state.location_path(), check_name(&state.name)) else {
            return Task::none();
        };
        let layout = if state.make_folder {
            NewProjectLayout::NamedFolder
        } else {
            NewProjectLayout::InPlace
        };
        let plan = NewProjectPlan {
            layout,
            setup: Some(state.setup.clone()),
        };
        let chosen = location.join(format!("{name}.snxprj"));
        self.ui_state.new_project = None;
        self.start_new_project(chosen, plan)
    }
}

/// Read a rule file (at most the size cap) and parse it; the error is
/// worded for the dialog.
fn read_rule_file(path: &std::path::Path) -> Result<signex_types::project::RuleTemplate, String> {
    use std::io::Read as _;
    let cap = signex_types::project::MAX_RULE_FILE_BYTES as u64 + 1;
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(cap).read_to_string(&mut text))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    signex_types::project::RuleTemplate::parse(&text).map_err(|e| e.to_string())
}

/// Import rules: pick a rule file and read it off the update thread. `wrap`
/// routes the result to the surface that asked (the dialog or the Project
/// tab).
pub(crate) fn import_rule_file(
    picker: &crate::io_ports::Picker,
    wrap: impl Fn(NewProjectMsg) -> Message + Send + 'static,
) -> Task<Message> {
    let picker = picker.clone();
    Task::perform(
        async move {
            let path = picker
                .dialog()
                .set_title("Import rules")
                .add_filter("Rule file", &["toml"])
                .pick_file()
                .await?;
            Some(on_blocking_pool(move || read_rule_file(&path)).await)
        },
        move |read| wrap(NewProjectMsg::RulesFileRead(read)),
    )
}
