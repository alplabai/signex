//! #3015: the constraint menu (mockup row 188) through `Signex::update` on
//! both hosts. Show conflicts is on every constraint's menu, greyed with
//! "none" outside a conflict and selecting the conflict inside one;
//! Suppress is greyed with its reason on the row. #3014: a dimension bound
//! to a parameter reads its name and value on both hosts.

use iced::Size;
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::tests::{footprint_sketching, send, sketch_edit, sketching_off_grid};
use crate::app::{FootprintDocument, Message, Signex};
use crate::layout_probe;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::constraint_menu;
use crate::library::editor::footprint::sketch_mode::value_edit::{
    conflicting_constraints, dimension_label, resolved_params,
};
use crate::library::editor::footprint::sketch_mode::{SketchCanvasMsg, SketchEdit};
use crate::library::editor::footprint::{ValueTarget, state::FootprintEditorState};
use crate::library::messages::FootprintEditorMsg;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// P0 fixed at the origin; P1 0.65 mm away by the parameter `pitch`; P2
/// 4 mm from P1 by two equal literal distances, so a different value for
/// either over-constrains the sketch.
struct Body {
    data: SketchData,
    bound: ConstraintId,
    plain: ConstraintId,
    twin: ConstraintId,
}

fn body() -> Body {
    let plane = PlaneId::new();
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    data.parameters.insert("pitch", "0.65mm");
    let mut add = |x, y| {
        let id = SketchEntityId::new();
        data.entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let (p0, p1, p2) = (add(10.0, 10.0), add(10.65, 10.0), add(10.65, 14.0));
    let (bound, plain, twin) = (
        ConstraintId::new(),
        ConstraintId::new(),
        ConstraintId::new(),
    );
    let distance = |a, b, target| ConstraintKind::DistancePtPt {
        p1: a,
        p2: b,
        target,
    };
    let kinds = [
        (ConstraintId::new(), ConstraintKind::Fixed { point: p0 }),
        (bound, distance(p0, p1, DimTarget::Expr("pitch".into()))),
        (plain, distance(p1, p2, DimTarget::Literal(4.0))),
        (twin, distance(p1, p2, DimTarget::Literal(4.0))),
    ];
    data.constraints
        .extend(kinds.map(|(id, kind)| Constraint { id, kind }));
    Body {
        data,
        bound,
        plain,
        twin,
    }
}

fn install(editor: &mut FootprintDocument, data: SketchData) {
    editor.primitive_mut().sketch = Some(data);
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.mark_saved();
}

/// A footprint tab in sketch mode holding [`body`].
fn on_footprint() -> (Signex, std::path::PathBuf, Body) {
    let (mut app, path) = footprint_sketching();
    let body = body();
    let editor = app.document_state.footprint_editors.get_mut(&path);
    install(editor.expect("the editor"), body.data.clone());
    (app, path, body)
}

/// A board in sketch mode holding [`body`].
fn on_board() -> (Signex, tempfile::TempDir, std::path::PathBuf, Body) {
    let (mut app, tmp) = sketching_off_grid();
    let path = app.active_tab_path().expect("the board tab");
    let body = body();
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    install(&mut session.editor, body.data.clone());
    (app, tmp, path, body)
}

fn footprint_state<'a>(app: &'a Signex, path: &std::path::Path) -> &'a FootprintEditorState {
    &app.document_state
        .footprint_editors
        .get(path)
        .expect("the editor")
        .state
}

fn canvas(msg: SketchCanvasMsg) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchCanvas(msg)
}

fn open_menu(app: &mut Signex, path: &std::path::Path, constraint: ConstraintId) {
    let opened = SketchCanvasMsg::ConstraintMenuOpened {
        constraint,
        x: 300.0,
        y: 200.0,
    };
    sketch_edit(app, path, canvas(opened));
}

/// The main window's texts, isolation marks stripped.
fn main_window_texts(app: &mut Signex) -> Vec<String> {
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    layout_probe::lay_out_texts(app.view(window), Size::new(1400.0, 900.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// The open menu's Show conflicts and Suppress rows in the main window.
fn assert_menu_shows_both_rows(app: &mut Signex) {
    let texts = main_window_texts(app);
    let shows = |label: &str| texts.iter().any(|t| t == label);
    for label in ["Show conflicts", "none", "Suppress", "not available yet"] {
        assert!(shows(label), "{label:?} not in {texts:#?}");
    }
}

#[test]
fn the_footprint_menu_shows_show_conflicts_and_why_suppress_is_off() {
    let (mut app, path, body) = on_footprint();
    open_menu(&mut app, &path, body.plain);
    assert!(footprint_state(&app, &path).sketch_overlay.menu.is_some());
    assert_menu_shows_both_rows(&mut app);
}

#[test]
fn the_board_sketch_menu_shows_show_conflicts_and_why_suppress_is_off() {
    let (mut app, _tmp, path, body) = on_board();
    open_menu(&mut app, &path, body.plain);
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    assert!(session.editor.state.sketch_overlay.menu.is_some());
    assert_menu_shows_both_rows(&mut app);
}

/// A rejected value names its conflict; the menu's Show conflicts then
/// acts and selects the conflict's points.
#[test]
fn show_conflicts_selects_the_conflict_after_a_rejected_value() {
    let (mut app, path, body) = on_footprint();
    let edit = |app: &mut Signex, msg| sketch_edit(app, &path, canvas(msg));
    edit(
        &mut app,
        SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(body.twin)),
    );
    for _ in 0..8 {
        edit(&mut app, SketchCanvasMsg::ValueErased);
    }
    edit(&mut app, SketchCanvasMsg::ValueTyped('9'));
    edit(&mut app, SketchCanvasMsg::ValueCommitted);
    let in_conflict = conflicting_constraints(footprint_state(&app, &path));
    let fighting = *in_conflict
        .iter()
        .next()
        .expect("the rejected value names a conflict");

    open_menu(&mut app, &path, fighting);
    let state = footprint_state(&app, &path);
    let sketch = app.document_state.footprint_editors[&path]
        .primitive()
        .sketch
        .clone()
        .expect("a sketch");
    let i18n = signex_i18n::I18n::en_us_ref();
    let entries = constraint_menu::entries(state, &sketch, path.clone(), i18n).expect("open");
    let show = entries
        .into_iter()
        .find_map(|e| match e {
            DropdownEntry::Item(item) if item.label == "Show conflicts" => Some(item),
            _ => None,
        })
        .expect("Show conflicts is on the menu");
    assert!(!show.disabled);
    assert!(show.shortcut.is_none());
    let pressed = show.on_press.expect("Show conflicts acts in a conflict");
    send(&mut app, Message::Library(pressed));

    let state = footprint_state(&app, &path);
    assert!(state.sketch_overlay.menu.is_none(), "the menu closes");
    assert!(state.selected_sketch.is_some(), "the conflict is selected");
}

/// #3014: a dimension bound to a parameter reads `pitch` and its value;
/// an unnamed one reads its value only, on both hosts, in the words the
/// app rendered.
#[test]
fn a_bound_dimension_names_its_parameter_on_both_hosts() {
    let check = |state: &FootprintEditorState, sketch: &SketchData, body: &Body| {
        let params = resolved_params(sketch, state);
        let label = |id: ConstraintId| {
            let c = sketch.constraints.iter().find(|c| c.id == id);
            let shown = state.shown_constraint(&c.expect("the dimension").kind);
            dimension_label(&shown, sketch, &params, &state.canvas_text).expect("a label")
        };
        assert_eq!(
            label(body.bound),
            (Some("pitch".to_string()), "0.65 mm".to_string())
        );
        assert_eq!(label(body.plain), (None, "4.00 mm".to_string()));
    };

    let (mut app, path, body) = on_footprint();
    open_menu(&mut app, &path, body.plain);
    let sketch = app.document_state.footprint_editors[&path]
        .primitive()
        .sketch
        .clone()
        .expect("a sketch");
    check(footprint_state(&app, &path), &sketch, &body);

    let (mut app, _tmp, path, body) = on_board();
    open_menu(&mut app, &path, body.plain);
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    check(&session.editor.state, &session.working_sketch(), &body);
}
