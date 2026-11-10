//! Command-driven editing of the project's General settings (#1202).
//!
//! The Project tab's General, Parameters and Schematic defaults sections edit
//! the `.snxprj` `description`, `parameters` (#1080), designator standard
//! (#1421) and `schematic_defaults` (#2026) in place on the project's one
//! in-memory `ProjectData`. The project name is
//! not edited here: it is the file stem, changed only by the Rename flow.
//! Like the net-class table, the settings are tiny, so the history keeps
//! snapshots rather than inverse commands. A snapshot holds only the fields
//! this section owns: undo never reverts what another writer (a library add,
//! a net class, a rule change, a rename) did to the same project.
//!
//! Text is edited per keystroke. Consecutive edits coalesce into one undo
//! step (a "run"); the run closes on undo, redo, or
//! [`ProjectGeneralHistory::close_run`] (a section switch, a save).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use signex_history::{DEFAULT_UNDO_DEPTH, History};
use signex_types::agent_journal::AgentJournalSettings;
use signex_types::designator::DesignatorStandard;
use signex_types::project::{
    AnnotationState, PcbField, ProjectData, ProjectDefaults, SchematicDefaultField,
    SchematicDefaultOverrides,
};

/// How many undo steps are kept unless the `undo_depth` setting says
/// otherwise.
pub const HISTORY_LIMIT: usize = DEFAULT_UNDO_DEPTH;

/// One edit of the General section. Plain data, so a script can emit it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "text", rename_all = "snake_case")]
pub enum ProjectGeneralCommand {
    /// The description as typed; an empty text clears it.
    SetDescription(String),
    /// A project parameter as typed (#1080). An empty value on one of the
    /// sheet title-block keys clears it; a user-named parameter keeps an
    /// empty value until [`Self::RemoveParameter`].
    SetParameter { name: String, value: String },
    /// Delete a parameter.
    RemoveParameter(String),
    /// Pick the designator standard (#1421); `None` is "Library designator".
    /// Always its own undo step, and it closes the description typing run.
    /// An unreadable section from a newer Signex is replaced only by a real
    /// standard: picking `None` over it changes nothing.
    SetDesignatorStandard(Option<DesignatorStandard>),
    /// A schematic default as typed in the Schematic defaults section
    /// (#2026); `None` clears it back to the built-in value.
    SetSchematicDefault {
        field: SchematicDefaultField,
        value: Option<f64>,
    },
    /// The font new text is placed in (#2026); `None` clears it.
    SetSchematicDefaultFont(Option<String>),
    /// The line style of wires whose class sets none (#2026).
    SetSchematicDefaultLineStyle(Option<signex_types::net_class::NetLineStyle>),
    /// The fill new shapes are placed with (#2026).
    SetSchematicDefaultShapeFill(Option<signex_types::schematic::FillType>),
    /// The Project setup section (#2232): what new PCBs and sheets start
    /// from, and the designator standard when it changed, as one step.
    /// `typing` names the rule field being typed in: edits of the same field
    /// join one undo step; a pick (`None`) is always a step of its own.
    SetProjectDefaults {
        defaults: ProjectDefaults,
        designators: Option<DesignatorStandard>,
        typing: Option<PcbField>,
    },
    /// The agent journal settings (#2253). `typing` is true while the line
    /// cap is being typed: those edits join one undo step; a checkbox is a
    /// step of its own.
    SetAgentJournal {
        settings: AgentJournalSettings,
        typing: bool,
    },
}

/// The `title_block` keys a project parameter shares with a sheet: empty
/// means "not set", so the key is dropped rather than stored empty.
const TITLE_BLOCK_KEYS: [&str; 12] = [
    "title",
    "document_number",
    "rev",
    "date",
    "company",
    "drawn_by",
    "checked_by",
    "approved_by",
    "comment1",
    "comment2",
    "comment3",
    "comment4",
];

/// The fields the General section owns, as one undo snapshot.
#[derive(Debug, Clone, PartialEq)]
struct GeneralSnapshot {
    description: Option<String>,
    parameters: BTreeMap<String, String>,
    annotation: AnnotationState,
    schematic_defaults: SchematicDefaultOverrides,
    project_defaults: ProjectDefaults,
    agent_journal: AgentJournalSettings,
}

impl GeneralSnapshot {
    fn of(data: &ProjectData) -> Self {
        Self {
            description: data.description.clone(),
            parameters: data.parameters.clone(),
            annotation: data.annotation_state(),
            schematic_defaults: data.schematic_defaults.clone(),
            project_defaults: data.project_defaults.clone(),
            agent_journal: data.agent_journal,
        }
    }

    /// Put these fields back, leaving every other field as it is now.
    fn restore(self, data: &mut ProjectData) {
        data.description = self.description;
        data.parameters = self.parameters;
        data.restore_annotation_state(self.annotation);
        data.schematic_defaults = self.schematic_defaults;
        data.project_defaults = self.project_defaults;
        data.agent_journal = self.agent_journal;
    }
}

/// Undo/redo for one project's General settings.
#[derive(Debug, Clone)]
pub struct ProjectGeneralHistory {
    history: History<GeneralSnapshot>,
    /// An edit run is open: the next edit joins the last step.
    run_open: bool,
    /// What the open run edits; an edit of something else starts a new step.
    run_field: Option<RunField>,
}

/// What a typing run edits. Description and parameter edits share one run,
/// as before; each schematic default is a run of its own (#2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunField {
    General,
    SchematicDefault(SchematicDefaultField),
    SchematicDefaultFont,
    ProjectRule(PcbField),
    AgentJournalLines,
}

impl RunField {
    fn of(command: &ProjectGeneralCommand) -> Self {
        match command {
            ProjectGeneralCommand::SetSchematicDefault { field, .. } => {
                Self::SchematicDefault(*field)
            }
            ProjectGeneralCommand::SetSchematicDefaultFont(_) => Self::SchematicDefaultFont,
            ProjectGeneralCommand::SetProjectDefaults {
                typing: Some(field),
                ..
            } => Self::ProjectRule(*field),
            ProjectGeneralCommand::SetAgentJournal { typing: true, .. } => Self::AgentJournalLines,
            _ => Self::General,
        }
    }
}

impl Default for ProjectGeneralHistory {
    fn default() -> Self {
        Self::with_depth(HISTORY_LIMIT)
    }
}

impl ProjectGeneralHistory {
    /// An empty history that keeps `depth` undo steps (the `undo_depth`
    /// setting).
    pub fn with_depth(depth: usize) -> Self {
        Self {
            history: History::unknown(depth),
            run_open: false,
            run_field: None,
        }
    }

    /// Keep `depth` undo steps from now on; lowering it drops the oldest.
    pub fn set_undo_depth(&mut self, depth: usize) {
        self.history.set_cap(depth);
    }

    /// How many undo steps are kept.
    pub fn undo_depth(&self) -> usize {
        self.history.cap()
    }

    /// Apply `command` to `data`. Returns `true` when a field changed. A
    /// change opens a new undo step unless it continues the open run; a
    /// no-op records nothing.
    pub fn apply(&mut self, data: &mut ProjectData, command: ProjectGeneralCommand) -> bool {
        let before = GeneralSnapshot::of(data);
        let is_pick = matches!(
            command,
            ProjectGeneralCommand::SetDesignatorStandard(_)
                | ProjectGeneralCommand::SetSchematicDefaultLineStyle(_)
                | ProjectGeneralCommand::SetSchematicDefaultShapeFill(_)
                | ProjectGeneralCommand::SetProjectDefaults { typing: None, .. }
                | ProjectGeneralCommand::SetAgentJournal { typing: false, .. }
        );
        let field = RunField::of(&command);
        if is_pick || self.run_field != Some(field) {
            self.run_open = false;
        }
        self.run_field = Some(field);
        match command {
            ProjectGeneralCommand::SetDescription(text) => {
                data.description = (!text.is_empty()).then_some(text);
            }
            ProjectGeneralCommand::SetParameter { name, value } => {
                if name.is_empty()
                    || (value.is_empty() && TITLE_BLOCK_KEYS.contains(&name.as_str()))
                {
                    data.parameters.remove(&name);
                } else {
                    data.parameters.insert(name, value);
                }
            }
            ProjectGeneralCommand::RemoveParameter(name) => {
                data.parameters.remove(&name);
            }
            ProjectGeneralCommand::SetDesignatorStandard(standard) => {
                if !(standard.is_none() && data.has_unreadable_annotation()) {
                    data.set_designator_standard(standard);
                }
            }
            ProjectGeneralCommand::SetSchematicDefault { field, value } => {
                data.schematic_defaults = data.schematic_defaults.with(field, value);
            }
            ProjectGeneralCommand::SetSchematicDefaultFont(font) => {
                data.schematic_defaults.set_font_family(font);
            }
            ProjectGeneralCommand::SetSchematicDefaultLineStyle(line) => {
                data.schematic_defaults.set_wire_line_style(line);
            }
            ProjectGeneralCommand::SetSchematicDefaultShapeFill(fill) => {
                data.schematic_defaults.set_shape_fill(fill);
            }
            ProjectGeneralCommand::SetProjectDefaults {
                defaults,
                designators,
                ..
            } => {
                data.project_defaults = defaults;
                if let Some(standard) = designators {
                    data.set_designator_standard(Some(standard));
                }
            }
            ProjectGeneralCommand::SetAgentJournal { settings, .. } => {
                data.agent_journal = settings.clamped();
            }
        }
        if GeneralSnapshot::of(data) == before {
            return false;
        }
        // An open run follows a recorded step, so no redo step exists to cut.
        if !self.run_open {
            self.history.record(before);
            // A pick stays a step of its own: the next edit opens a new one.
            self.run_open = !is_pick;
        }
        true
    }

    /// End the open edit run: the next edit is a new undo step.
    pub fn close_run(&mut self) {
        self.run_open = false;
    }

    /// Step back. `false` when there is nothing to undo.
    pub fn undo(&mut self, data: &mut ProjectData) -> bool {
        self.run_open = false;
        let Some(previous) = self.history.begin_step(true) else {
            return false;
        };
        self.history.finish_step(GeneralSnapshot::of(data), true);
        previous.restore(data);
        true
    }

    /// Step forward. `false` when there is nothing to redo.
    pub fn redo(&mut self, data: &mut ProjectData) -> bool {
        self.run_open = false;
        let Some(next) = self.history.begin_step(false) else {
            return false;
        };
        self.history.finish_step(GeneralSnapshot::of(data), false);
        next.restore(data);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
}

#[cfg(test)]
mod tests;
