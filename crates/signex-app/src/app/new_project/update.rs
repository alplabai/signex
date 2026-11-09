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
        // The name, location or folder choice changed: a reason given for
        // the old target no longer applies.
        state.create_error = None;
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
            M::ImportRequested => {
                import_rule_file(&self.ui_state.io.picker, &self.ui_state.i18n, Message::from)
            }
            M::TargetChecked { generation, status } => {
                self.with_new_project(|s| {
                    if s.generation == generation {
                        s.target = status;
                    }
                });
                Task::none()
            }
            M::SubmitRequested => self.create_project_from_dialog(),
            // While Create writes, Cancel and Esc wait: the project would be
            // created and opened anyway.
            M::CancelRequested
                if self
                    .ui_state
                    .new_project
                    .as_ref()
                    .is_some_and(|s| s.creating.is_some()) =>
            {
                Task::none()
            }
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
        let title = signex_i18n::t!(
            self.ui_state.i18n,
            signex_i18n::ids::DIALOG_NEW_PROJECT_PICK_LOCATION_TITLE
        )
        .into_string();
        Task::perform(
            async move {
                let mut dialog = picker.dialog().set_title(title);
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
        // A Create that would not start says why in the dialog instead of
        // leaving it waiting.
        let refusal = if self.document_state.pending_opens.contains(&chosen) {
            Some(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::DIALOG_NEW_PROJECT_ERR_CREATE_PENDING
                )
                .into_string(),
            )
        } else {
            crate::app::handlers::document_files::new_project::new_project_parts(&chosen)
                .err()
                .map(|e| format!("{e:#}"))
        };
        if let Some(why) = refusal {
            if let Some(state) = self.ui_state.new_project.as_mut() {
                state.create_error = Some(why);
            }
            return Task::none();
        }
        // The dialog stays open, its Create off, until the writes land: a
        // failure keeps the setup and shows why (`finish_dialog_create`).
        if let Some(state) = self.ui_state.new_project.as_mut() {
            state.creating = Some(chosen.clone());
            state.create_error = None;
        }
        self.start_new_project(chosen, plan)
    }

    /// The writes of the dialog's Create landed. `true` when they failed and
    /// were the dialog's: it stays open with the reason, and the caller
    /// raises no error card. On success the dialog closes.
    pub(crate) fn finish_dialog_create(
        &mut self,
        path: &std::path::Path,
        result: &Result<PathBuf, String>,
    ) -> bool {
        let Some(state) = self.ui_state.new_project.as_mut() else {
            return false;
        };
        if state.creating.as_deref() != Some(path) {
            return false;
        }
        state.creating = None;
        match result {
            Ok(_) => {
                self.ui_state.new_project = None;
                false
            }
            Err(why) => {
                state.create_error = Some(why.clone());
                crate::diagnostics::log_error(
                    "Failed to create new project",
                    &anyhow::anyhow!(why.clone()),
                );
                true
            }
        }
    }
}

/// Read a rule file (at most the size cap) and parse it.
fn read_rule_file(
    path: &std::path::Path,
) -> Result<signex_types::project::RuleTemplate, super::RuleReadError> {
    use std::io::Read as _;
    let cap = signex_types::project::MAX_RULE_FILE_BYTES as u64 + 1;
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(cap).read_to_string(&mut text))
        .map_err(|e| super::RuleReadError::Read {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
    signex_types::project::RuleTemplate::parse(&text).map_err(super::RuleReadError::File)
}

/// Import rules: pick a rule file and read it off the update thread. `wrap`
/// routes the result to the surface that asked (the dialog or the Project
/// tab).
pub(crate) fn import_rule_file(
    picker: &crate::io_ports::Picker,
    i18n: &signex_i18n::I18n,
    wrap: impl Fn(NewProjectMsg) -> Message + Send + 'static,
) -> Task<Message> {
    use signex_i18n::{ids, t};
    let picker = picker.clone();
    let title = t!(i18n, ids::DIALOG_NEW_PROJECT_PICK_RULES_TITLE).into_string();
    let filter = t!(i18n, ids::DIALOG_NEW_PROJECT_PICK_RULES_FILTER).into_string();
    Task::perform(
        async move {
            let path = picker
                .dialog()
                .set_title(title)
                .add_filter(filter, &["toml"])
                .pick_file()
                .await?;
            let shown = path.display().to_string();
            let read = on_blocking_pool(move || Ok(read_rule_file(&path)))
                .await
                .unwrap_or_else(|detail| {
                    Err(super::RuleReadError::Read {
                        path: shown,
                        detail,
                    })
                });
            Some(read)
        },
        move |read| wrap(NewProjectMsg::RulesFileRead(read)),
    )
}
