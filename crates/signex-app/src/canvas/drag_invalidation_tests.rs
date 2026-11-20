//! Drag invalidation baseline (ADR-0006, "Safety net before the high-risk
//! slices": a drag must not over-invalidate the projector). A schematic drag
//! runs through the GPU mount's input canvas, every published message and the
//! window's `DragMove` go through `Signex::update`, and both shader layers
//! draw each step, with no GPU. The counters are counts, never time.
//!
//! The baselines are trunk's counts when this landed. A later slice may lower
//! them (then lower the constant with it), never raise them.

use iced::widget::canvas::Program as _;
use iced::{Event, mouse};

use super::gpu_drag::SchematicDragShaderProgram;
use super::gpu_mount::SchematicShaderProgram;
use super::move_pick::tests::{BOUNDS, screen, slot_with_wire};
use super::tests::test_prefs;
use super::*;
use crate::app::{Message, Signex, TabInfo, TabKind, UiMsg};
use signex_types::schematic::{SelectedItem, SelectedKind};

/// Cursor steps of one drag.
const STEPS: usize = 8;
/// World distance the cursor travels per step: one 1.27 mm grid pitch.
const STRIDE_MM: f32 = 1.27;

/// The invalidation counters the engine and the canvas already keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counts {
    /// Recorded engine edits (the undo position; revisions are process-wide
    /// ids, not a count): what reaches the projector as a document change.
    edits: usize,
    content_clears: u32,
    overlay_clears: u32,
    bg_clears: u32,
    /// Builds of the GPU content layer's scene.
    content_scene_builds: u32,
    /// Builds of the GPU drag static layer's scene (the sheet minus the
    /// moving items).
    drag_scene_builds: u32,
}

impl Counts {
    fn read(app: &Signex) -> Self {
        let slot = app.interaction_state.active_canvas();
        Self {
            edits: engine(app).undo_position(),
            content_clears: slot.content_cache_clears,
            overlay_clears: slot.overlay_cache_clears,
            bg_clears: slot.bg_cache_clears,
            content_scene_builds: slot.content_scene.borrow().builds(),
            drag_scene_builds: slot.drag_layers.borrow().static_scene_builds(),
        }
    }

    fn since(self, before: Self) -> Self {
        Self {
            edits: self.edits - before.edits,
            content_clears: self.content_clears - before.content_clears,
            overlay_clears: self.overlay_clears - before.overlay_clears,
            bg_clears: self.bg_clears - before.bg_clears,
            content_scene_builds: self.content_scene_builds - before.content_scene_builds,
            drag_scene_builds: self.drag_scene_builds - before.drag_scene_builds,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            edits: self.edits + other.edits,
            content_clears: self.content_clears + other.content_clears,
            overlay_clears: self.overlay_clears + other.overlay_clears,
            bg_clears: self.bg_clears + other.bg_clears,
            content_scene_builds: self.content_scene_builds + other.content_scene_builds,
            drag_scene_builds: self.drag_scene_builds + other.drag_scene_builds,
        }
    }

    /// Every counter at most `cap`'s.
    fn within(self, cap: Self) -> bool {
        self.edits <= cap.edits
            && self.content_clears <= cap.content_clears
            && self.overlay_clears <= cap.overlay_clears
            && self.bg_clears <= cap.bg_clears
            && self.content_scene_builds <= cap.content_scene_builds
            && self.drag_scene_builds <= cap.drag_scene_builds
    }
}

const ZERO: Counts = Counts {
    edits: 0,
    content_clears: 0,
    overlay_clears: 0,
    bg_clears: 0,
    content_scene_builds: 0,
    drag_scene_builds: 0,
};

fn sheet_path() -> std::path::PathBuf {
    std::env::temp_dir()
        .join("snx-drag-baseline")
        .join("drag.snxsch")
}

fn engine(app: &Signex) -> &signex_engine::Engine {
    app.document_state
        .engines
        .get(&sheet_path())
        .expect("the sheet's engine")
}

/// An app with one sheet holding the wire (10,10)-(30,10) mm, selected, on
/// the main canvas at 8 px/mm.
fn app_with_selected_wire() -> Signex {
    let mut slot = slot_with_wire();
    let sheet = slot.active_snapshot().expect("snapshot").clone();
    let path = sheet_path();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    let revision = engine.revision();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "drag".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&sheet, revision),
    ));
    *slot.camera_mut() = Camera {
        offset: iced::Point::ORIGIN,
        scale: 8.0,
    };
    slot.selected = vec![SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire)];
    *app.interaction_state.active_canvas_mut() = slot;
    app
}

/// One runtime frame: `event` through the input canvas, its message and the
/// window's `DragMove` through `Signex::update`, then both shader layers draw.
fn frame(app: &mut Signex, state: &mut CanvasState, event: Event, at: iced::Point) {
    let all: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
        .iter()
        .copied()
        .collect();
    let overrides = std::collections::HashMap::new();
    let published = {
        let slot = app.interaction_state.active_canvas();
        let overlay = SchematicCanvas::new(slot, test_prefs(&overrides, &all)).gpu_overlay();
        overlay
            .update(state, &event, BOUNDS, mouse::Cursor::Available(at))
            .and_then(|action| action.into_inner().0)
    };
    for message in published
        .into_iter()
        .chain([Message::Ui(UiMsg::DragMove(at.x, at.y))])
    {
        let _ = app.update(message);
    }
    let slot = app.interaction_state.active_canvas();
    let cursor = mouse::Cursor::Available(at);
    let content = SchematicShaderProgram::new(slot, test_prefs(&overrides, &all));
    let _ = iced::widget::shader::Program::draw(&content, &(), cursor, BOUNDS);
    let drag = SchematicDragShaderProgram::new(slot, test_prefs(&overrides, &all));
    let _ = iced::widget::shader::Program::draw(&drag, &(), cursor, BOUNDS);
}

fn left(pressed: bool) -> Event {
    let button = mouse::Button::Left;
    Event::Mouse(if pressed {
        mouse::Event::ButtonPressed(button)
    } else {
        mouse::Event::ButtonReleased(button)
    })
}

/// What one drag cost: per move step, and at the drop.
struct DragCounts {
    steps: Vec<Counts>,
    drop: Counts,
}

/// Press at world `grab`, move `STEPS` grid pitches right, release.
fn drag_from(app: &mut Signex, grab: (f32, f32)) -> DragCounts {
    let mut state = CanvasState::default();
    let start = screen(app.interaction_state.active_canvas(), grab.0, grab.1);
    frame(app, &mut state, left(true), start);
    let mut steps = Vec::with_capacity(STEPS);
    let mut at = start;
    for i in 1..=STEPS {
        let world_x = grab.0 + STRIDE_MM * i as f32;
        at = screen(app.interaction_state.active_canvas(), world_x, grab.1);
        let before = Counts::read(app);
        let moved = Event::Mouse(mouse::Event::CursorMoved { position: at });
        frame(app, &mut state, moved, at);
        steps.push(Counts::read(app).since(before));
    }
    assert!(
        state.move_dragging,
        "the fixture really drags before the drop"
    );
    let before = Counts::read(app);
    frame(app, &mut state, left(false), at);
    DragCounts {
        steps,
        drop: Counts::read(app).since(before),
    }
}

/// Every move step within `per_step`, and all of them together within
/// `whole_drag` (so the one-off build cannot repeat on every step).
fn assert_drag_within(counts: &DragCounts, per_step: Counts, whole_drag: Counts, what: &str) {
    for (i, step) in counts.steps.iter().enumerate() {
        assert!(
            step.within(per_step),
            "{what} step {i} invalidated more than the baseline: {step:?} > {per_step:?} \
             (ADR-0006: a drag must not over-invalidate the projector)"
        );
    }
    let total = counts.steps.iter().fold(ZERO, |acc, step| acc.plus(*step));
    assert!(
        total.within(whole_drag),
        "{what} steps together invalidated more than the baseline: {total:?} > {whole_drag:?}"
    );
}

// Baselines measured on trunk ecae6fb61. Lower them when a slice does; a
// change that needs them raised is an over-invalidation (ADR-0006).

/// Per move step: no engine edit, no cache clear, no content scene build; at
/// most one drag static scene build.
const STEP_BASELINE: Counts = Counts {
    drag_scene_builds: 1,
    ..ZERO
};

/// All move steps of one drag together: the static scene is built once, when
/// the drag goes live, and reused after.
const WHOLE_DRAG_BASELINE: Counts = Counts {
    drag_scene_builds: 1,
    ..ZERO
};

/// The drop: one engine edit and the refresh it causes.
const DROP: Counts = Counts {
    edits: 1,
    content_clears: 3,
    overlay_clears: 1,
    bg_clears: 1,
    content_scene_builds: 1,
    drag_scene_builds: 0,
};

#[test]
fn an_item_drag_stays_within_the_invalidation_baseline() {
    let mut app = app_with_selected_wire();
    let counts = drag_from(&mut app, (20.0, 10.0));
    assert_drag_within(&counts, STEP_BASELINE, WHOLE_DRAG_BASELINE, "item drag");
    assert_eq!(counts.drop, DROP, "item drag drop");
    let wire = &engine(&app).document().wires[0];
    assert!(
        wire.start.x > 10.0 && wire.end.x > 30.0,
        "the drop moved the wire: {wire:?}"
    );
}

#[test]
fn a_wire_end_drag_stays_within_the_invalidation_baseline() {
    let mut app = app_with_selected_wire();
    let counts = drag_from(&mut app, (30.0, 10.0));
    assert_drag_within(&counts, STEP_BASELINE, WHOLE_DRAG_BASELINE, "wire end drag");
    assert_eq!(counts.drop, DROP, "wire end drag drop");
    let wire = &engine(&app).document().wires[0];
    assert!(
        wire.end.x > 30.0 && (wire.start.x - 10.0).abs() < 1e-9,
        "the drop moved only the end: {wire:?}"
    );
}
