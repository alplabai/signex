//! The Differential Pair Classes dialog's events. Every edit changes the
//! dialog's copy only; OK applies it to the project as one General-history
//! step, Cancel and Esc drop it.

use iced::Task;
use signex_engine::ProjectGeneralCommand;

use super::{
    DiffPairClassesMsg, DiffPairClassesState, PairClassField, PairMemberRow, free_name, new_class,
    set_field,
};
use crate::app::handlers::canvas::parse_value;
use crate::app::{Message, Signex};

/// The id of the class Name field, focused by Rename.
pub(crate) const NAME_INPUT_ID: &str = "diff-pair-class-name";

impl Signex {
    pub(crate) fn dispatch_diff_pair_classes_message(
        &mut self,
        msg: DiffPairClassesMsg,
    ) -> Task<Message> {
        use DiffPairClassesMsg as M;
        match msg {
            M::Requested => self.open_diff_pair_classes(),
            M::ClassSelected(index) => self.with_dpc(|s| select(s, index)),
            M::ClassMenuToggled(menu) => self.with_dpc(|s| s.menu = menu),
            M::ClassRenameStarted(index) => {
                self.with_dpc(|s| select(s, index));
                return iced::widget::operation::focus(iced::widget::Id::new(NAME_INPUT_ID));
            }
            M::ClassDuplicated(index) => self.with_dpc(|s| {
                if let Some(class) = s.classes.get(index).cloned() {
                    let mut copy = class;
                    copy.name = free_name(&s.classes, &copy.name);
                    s.insert_new(index + 1, copy);
                    select(s, index + 1);
                }
            }),
            M::ClassDeleted(index) => self.with_dpc(|s| {
                if s.can_delete(index) {
                    s.remove(index);
                    select(s, index.min(s.classes.len().saturating_sub(1)));
                }
            }),
            M::ClassAdded => {
                let base = signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::DIALOG_DPC_NEW_CLASS_NAME
                )
                .into_string();
                self.with_dpc(|s| {
                    let name = free_name(&s.classes, &base);
                    let at = s.classes.len();
                    s.insert_new(at, new_class(name));
                    select(s, at);
                });
            }
            // A locked class keeps its name: its directives use it.
            M::NameEdited(name) => self.with_dpc(|s| {
                if s.lock(s.selected).is_some() {
                    return;
                }
                if let Some(class) = s.classes.get_mut(s.selected) {
                    class.name = name;
                }
                s.problem = None;
            }),
            M::FieldEdited(field, text) => self.with_dpc(|s| edit_field(s, field, text)),
            M::PairBySuffixToggled(on) => self.with_dpc(|s| {
                if let Some(class) = s.classes.get_mut(s.selected) {
                    class.pair_by_suffix = on;
                }
            }),
            M::PreviewActed(msg) => {
                let Some(state) = self.ui_state.diff_pair_classes.as_mut() else {
                    return Task::none();
                };
                state.menu = None;
                return state
                    .act_on_preview(msg)
                    .map(|msg| crate::app::Message::from(M::PreviewActed(msg)));
            }
            M::RulesLinkClicked => {
                let id = crate::keymap::AppCommandId::new("open_rules").expect("a catalog id");
                return self.dispatch_command(&id, crate::app::command::CommandArgs::none(), None);
            }
            M::CancelRequested => self.ui_state.diff_pair_classes = None,
            M::OkRequested => self.apply_diff_pair_classes(),
        }
        Task::none()
    }

    fn with_dpc(&mut self, edit: impl FnOnce(&mut DiffPairClassesState)) {
        if let Some(state) = self.ui_state.diff_pair_classes.as_mut() {
            state.menu = None;
            edit(state);
        }
    }

    /// Open the dialog on the project of the document in view, with a copy
    /// of its classes and the pairs of its open sheets.
    fn open_diff_pair_classes(&mut self) {
        if self.ui_state.diff_pair_classes.is_some() {
            return;
        }
        let Some(project) = self.document_state.active_document_project() else {
            let line = signex_i18n::t!(self.ui_state.i18n, signex_i18n::ids::DIALOG_DPC_NO_PROJECT);
            crate::diagnostics::log_info(line.into_string());
            return;
        };
        let id = project.id;
        let classes = project.data.schematic_design.diff_pair_classes.clone();
        let project_name = project.data.name.clone();
        let has_closed_sheets = self.project_has_closed_sheets(project);
        let members = self.project_pair_members(id, &classes);
        let original_names = classes.iter().map(|c| Some(c.name.clone())).collect();
        self.interaction_state.context_menu = None;
        self.ui_state.diff_pair_classes = Some(Box::new(DiffPairClassesState {
            project: id,
            project_name,
            classes,
            original_names,
            has_closed_sheets,
            selected: 0,
            menu: None,
            members,
            drafts: Vec::new(),
            problem: None,
            preview: super::PairPreview::default(),
        }));
    }

    /// Whether a page of `project` has no open tab, so the pairs on it are
    /// not counted.
    fn project_has_closed_sheets(&self, project: &crate::app::state::LoadedProject) -> bool {
        use crate::app::sheet_hierarchy::normalised_key;
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let open: std::collections::HashSet<String> = self
            .document_state
            .engines
            .keys()
            .map(|path| normalised_key(path, fold))
            .collect();
        crate::app::project_sheets::project_pages(Some(project))
            .iter()
            .any(|page| !open.contains(&normalised_key(page, fold)))
    }

    /// The classed pairs of project `id`'s sheets that are open, sheet by
    /// sheet in file-name order.
    fn project_pair_members(
        &self,
        id: crate::app::state::ProjectId,
        classes: &[signex_types::diff_pair_class::DiffPairClass],
    ) -> Vec<PairMemberRow> {
        let mut sheets: Vec<(String, &signex_types::schematic::SchematicSheet)> = self
            .document_state
            .engines
            .iter()
            .filter(|(path, _)| {
                self.document_state
                    .document_project(path)
                    .is_some_and(|p| p.id == id)
            })
            .map(|(path, engine)| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (name, engine.document())
            })
            .collect();
        sheets.sort_by(|a, b| a.0.cmp(&b.0));
        signex_net::class_members(&sheets, classes)
            .into_iter()
            .map(|m| PairMemberRow {
                class: m.pair.class.clone().unwrap_or_default(),
                pair: m.pair.name.clone(),
                positive: m.pair.positive.clone().unwrap_or_default(),
                negative: m.pair.negative.clone().unwrap_or_default(),
                sheet: m.sheet,
            })
            .collect()
    }

    /// OK: write the classes to the project as one undo step and close; a
    /// blank or repeated name keeps the dialog open and says why.
    fn apply_diff_pair_classes(&mut self) {
        let Some(project_id) = self.ui_state.diff_pair_classes.as_ref().map(|s| s.project) else {
            return;
        };
        if self.refuse_schematic_design_edit(project_id) {
            return;
        }
        let Some(state) = self.ui_state.diff_pair_classes.as_mut() else {
            return;
        };
        if let Some(problem) = state.problem() {
            state.problem = Some(problem);
            return;
        }
        let classes = state.applied();
        self.ui_state.diff_pair_classes = None;
        let closed = signex_i18n::t!(
            self.ui_state.i18n,
            signex_i18n::ids::DIALOG_DPC_PROJECT_CLOSED
        );
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(closed.into_string());
            return;
        };
        let history = document.general_history.entry(project_id).or_default();
        let command = ProjectGeneralCommand::SetDiffPairClasses(classes);
        if history.apply(&mut project.data, command) {
            // Classes feed the board, not ERC.
            document.sync_project_history_dirty(project_id, false);
            self.refresh_panel_ctx();
        }
    }
}

/// Select class `index`; its drafts start empty and a value open on the
/// preview closes.
fn select(state: &mut DiffPairClassesState, index: usize) {
    state.selected = index;
    state.drafts.clear();
    state.preview.close_labels();
}

/// A typed value: kept as the draft, stored when it reads as a number of 0
/// or more (blank clears it). A value set on the preview comes here too.
pub(super) fn edit_field(state: &mut DiffPairClassesState, field: PairClassField, text: String) {
    if let Ok(value) = parse_value(&text)
        && let Some(class) = state.classes.get_mut(state.selected)
    {
        set_field(class, field, value);
    }
    state.drafts.retain(|(f, _)| *f != field);
    state.drafts.push((field, text));
}
