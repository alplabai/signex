//! mockup row 187 / mockup row 188 (decision 5, Caner 2026-11-20) - Convert to reference: the
//! conflicting dimension of a rejected edit becomes a reference as one
//! undo step and the conflict clears; the reference then follows the
//! driving dimension and never moves geometry; Make driving that
//! over-constrains is refused and records nothing; the Properties
//! conflict row's Convert does the same as the menu; undo restores driving.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::EditorMode;
use signex_app::library::editor::footprint::ValueTarget;
use signex_app::library::editor::footprint::sketch_mode::SketchCanvasMsg;
use signex_app::library::editor::footprint::sketch_mode::constraint_menu::SketchReferenceMsg;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_app::panels::{PanelMsg, SketchFixMsg};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::measure::measured_value;
use signex_sketch::solver::state::point_xy;
use std::path::PathBuf;

struct Rig {
    app: Signex,
    path: PathBuf,
    /// The point both distances measure to.
    b: SketchEntityId,
    /// A 5 mm distance from the fixed origin to `b`.
    first: ConstraintId,
    /// A second 5 mm distance between the same points: a different value
    /// for either over-constrains the sketch.
    second: ConstraintId,
}

fn rig() -> Rig {
    let plane = PlaneId::new();
    let (a, b) = (SketchEntityId::new(), SketchEntityId::new());
    let (first, second) = (ConstraintId::new(), ConstraintId::new());
    let distance = |id| Constraint {
        id,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Literal(5.0),
        },
    };
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(b, plane, EntityKind::Point { x: 5.0, y: 0.0 }),
        ],
        constraints: vec![
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Fixed { point: a },
            },
            distance(first),
            distance(second),
        ],
        ..SketchData::default()
    };
    let mut fp = Footprint::empty("reference");
    fp.sketch = Some(sketch);
    let path = PathBuf::from("reference.snxfpt");
    let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "reference".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    let mut rig = Rig {
        app,
        path,
        b,
        first,
        second,
    };
    rig.edit(FootprintEditorMsg::SetMode(EditorMode::Sketch));
    rig
}

impl Rig {
    fn edit(&mut self, msg: FootprintEditorMsg) {
        drop(
            self.app
                .update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path: self.path.clone(),
                    msg: PrimitiveEdit::Footprint(msg),
                })),
        );
    }

    fn canvas(&mut self, msg: SketchCanvasMsg) {
        self.edit(FootprintEditorMsg::SketchCanvas(msg));
    }

    fn reference(&mut self, msg: SketchReferenceMsg) {
        self.edit(FootprintEditorMsg::SketchReference(msg));
    }

    fn panel(&mut self, msg: PanelMsg) {
        drop(self.app.update(Message::Dock(DockMessage::Panel(msg))));
    }

    /// Write `text` into the value of `id` on the drawing and press Enter.
    fn write_value(&mut self, id: ConstraintId, text: &str) {
        self.canvas(SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(
            id,
        )));
        while self
            .editor()
            .state
            .sketch_overlay
            .value_edit
            .as_ref()
            .is_some_and(|e| !e.buffer.is_empty())
        {
            self.canvas(SketchCanvasMsg::ValueErased);
        }
        for ch in text.chars() {
            self.canvas(SketchCanvasMsg::ValueTyped(ch));
        }
        self.canvas(SketchCanvasMsg::ValueCommitted);
    }

    /// A rejected edit: `second` set to 7 mm clashes with `first`.
    fn reject_an_edit(&mut self) {
        self.write_value(self.second, "7");
        assert!(
            !self.editor().state.conflict.is_empty(),
            "the clash is named"
        );
        assert!(!self.sketch().is_driven(self.second));
    }

    fn editor(&self) -> &FootprintDocument {
        &self.app.document_state.footprint_editors[&self.path]
    }

    fn editor_mut(&mut self) -> &mut FootprintDocument {
        self.app
            .document_state
            .footprint_editors
            .get_mut(&self.path)
            .expect("editor")
    }

    fn sketch(&self) -> &SketchData {
        self.editor().primitive().sketch.as_ref().expect("sketch")
    }

    fn undo_steps(&self) -> usize {
        self.editor().history.undo_len()
    }

    fn target(&self, id: ConstraintId) -> DimTarget {
        let c = self.sketch().constraints.iter().find(|c| c.id == id);
        c.and_then(|c| c.kind.dim_target())
            .cloned()
            .expect("a dimension")
    }

    /// What dimension `id` measures at the last solve.
    fn measured(&self, id: ConstraintId) -> Option<f64> {
        let c = self.sketch().constraints.iter().find(|c| c.id == id)?;
        let solve = self.editor().state.last_solve.as_ref().map(|s| &s.result);
        measured_value(self.sketch(), solve, c)
    }

    /// How far the drawing puts `b` from the origin: where the last applied
    /// solve put it. A solve never writes into the point as authored.
    fn b_distance(&self) -> f64 {
        let solve = self.editor().state.last_solve.as_ref().expect("solved");
        let (state, index) = (&solve.result.state, &solve.result.index);
        let (x, y) = point_xy(self.b, state, index, self.sketch()).expect("b");
        x.hypot(y)
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn convert_to_reference_clears_the_conflict_as_one_undo_step() {
    let mut rig = rig();
    rig.reject_an_edit();
    let before = rig.undo_steps();

    rig.reference(SketchReferenceMsg::ReferenceConverted(rig.second));

    assert!(rig.sketch().is_driven(rig.second));
    assert_eq!(rig.sketch().driven_dimensions, [rig.second]);
    assert!(
        rig.editor().state.conflict.is_empty(),
        "the conflict clears"
    );
    assert!(
        rig.editor()
            .state
            .last_solve
            .as_ref()
            .is_some_and(|s| s.over_constraints.is_empty()),
        "and the sketch solves"
    );
    assert_eq!(rig.undo_steps(), before + 1, "one undo step");
    assert!(rig.editor().is_dirty());
}

#[test]
fn a_reference_follows_the_driving_dimension_and_never_moves_geometry() {
    let mut rig = rig();
    rig.reference(SketchReferenceMsg::ReferenceConverted(rig.second));

    rig.write_value(rig.first, "7");

    assert_eq!(rig.target(rig.first), DimTarget::Literal(7.0));
    assert!(close(rig.b_distance(), 7.0), "{}", rig.b_distance());
    assert_eq!(
        rig.target(rig.second),
        DimTarget::Literal(5.0),
        "the reference keeps its old target"
    );
    let measured = rig.measured(rig.second).expect("measures");
    assert!(close(measured, 7.0), "it measures the 7 mm: {measured}");
}

#[test]
fn make_driving_that_over_constrains_is_refused_and_records_nothing() {
    let mut rig = rig();
    rig.reference(SketchReferenceMsg::ReferenceConverted(rig.second));
    rig.write_value(rig.first, "7");
    let before = rig.undo_steps();

    rig.reference(SketchReferenceMsg::DrivingRestored(rig.second));

    assert!(rig.sketch().is_driven(rig.second), "still a reference");
    assert_eq!(rig.undo_steps(), before, "no step");
    assert!(
        !rig.editor().state.conflict.is_empty(),
        "the conflict is named"
    );
    assert!(close(rig.b_distance(), 7.0), "the geometry stays");
}

#[test]
fn make_driving_that_fits_is_one_undo_step() {
    let mut rig = rig();
    rig.reference(SketchReferenceMsg::ReferenceConverted(rig.second));
    let before = rig.undo_steps();

    rig.reference(SketchReferenceMsg::DrivingRestored(rig.second));

    assert!(!rig.sketch().is_driven(rig.second));
    assert!(rig.sketch().driven_dimensions.is_empty());
    assert_eq!(rig.undo_steps(), before + 1);
}

#[test]
fn the_properties_conflict_convert_does_what_the_menu_does() {
    let mut rig = rig();
    rig.reject_an_edit();
    let before = rig.undo_steps();

    rig.panel(PanelMsg::SketchFix(SketchFixMsg::ConflictConvertClicked(
        rig.second,
    )));

    assert!(rig.sketch().is_driven(rig.second));
    assert!(
        rig.editor().state.conflict.is_empty(),
        "the conflict clears"
    );
    assert_eq!(rig.undo_steps(), before + 1, "one undo step");
}

#[test]
fn undo_restores_the_dimension_as_driving() {
    let mut rig = rig();
    rig.reject_an_edit();
    rig.reference(SketchReferenceMsg::ReferenceConverted(rig.second));
    assert!(rig.sketch().is_driven(rig.second));

    assert!(rig.editor_mut().undo(), "the step undoes");

    assert!(!rig.sketch().is_driven(rig.second), "driving again");
    assert!(rig.sketch().driven_dimensions.is_empty());
}

#[test]
fn a_reference_value_does_not_open_for_editing() {
    let mut rig = rig();
    rig.reference(SketchReferenceMsg::ReferenceConverted(rig.second));
    rig.canvas(SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(
        rig.second,
    )));
    assert!(rig.editor().state.sketch_overlay.value_edit.is_none());
}
