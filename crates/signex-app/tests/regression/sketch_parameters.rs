//! mockup row 189 — the sketch parameters table: open and close, add, typing that
//! re-solves and an error that keeps the last solve, rename, the in-use
//! delete prompt, and Make parameter on a dimension, each one undo step.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::library::editor::footprint::EditorMode;
use signex_app::library::editor::footprint::parameters_table::SketchParametersMsg;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::parameter_refs::{ParameterNameError, RowValue, resolve_rows};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::state::point_xy;
use std::path::{Path, PathBuf};

struct Rig {
    app: Signex,
    path: PathBuf,
    /// The point a distance driven by `w` moves.
    b: SketchEntityId,
    /// A distance with a literal value, for Make parameter.
    literal: ConstraintId,
}

/// `a` fixed at the origin; `b` at the distance `w` (2 mm) from it; `c` at
/// a literal 1 mm from it. Opened in Sketch mode.
fn rig() -> Rig {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let mut point = |x: f64, y: f64| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let (a, b, c) = (point(0.0, 0.0), point(1.0, 0.0), point(0.0, 1.5));
    sketch.parameters.insert("w", "2mm");
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: a },
    });
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Expr("w".into()),
        },
    });
    let literal = ConstraintId::new();
    sketch.constraints.push(Constraint {
        id: literal,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: c,
            target: DimTarget::Literal(1.0),
        },
    });
    let mut fp = Footprint::empty("params");
    fp.sketch = Some(sketch);
    let path = PathBuf::from("params.snxfpt");
    let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "params".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SetMode(EditorMode::Sketch),
    );
    Rig {
        app,
        path,
        b,
        literal,
    }
}

fn edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        })),
    );
}

impl Rig {
    fn send(&mut self, msg: SketchParametersMsg) {
        edit(
            &mut self.app,
            &self.path,
            FootprintEditorMsg::SketchParameters(msg),
        );
    }

    fn editor(&self) -> &FootprintDocument {
        &self.app.document_state.footprint_editors[&self.path]
    }

    fn editor_mut(&mut self) -> &mut FootprintDocument {
        self.app
            .document_state
            .footprint_editors
            .get_mut(&self.path)
            .unwrap()
    }

    fn sketch(&self) -> &SketchData {
        self.editor().primitive().sketch.as_ref().unwrap()
    }

    fn parameter(&self, name: &str) -> Option<String> {
        self.sketch().parameters.get_raw(name).map(str::to_string)
    }

    fn target(&self, index: usize) -> DimTarget {
        self.sketch().constraints[index]
            .kind
            .dim_target()
            .cloned()
            .unwrap()
    }

    /// How far the drawing puts `b` from the origin: where the last applied
    /// solve put it. A solve never writes into the point as authored.
    fn b_distance(&self) -> f64 {
        let solve = self.editor().state.last_solve.as_ref().expect("solved");
        let (state, index) = (&solve.result.state, &solve.result.index);
        let (x, y) = point_xy(self.b, state, index, self.sketch()).expect("b");
        x.hypot(y)
    }

    fn undo_steps(&self) -> usize {
        self.editor().history.undo_len()
    }
}

#[test]
fn parameters_opens_the_table_and_close_or_esc_closes_it() {
    let mut rig = rig();
    assert!(rig.editor().state.parameters_table.is_none());
    rig.send(SketchParametersMsg::TableToggled);
    assert!(rig.editor().state.parameters_table.is_some());
    rig.send(SketchParametersMsg::TableClosed);
    assert!(rig.editor().state.parameters_table.is_none());
    rig.send(SketchParametersMsg::TableToggled);
    rig.send(SketchParametersMsg::RowMenuToggled { name: "w".into() });
    edit(
        &mut rig.app,
        &rig.path.clone(),
        FootprintEditorMsg::ToolEscape,
    );
    let table = rig.editor().state.parameters_table.as_ref().expect("open");
    assert_eq!(table.menu, None, "Esc closes the row menu first");
    edit(
        &mut rig.app,
        &rig.path.clone(),
        FootprintEditorMsg::ToolEscape,
    );
    assert!(
        rig.editor().state.parameters_table.is_none(),
        "then the table"
    );
    assert_eq!(rig.undo_steps(), 0, "opening and closing is not an edit");
}

#[test]
fn add_parameter_adds_a_unique_focused_row_as_one_undo_step() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    let before = rig.undo_steps();
    rig.send(SketchParametersMsg::AddClicked);
    rig.send(SketchParametersMsg::AddClicked);
    assert_eq!(rig.parameter("param_1").as_deref(), Some("1mm"));
    assert_eq!(rig.parameter("param_2").as_deref(), Some("1mm"));
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(table.focus.as_deref(), Some("param_2"));
    assert_eq!(rig.undo_steps(), before + 2);
    assert!(rig.editor_mut().undo());
    assert_eq!(rig.parameter("param_2"), None);
    assert!(rig.parameter("param_1").is_some());
}

#[test]
fn typing_re_solves_and_an_error_keeps_the_last_good_solve() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    assert!((rig.b_distance() - 2.0).abs() < 1e-6, "solved on entry");
    rig.send(SketchParametersMsg::ExpressionEdited {
        name: "w".into(),
        expr: "3mm".into(),
    });
    assert!(
        (rig.b_distance() - 3.0).abs() < 1e-6,
        "re-solved on the keystroke"
    );
    rig.send(SketchParametersMsg::ExpressionEdited {
        name: "w".into(),
        expr: "bodyy + 1mm".into(),
    });
    // The text is kept, the drawing keeps the last solve, the row is
    // marked, and the table remembers the value the drawing uses.
    assert_eq!(rig.parameter("w").as_deref(), Some("bodyy + 1mm"));
    assert!(
        (rig.b_distance() - 3.0).abs() < 1e-6,
        "last good solve kept"
    );
    let rows = resolve_rows(&rig.sketch().parameters);
    assert!(matches!(rows["w"], RowValue::Error(_)));
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(table.focus.as_deref(), Some("w"));
    let last = table.last_good.get("w").expect("remembered");
    assert!((last.value - 3.0).abs() < 1e-9);
}

#[test]
fn typing_records_one_undo_step_per_keystroke_as_the_properties_list_does() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    let before = rig.undo_steps();
    for expr in ["3", "3m", "3mm"] {
        rig.send(SketchParametersMsg::ExpressionEdited {
            name: "w".into(),
            expr: expr.into(),
        });
    }
    assert_eq!(rig.undo_steps(), before + 3);
}

#[test]
fn rename_renames_every_reference_as_one_undo_step() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    rig.send(SketchParametersMsg::RowMenuToggled { name: "w".into() });
    rig.send(SketchParametersMsg::RenameStarted { name: "w".into() });
    rig.send(SketchParametersMsg::RenameDraftChanged("width".into()));
    let before = rig.undo_steps();
    rig.send(SketchParametersMsg::RenameSubmitted);
    assert_eq!(rig.parameter("w"), None);
    assert_eq!(rig.parameter("width").as_deref(), Some("2mm"));
    assert_eq!(rig.target(1), DimTarget::Expr("width".into()));
    assert_eq!(rig.undo_steps(), before + 1);
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(table.rename, None);
    assert_eq!(table.focus.as_deref(), Some("width"));
    assert!(rig.editor_mut().undo());
    assert_eq!(rig.parameter("w").as_deref(), Some("2mm"));
    assert_eq!(rig.target(1), DimTarget::Expr("w".into()));
}

#[test]
fn a_refused_rename_says_why_and_changes_nothing() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    rig.send(SketchParametersMsg::RenameStarted { name: "w".into() });
    rig.send(SketchParametersMsg::RenameDraftChanged("pi".into()));
    let (before, steps) = (rig.sketch().clone(), rig.undo_steps());
    rig.send(SketchParametersMsg::RenameSubmitted);
    assert_eq!(rig.sketch(), &before);
    assert_eq!(rig.undo_steps(), steps);
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    let rename = table.rename.as_ref().expect("still renaming");
    assert_eq!(
        rename.error,
        Some(ParameterNameError::Reserved("pi".into()))
    );
    // Typing again clears the refusal.
    rig.send(SketchParametersMsg::RenameDraftChanged("p".into()));
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(table.rename.as_ref().unwrap().error, None);
}

#[test]
fn deleting_a_used_parameter_asks_and_replace_with_value_inlines_it() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    let steps = rig.undo_steps();
    rig.send(SketchParametersMsg::DeleteRequested { name: "w".into() });
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(table.delete_prompt.as_deref(), Some("w"), "asks first");
    assert!(rig.parameter("w").is_some(), "nothing deleted yet");
    rig.send(SketchParametersMsg::DeleteCancelled);
    assert!(rig.parameter("w").is_some(), "Cancel keeps it");
    assert_eq!(rig.undo_steps(), steps);

    rig.send(SketchParametersMsg::DeleteRequested { name: "w".into() });
    rig.send(SketchParametersMsg::DeleteConfirmed);
    assert_eq!(rig.parameter("w"), None);
    assert_eq!(rig.target(1), DimTarget::Expr("2mm".into()));
    assert!(
        (rig.b_distance() - 2.0).abs() < 1e-6,
        "the drawing is unchanged"
    );
    assert_eq!(
        rig.undo_steps(),
        steps + 1,
        "inline and delete are one step"
    );
    assert!(rig.editor_mut().undo());
    assert_eq!(rig.parameter("w").as_deref(), Some("2mm"));
    assert_eq!(rig.target(1), DimTarget::Expr("w".into()));
}

#[test]
fn deleting_an_unused_parameter_needs_no_answer() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    rig.send(SketchParametersMsg::AddClicked);
    rig.send(SketchParametersMsg::DeleteRequested {
        name: "param_1".into(),
    });
    assert_eq!(rig.parameter("param_1"), None);
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(table.delete_prompt, None);
}

#[test]
fn make_parameter_names_a_dimension_as_one_undo_step_and_opens_the_table() {
    let mut rig = rig();
    assert!(rig.editor().state.parameters_table.is_none());
    let steps = rig.undo_steps();
    let constraint = rig.literal;
    rig.send(SketchParametersMsg::ParameterFromConstraintRequested { constraint });
    assert_eq!(rig.parameter("dim_1").as_deref(), Some("1mm"));
    assert_eq!(rig.target(2), DimTarget::Expr("dim_1".into()));
    assert_eq!(rig.undo_steps(), steps + 1);
    let table = rig
        .editor()
        .state
        .parameters_table
        .as_ref()
        .expect("opened");
    assert_eq!(table.focus.as_deref(), Some("dim_1"));
    assert!(rig.editor_mut().undo());
    assert_eq!(rig.parameter("dim_1"), None);
    assert_eq!(rig.target(2), DimTarget::Literal(1.0));
}

#[test]
fn make_parameter_on_a_dimension_that_reads_one_keeps_it_and_records_nothing() {
    let mut rig = rig();
    let steps = rig.undo_steps();
    let constraint = rig.sketch().constraints[1].id;
    rig.send(SketchParametersMsg::ParameterFromConstraintRequested { constraint });
    assert_eq!(rig.undo_steps(), steps);
    assert_eq!(rig.target(1), DimTarget::Expr("w".into()));
    let table = rig
        .editor()
        .state
        .parameters_table
        .as_ref()
        .expect("opened");
    assert_eq!(table.focus.as_deref(), Some("w"));
}

/// On a board, Esc with the table open closes the table and stays in
/// sketch mode: it does not reach Discard changes first.
#[test]
fn on_a_board_esc_closes_the_table_before_discard() {
    use crate::board_sketch::{edit as board_edit, in_sketch, open_board, send};
    use crate::menu_support::menu_row;
    use signex_app::app::InputMsg;

    let mut f = open_board();
    send(&mut f, menu_row("board_sketch"));
    board_edit(
        &mut f,
        FootprintEditorMsg::SketchParameters(SketchParametersMsg::TableToggled),
    );
    let table_open = |f: &crate::board_sketch::Fixture| {
        f.app
            .interaction_state
            .board_sketch
            .as_ref()
            .is_some_and(|s| s.editor.state.parameters_table.is_some())
    };
    assert!(table_open(&f));
    send(
        &mut f,
        Message::Input(InputMsg::EscapePressed { window: None }),
    );
    assert!(in_sketch(&f), "Esc closed the table, not sketch mode");
    assert!(!table_open(&f), "the table closed");
}

/// Mockup row 189: Import from another footprint lists its parameters, a
/// name in use stays unticked and keeps its value, and the copy is one step.
#[test]
fn importing_parameters_copies_the_ticked_ones_as_one_undo_step() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    let steps = rig.undo_steps();
    rig.send(SketchParametersMsg::ImportSourcePicked {
        source: "QFN-16".into(),
        parameters: vec![
            ("w".into(), "9mm".into()),
            ("pitch".into(), "0.65mm".into()),
            ("span".into(), "4mm".into()),
        ],
    });
    assert_eq!(rig.undo_steps(), steps, "listing copies nothing");
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    let import = table.import.as_ref().expect("the list is open");
    let ticked: Vec<&str> = import.picked().map(|r| r.name.as_str()).collect();
    assert_eq!(ticked, ["pitch", "span"], "a name in use is not offered");

    rig.send(SketchParametersMsg::ImportRowToggled {
        name: "span".into(),
    });
    rig.send(SketchParametersMsg::ImportRowToggled { name: "w".into() });
    rig.send(SketchParametersMsg::ImportConfirmed);

    assert_eq!(rig.parameter("w").as_deref(), Some("2mm"), "never overwritten");
    assert_eq!(rig.parameter("pitch").as_deref(), Some("0.65mm"));
    assert_eq!(rig.parameter("span"), None, "unticked");
    assert_eq!(rig.undo_steps(), steps + 1);
    let table = rig.editor().state.parameters_table.as_ref().unwrap();
    assert!(table.import.is_none(), "the list closed");
    assert!(rig.editor_mut().undo());
    assert_eq!(rig.parameter("pitch"), None);
}

#[test]
fn cancelling_an_import_copies_nothing() {
    let mut rig = rig();
    rig.send(SketchParametersMsg::TableToggled);
    let steps = rig.undo_steps();
    rig.send(SketchParametersMsg::ImportSourcePicked {
        source: "QFN-16".into(),
        parameters: vec![("pitch".into(), "0.65mm".into())],
    });
    rig.send(SketchParametersMsg::ImportCancelled);
    assert_eq!(rig.parameter("pitch"), None);
    assert_eq!(rig.undo_steps(), steps);
}
