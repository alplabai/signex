//! An unpaused dimension's points belong to the window that clicked them
//! (mockup row 157): another window's form, Space and Tab leave them alone.
//! Each test runs with the holder in main and in an undocked window, on
//! sheets of different page height.

use signex_types::schematic::Point;

use super::dimension_tool_pause_tests::{app_on_sheet_a, click_in, undocked_over};
use super::placement_tests::click;
use crate::app::sch_tables::{AnnotationEdit, Axis};
use crate::app::{Message, Signex, ToolMessage};
use crate::panels::{AnnotationField, AnnotationPanelMsg, PanelContext, PanelMsg};

type WindowId = Option<iced::window::Id>;

const HELD: (f64, f64) = (25.4, 25.4);
const ELSEWHERE: (f64, f64) = (50.8, 50.8);
const TYPED_Y: f64 = 10.0;
const HELD_OVERRIDE: &str = "held";
const FROM_Y: AnnotationField = AnnotationField::DimensionPoint(false, Axis::Y);

/// One point down in `holder`; `other` is the window that does not hold it.
struct Held {
    app: Signex,
    holder: WindowId,
    other: WindowId,
}

impl Held {
    fn new(in_window: bool) -> Self {
        let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
        for (path, paper) in paths.iter().zip(["A4", "A2"]) {
            let engine = app.document_state.engines.get_mut(path).expect("engine");
            let mut sheet = engine.document().clone();
            sheet.paper_size = paper.to_string();
            engine.set_document(sheet);
        }
        let window = undocked_over(&mut app, &paths[1]);
        let _ = app.run_in_window(window, true, |_| iced::Task::none());
        app.sync_annotation_draft();
        let _ = app.run_in_window_document(window, |app| {
            app.sync_annotation_draft();
            iced::Task::none()
        });
        let (holder, other) = if in_window {
            click_in(&mut app, window, HELD);
            (Some(window), None)
        } else {
            click(&mut app, HELD);
            (None, Some(window))
        };
        app.sync_window_panel_ctx_shared();
        Self { app, holder, other }
    }

    fn each(mut check: impl FnMut(Self)) {
        for in_window in [false, true] {
            check(Self::new(in_window));
        }
    }

    fn run_in(&mut self, window: WindowId, act: impl FnOnce(&mut Signex)) {
        match window {
            Some(id) => {
                let _ = self.app.run_in_window_document(id, |app| {
                    act(app);
                    iced::Task::none()
                });
            }
            None => act(&mut self.app),
        }
    }

    fn form_message(&mut self, window: WindowId, msg: AnnotationPanelMsg) {
        self.run_in(window, |app| {
            let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
                PanelMsg::Annotation(msg),
            )));
        });
    }

    fn hold_override(&mut self) {
        let style = &mut self.app.interaction_state.annotation_tool.dimension;
        style.override_text = Some(HELD_OVERRIDE.to_string());
    }

    fn held_override(&self) -> Option<&str> {
        let style = &self.app.interaction_state.annotation_tool.dimension;
        style.override_text.as_deref()
    }

    fn points(&self) -> &[Point] {
        &self.app.interaction_state.annotation_tool.points
    }

    fn ctx(&self, window: WindowId) -> &PanelContext {
        match window {
            Some(id) => &self.app.document_state.window_panel_ctx[&id],
            None => &self.app.document_state.panel_ctx,
        }
    }
}

fn held_point() -> Point {
    Point::new(HELD.0, HELD.1)
}

#[test]
fn a_form_elsewhere_does_not_move_a_held_point() {
    Held::each(|mut held| {
        let other = held.other;
        let typed = AnnotationPanelMsg::FieldTyped(FROM_Y, TYPED_Y.to_string());
        held.form_message(other, typed);
        let edit = AnnotationEdit::DimensionPoint(false, Axis::Y, TYPED_Y);
        held.form_message(other, AnnotationPanelMsg::Picked(edit));
        assert_eq!(held.points(), [held_point()]);
    });
}

#[test]
fn the_holder_edits_its_point_on_its_own_page() {
    Held::each(|mut held| {
        let holder = held.holder;
        let typed = AnnotationPanelMsg::FieldTyped(FROM_Y, TYPED_Y.to_string());
        held.form_message(holder, typed);
        let mut expected = 0.0;
        held.run_in(holder, |app| {
            let engine = app.document_state.active_engine().expect("the sheet");
            expected = engine.document().page_y(TYPED_Y);
        });
        assert_eq!(held.points()[0].y, expected);
    });
}

#[test]
fn a_form_elsewhere_does_not_touch_the_held_override() {
    Held::each(|mut held| {
        held.hold_override();
        let other = held.other;
        let typed = AnnotationPanelMsg::FieldTyped(AnnotationField::OverrideText, "x".to_string());
        held.form_message(other, typed);
        let off = AnnotationEdit::OverrideEnabled(false);
        held.form_message(other, AnnotationPanelMsg::Picked(off));
        assert_eq!(held.held_override(), Some(HELD_OVERRIDE));
    });
}

#[test]
fn a_setting_picked_elsewhere_keeps_the_held_override() {
    Held::each(|mut held| {
        held.hold_override();
        let other = held.other;
        let precision = AnnotationEdit::Precision(1);
        held.form_message(other, AnnotationPanelMsg::Picked(precision));
        let tool = &held.app.interaction_state.annotation_tool;
        assert_eq!(tool.dimension.precision, 1, "the tool's setting applies");
        assert_eq!(held.held_override(), Some(HELD_OVERRIDE));
    });
}

#[test]
fn space_elsewhere_does_not_flip_a_held_dimension() {
    Held::each(|mut held| {
        let (holder, other) = (held.holder, held.other);
        held.run_in(other, |app| {
            assert!(app.flip_dimension_side(), "the key is still answered");
        });
        assert!(!held.app.interaction_state.annotation_tool.dimension.flipped);
        held.run_in(holder, |app| {
            assert!(app.flip_dimension_side());
        });
        assert!(held.app.interaction_state.annotation_tool.dimension.flipped);
    });
}

#[test]
fn tab_elsewhere_takes_no_pause_over_a_held_point() {
    Held::each(|mut held| {
        held.run_in(held.other, |app| {
            let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
        });
        let state = &held.app.interaction_state;
        assert!(state.placement_pause.is_none());
        assert_eq!(held.points(), [held_point()]);
    });
}

#[test]
fn the_form_of_a_dropped_point_goes_blank() {
    Held::each(|mut held| {
        let (first, second) = (held.holder, held.other);
        held.run_in(first, Signex::sync_annotation_draft);
        let form = held.ctx(first).annotation.dimension_tool.as_ref();
        assert!(form.expect("the holder's form").has_from);
        match second {
            Some(window) => click_in(&mut held.app, window, ELSEWHERE),
            None => click(&mut held.app, ELSEWHERE),
        }
        held.app.sync_window_panel_ctx_shared();
        let form = held.ctx(first).annotation.dimension_tool.as_ref();
        let form = form.expect("the first window's form");
        assert!(!form.has_from, "the dropped point is not shown");
    });
}

#[test]
fn the_form_elsewhere_shows_no_held_point_or_override() {
    Held::each(|mut held| {
        held.hold_override();
        let holder = held.holder;
        held.run_in(holder, Signex::sync_annotation_draft);
        held.app.sync_window_panel_ctx_shared();
        let elsewhere = held.ctx(held.other).annotation.dimension_tool.as_ref();
        let elsewhere = elsewhere.expect("the other window's form");
        assert!(!elsewhere.has_from && !elsewhere.has_to);
        assert_eq!(elsewhere.dimension.override_text, None);
        let own = held.ctx(holder).annotation.dimension_tool.as_ref();
        let own = own.expect("the holder's form");
        assert!(own.has_from);
        assert_eq!(own.dimension.from, held_point());
        assert_eq!(own.dimension.override_text.as_deref(), Some(HELD_OVERRIDE));
    });
}
