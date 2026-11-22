//! Function audit, wave A batch g5: mockup rows 89 to 92 (the Rules tab, its
//! clearance matrix, presets and text view, and the rule editor). Each test
//! drives `Signex::update` and checks what the control changed.

#[path = "g5/m89_grid.rs"]
mod m89_grid;
#[path = "g5/m90_matrix.rs"]
mod m90_matrix;
#[path = "g5/m91_presets_sets.rs"]
mod m91_presets_sets;
#[path = "g5/m91_text.rs"]
mod m91_text;
#[path = "g5/m92_profiles.rs"]
mod m92_profiles;

use std::path::PathBuf;

use signex_app::app::{
    LoadedProject, Message, ProjectId, ProjectMsg, RulesTabMsg, RulesTabState, Signex,
};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::{self, LimitField, RuleTable};
use signex_types::net_class::NetClass;
use signex_types::project::{DrcRuleSet, ProjectData, ProjectRules};
use signex_widgets::constraint_grid::{CellRef, ColumnId, GridMessage, MatrixMessage, RowId};

/// The one rule most projects here start with.
const SIG_RULE: &str =
    "rule sig on clearance when net == \"SIG\" require clearance >= 0.3mm then error\n";

/// A project with a Rules tab open on the Clearance domain.
struct Rig {
    _dir: tempfile::TempDir,
    app: Signex,
    id: ProjectId,
    path: PathBuf,
}

impl Rig {
    /// `text` is the project's stored rule text; `None` stores no rules, so
    /// the tab reads this machine's.
    fn new(text: Option<&str>, classes: &[&str]) -> Self {
        let dir = tempfile::tempdir().expect("a temp dir");
        let (mut app, _task) = Signex::new();
        let path = dir.path().join("audit.snxprj");
        let rules = text.map(|text| ProjectRules {
            drc: DrcRuleSet {
                dsl: Some(text.to_string()),
                ..Default::default()
            },
            ..ProjectRules::default()
        });
        let data = ProjectData {
            document_id: None,
            name: "audit".to_string(),
            description: None,
            dir: dir.path().to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: classes.iter().map(|c| NetClass::named(*c)).collect(),
            rules,
            annotation: None,
            parameters: Default::default(),
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            schematic_design: Default::default(),
            fab_outputs: None,
            router_tool_diameter: None,
            requirements: Default::default(),
            unknown: Default::default(),
        };
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: path.clone(),
            data,
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
        app.document_state.active_project = Some(id);
        let mut rig = Rig {
            _dir: dir,
            app,
            id,
            path,
        };
        rig.send(RulesTabMsg::OpenRequested { project: Some(id) });
        rig.send(RulesTabMsg::DomainPicked {
            project_id: id,
            domain: Domain::Clearance,
        });
        rig
    }

    fn send(&mut self, msg: RulesTabMsg) {
        let _ = self.app.update(Message::Project(ProjectMsg::Rules(msg)));
    }

    fn grid(&mut self, msg: GridMessage) {
        let project_id = self.id;
        self.send(RulesTabMsg::GridActed { project_id, msg });
    }

    fn matrix(&mut self, msg: MatrixMessage) {
        let project_id = self.id;
        self.send(RulesTabMsg::MatrixActed { project_id, msg });
    }

    /// Edit ▸ Undo, as the main menu sends it.
    fn undo(&mut self) {
        let undo = AppCommandId::new("undo").expect("a command id");
        let _ = self.app.update(Message::Menu(MenuMessage::Command(undo)));
    }

    /// Press the All row's Clearance cell, type `value`, press Enter.
    fn type_in_all_row(&mut self, value: &str) {
        self.grid(GridMessage::CellPressed(all_row_clearance()));
        self.grid(GridMessage::EditTextChanged(value.to_string()));
        self.grid(GridMessage::EditSubmitted);
    }

    fn tab(&self) -> &RulesTabState {
        &self.app.document_state.project_tabs[&self.id].rules
    }

    fn tab_mut(&mut self) -> &mut RulesTabState {
        &mut self
            .app
            .document_state
            .project_tabs
            .get_mut(&self.id)
            .expect("the project's tab state")
            .rules
    }

    fn rules(&self) -> Option<ProjectRules> {
        self.app
            .document_state
            .project_by_id(self.id)
            .expect("the project is open")
            .data
            .rules
            .clone()
    }

    /// The project's stored rule text.
    fn stored(&self) -> String {
        self.rules()
            .and_then(|rules| rules.drc.dsl)
            .unwrap_or_default()
    }

    fn table(&self) -> RuleTable {
        table::table(&self.stored())
    }

    /// The minimum clearance rule `name` writes.
    fn clearance_of(&self, name: &str) -> Option<String> {
        let rows = self.table();
        rows.row(name)
            .and_then(|row| row.limit(LimitField::MinClearance))
            .map(|limit| limit.value.clone())
    }

    fn router(&self) -> Option<f64> {
        self.app
            .document_state
            .project_by_id(self.id)
            .expect("the project is open")
            .data
            .router_tool_diameter()
    }

    fn is_dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    fn refusal(&self) -> String {
        self.tab().refused.clone().unwrap_or_default()
    }

    /// The stored rules are still `text` and the project is not dirty.
    #[track_caller]
    fn assert_untouched(&self, text: &str) {
        assert_eq!(self.stored(), text, "the stored rules did not change");
        assert!(!self.is_dirty(), "nothing changed, so the project is clean");
    }
}

/// The All row is the first row, the Clearance group's Min the first column
/// of the Clearance domain.
fn all_row_clearance() -> CellRef {
    CellRef::new(RowId(0), ColumnId(0))
}
