//! A paused dimension or Bezier belongs to its window (mockup row 157). Each
//! test runs with the owner in main and in an undocked window.

use iced::advanced::shell::{Bus, Waker};
use iced::advanced::widget::operation::{Operation, focusable};
use iced::keyboard::{self, Key, key::Named};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;

use super::dimension_tool_pause_tests::{app_on_sheet_a, click_in, tab_key, undocked_over};
use super::placement_tests::click;
use crate::app::state::WindowKind;
use crate::app::{ContextMenuMsg, InputMsg, Message, Signex, Tool, ToolMessage};
use crate::canvas::{CanvasEvent, CanvasSlot};
use crate::panels::{DIMENSION_OVERRIDE_INPUT, PanelContext, PanelKind};
use signex_types::schematic::SchDrawing;

type WindowId = Option<iced::window::Id>;

const FIRST: (f64, f64) = (2.54, 2.54);
const SECOND: (f64, f64) = (12.7, 2.54);
const ELSEWHERE: (f64, f64) = (25.4, 25.4);
const TOOLS: [Tool; 3] = [Tool::LinearDimension, Tool::AlignedDimension, Tool::Bezier];
const SIZE: iced::Size = iced::Size::new(1600.0, 1000.0);

/// A tool with two points down, paused by Tab in `owner`; `other` is the
/// window that does not own it (`None` is main).
struct Paused {
    app: Signex,
    tool: Tool,
    owner: WindowId,
    other: WindowId,
}

impl Paused {
    fn new(tool: Tool, in_window: bool) -> Self {
        let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
        let window = undocked_over(&mut app, &paths[1]);
        let _ = app.update(Message::Tool(ToolMessage::SelectTool(tool)));
        // Both windows hold a Properties context, as with the dock showing.
        let _ = app.run_in_window(window, true, |_| iced::Task::none());
        if in_window {
            click_in(&mut app, window, FIRST);
            click_in(&mut app, window, SECOND);
            let _ = app.run_in_window_document(window, |app| {
                app.update(Message::Tool(ToolMessage::PrePlacementTab))
            });
        } else {
            click(&mut app, FIRST);
            click(&mut app, SECOND);
            tab_key(&mut app);
        }
        app.sync_window_panel_ctx_shared();
        app.document_state.panel_ctx.has_schematic = true;
        let window_ctx = app.document_state.window_panel_ctx.get_mut(&window);
        window_ctx.expect("the window's context").has_schematic = true;
        let (owner, other) = if in_window {
            (Some(window), None)
        } else {
            (None, Some(window))
        };
        let paused = Self {
            app,
            tool,
            owner,
            other,
        };
        assert!(paused.is_paused(), "{tool:?} paused");
        paused
    }

    fn each(mut check: impl FnMut(Self)) {
        for tool in TOOLS {
            for in_window in [false, true] {
                check(Self::new(tool, in_window));
            }
        }
    }

    fn slot(&self, window: WindowId) -> &CanvasSlot {
        match window {
            Some(id) => &self.app.interaction_state.canvases[&id],
            None => &self.app.interaction_state.canvas,
        }
    }

    fn ctx(&self, window: WindowId) -> &PanelContext {
        match window {
            Some(id) => &self.app.document_state.window_panel_ctx[&id],
            None => &self.app.document_state.panel_ctx,
        }
    }

    fn is_paused(&self) -> bool {
        self.slot(self.owner).placement_paused
            && self.app.interaction_state.paused_annotation_window() == Some(self.owner)
    }

    fn points(&self) -> usize {
        self.app.interaction_state.annotation_tool.points.len()
    }

    /// Finished dimensions and Beziers, on every open sheet.
    fn placed(&self) -> usize {
        let engines = self.app.document_state.engines.values();
        let drawings = engines.flat_map(|engine| engine.document().drawings.iter());
        drawings
            .filter(|d| matches!(d, SchDrawing::Dimension(_) | SchDrawing::Bezier(_)))
            .count()
    }

    /// Clicks after the two held points that finish this tool's object.
    fn clicks_to_finish(&self) -> usize {
        if self.tool == Tool::Bezier { 2 } else { 1 }
    }

    fn double_click_in(&mut self, window: WindowId, (world_x, world_y): (f64, f64)) {
        let event = CanvasEvent::DoubleClicked {
            world_x,
            world_y,
            screen_x: 0.0,
            screen_y: 0.0,
        };
        self.canvas_event_in(window, event);
    }

    fn escape_in(&mut self, window: WindowId) {
        let window = window.or(self.app.ui_state.main_window_id);
        let _ = self
            .app
            .update(Message::Input(InputMsg::EscapePressed { window }));
    }

    fn canvas_event_in(&mut self, window: WindowId, event: CanvasEvent) {
        let message = match window {
            Some(window_id) => Message::CanvasEventInWindow { window_id, event },
            None => Message::CanvasEvent(event),
        };
        let _ = self.app.update(message);
    }

    fn right_click_in(&mut self, window: WindowId) {
        let show = ContextMenuMsg::Show(0.0, 0.0);
        let message = match window {
            Some(id) => ContextMenuMsg::ScopedToWindow(id, Box::new(show)),
            None => show,
        };
        let _ = self.app.update(Message::ContextMenu(message));
    }

    fn pick_tool_in(&mut self, window: WindowId, tool: Tool) {
        let pick = Message::Tool(ToolMessage::SelectTool(tool));
        let _ = match window {
            Some(id) => self
                .app
                .run_in_window_document(id, move |app| app.update(pick)),
            None => self.app.update(pick),
        };
    }

    fn detached_properties(&mut self) -> iced::window::Id {
        let window = iced::window::Id::unique();
        self.app
            .ui_state
            .windows
            .insert(window, WindowKind::DetachedPanel(PanelKind::Properties));
        window
    }

    /// A key in the Override box of `window`, through the real view.
    fn key_in_override(&mut self, window: iced::window::Id, key: Key, text: Option<&str>) {
        let mut renderer = crate::layout_probe::headless_renderer();
        let mut published = Bus::new();
        {
            let view = self.app.view(window);
            let mut ui = UserInterface::build(view, SIZE, Cache::default(), &mut renderer);
            let id = iced::widget::Id::new(DIMENSION_OVERRIDE_INPUT);
            ui.operate(&renderer, &mut focusable::focus::<()>(id));
            let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyJ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: text.map(Into::into),
                repeat: false,
            });
            let _ = ui.update(
                &iced::window::Headless,
                &Waker::noop(),
                std::slice::from_ref(&event),
                iced::mouse::Cursor::Unavailable,
                &mut renderer,
                &mut published,
            );
        }
        for message in published {
            let _ = self.app.update(message);
        }
    }

    fn override_text(&self) -> Option<&str> {
        let tool = &self.app.interaction_state.annotation_tool;
        tool.dimension.override_text.as_deref()
    }
}

/// Counts the widgets that can take the keys.
#[derive(Default)]
struct FocusableCount(usize);

impl Operation for FocusableCount {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: iced::Rectangle,
        _state: &mut dyn focusable::Focusable,
    ) {
        self.0 += 1;
    }
}

/// The boxes of the Properties form that can take the keys.
fn focusable_boxes(ctx: &PanelContext) -> usize {
    let mut renderer = crate::layout_probe::headless_renderer();
    let view = crate::panels::view_panel(PanelKind::Properties, ctx);
    let mut ui = UserInterface::build(view, SIZE, Cache::default(), &mut renderer);
    let mut count = FocusableCount::default();
    ui.operate(&renderer, &mut count);
    count.0
}

#[test]
fn esc_in_another_window_leaves_the_pause_and_its_points() {
    Paused::each(|mut p| {
        p.escape_in(p.other);
        assert!(p.is_paused(), "{:?} owner {:?}", p.tool, p.owner);
        assert_eq!(p.points(), 2, "{:?}", p.tool);
        assert_eq!(p.app.interaction_state.current_tool, p.tool);
    });
}

#[test]
fn esc_in_the_owning_window_resumes_and_keeps_the_tool() {
    Paused::each(|mut p| {
        p.escape_in(p.owner);
        assert!(!p.slot(p.owner).placement_paused, "{:?}", p.tool);
        assert!(p.app.interaction_state.placement_pause.is_none());
        assert_eq!(p.points(), 2, "the clicked points stay");
        assert_eq!(p.app.interaction_state.current_tool, p.tool);
    });
}

#[test]
fn another_canvas_ignores_clicks_double_clicks_and_right_clicks() {
    Paused::each(|mut p| {
        let (world_x, world_y) = ELSEWHERE;
        p.canvas_event_in(p.other, CanvasEvent::Clicked { world_x, world_y });
        p.canvas_event_in(
            p.other,
            CanvasEvent::DoubleClicked {
                world_x,
                world_y,
                screen_x: 0.0,
                screen_y: 0.0,
            },
        );
        p.right_click_in(p.other);
        assert!(p.is_paused(), "{:?} owner {:?}", p.tool, p.owner);
        assert_eq!(p.points(), 2, "{:?}", p.tool);
        assert_eq!(p.app.interaction_state.current_tool, p.tool);
    });
}

#[test]
fn a_double_click_on_the_owners_paused_canvas_feeds_nothing() {
    Paused::each(|mut p| {
        let tool = p.tool;
        p.double_click_in(p.owner, ELSEWHERE);
        assert!(p.is_paused(), "{tool:?} owner {:?}", p.owner);
        assert_eq!(p.points(), 2, "{tool:?}: no point during the pause");
        assert_eq!(p.placed(), 0, "{tool:?}: nothing placed during the pause");
        p.escape_in(p.owner);
        assert!(!p.slot(p.owner).placement_paused, "{tool:?}");
        assert_eq!(p.points(), 2, "{tool:?}: Esc restores the two points");
        for step in 0..p.clicks_to_finish() {
            assert_eq!(p.placed(), 0, "{tool:?} before click {step}");
            let (x, y) = ELSEWHERE;
            let (world_x, world_y) = (x + 2.54 * step as f64, y);
            p.canvas_event_in(p.owner, CanvasEvent::Clicked { world_x, world_y });
        }
        assert_eq!(p.placed(), 1, "{tool:?}: exactly one object");
        assert_eq!(p.points(), 0, "{tool:?}");
    });
}

#[test]
fn a_double_click_after_the_resume_feeds_the_tool_again() {
    Paused::each(|mut p| {
        let tool = p.tool;
        p.escape_in(p.owner);
        p.double_click_in(p.owner, ELSEWHERE);
        let (points, placed) = if tool == Tool::Bezier { (3, 0) } else { (0, 1) };
        assert_eq!(p.points(), points, "{tool:?} owner {:?}", p.owner);
        assert_eq!(p.placed(), placed, "{tool:?} owner {:?}", p.owner);
    });
}

#[test]
fn a_paused_wire_still_answers_a_click_on_another_canvas() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    tab_key(&mut app);
    assert!(app.interaction_state.canvas.placement_paused);
    click_in(&mut app, window, ELSEWHERE);
    assert!(
        app.interaction_state.wire_drawing,
        "the wire tool is as it was"
    );
    assert_eq!(app.interaction_state.wire_window, Some(window));
}

#[test]
fn another_windows_form_is_read_only_and_the_owners_is_not() {
    Paused::each(|mut p| {
        // A form left from an earlier Tab must not stay editable either.
        let form = p.ctx(p.owner).pre_placement.clone();
        match p.other {
            Some(id) => {
                let ctx = p.app.document_state.window_panel_ctx.get_mut(&id);
                ctx.expect("the window's context").pre_placement = form;
            }
            None => p.app.document_state.panel_ctx.pre_placement = form,
        }
        assert!(p.ctx(p.other).annotation.is_tool_paused_elsewhere);
        assert!(!p.ctx(p.owner).annotation.is_tool_paused_elsewhere);
        assert_eq!(focusable_boxes(p.ctx(p.other)), 0, "{:?}", p.tool);
        assert!(focusable_boxes(p.ctx(p.owner)) > 0, "{:?}", p.tool);
    });
}

#[test]
fn the_form_is_editable_again_once_the_pause_ends() {
    Paused::each(|mut p| {
        p.escape_in(p.owner);
        assert!(!p.ctx(p.other).annotation.is_tool_paused_elsewhere);
    });
}

#[test]
fn a_detached_properties_follows_the_owning_sheet() {
    Paused::each(|p| {
        assert_eq!(
            p.app.detached_properties_owner(PanelKind::Properties),
            p.owner
        );
        assert_eq!(p.app.detached_properties_owner(PanelKind::Messages), None);
    });
}

#[test]
fn typing_and_enter_in_a_detached_properties_act_on_the_owners_pause() {
    for in_window in [false, true] {
        let mut p = Paused::new(Tool::LinearDimension, in_window);
        let detached = p.detached_properties();
        p.key_in_override(detached, Key::Character("j".into()), Some("j"));
        assert_eq!(p.override_text(), Some("j"), "owner {:?}", p.owner);
        assert!(p.is_paused(), "typing keeps the pause");
        p.key_in_override(detached, Key::Named(Named::Enter), None);
        assert!(!p.slot(p.owner).placement_paused, "Enter resumes the owner");
        assert_eq!(p.override_text(), Some("j"), "Enter keeps the value");
    }
}

#[test]
fn esc_in_a_detached_properties_resumes_the_owner_without_the_edit() {
    for in_window in [false, true] {
        let mut p = Paused::new(Tool::LinearDimension, in_window);
        let detached = p.detached_properties();
        p.key_in_override(detached, Key::Character("j".into()), Some("j"));
        p.key_in_override(detached, Key::Named(Named::Escape), None);
        assert!(!p.slot(p.owner).placement_paused, "owner {:?}", p.owner);
        assert_eq!(p.override_text(), None, "the edit is dropped");
        assert_eq!(p.points(), 2);
        assert_eq!(p.app.interaction_state.current_tool, Tool::LinearDimension);
    }
}

#[test]
fn picking_another_tool_ends_the_pause_and_unfreezes_the_owner() {
    for (picked_in_owner, picked) in [
        (true, Tool::Wire),
        (false, Tool::Wire),
        (true, Tool::Select),
        (false, Tool::Select),
    ] {
        Paused::each(|mut p| {
            let picker = if picked_in_owner { p.owner } else { p.other };
            p.pick_tool_in(picker, picked);
            let state = &p.app.interaction_state;
            assert!(state.placement_pause.is_none(), "{:?}", p.tool);
            assert_eq!(state.current_tool, picked);
            assert_eq!(p.points(), 0, "the points end with the pause");
            let owner = p.slot(p.owner);
            assert!(!owner.placement_paused, "no frozen canvas or banner");
            assert!(owner.annotation_draft.is_none(), "no frozen preview");
            let ctx = p.ctx(p.owner);
            assert!(ctx.annotation.dimension_tool.is_none(), "no stale form");
            assert!(!ctx.annotation.is_tool_paused_elsewhere);
        });
    }
}

/// Whether Properties fell through to Document Options.
fn shows_document_options(ctx: &PanelContext) -> bool {
    let title = ctx
        .i18n
        .message_plain(signex_i18n::ids::PANELS_PROPERTIES_DOCUMENT_OPTIONS, &[]);
    let view = crate::panels::view_panel(PanelKind::Properties, ctx);
    crate::layout_probe::lay_out_texts(view, SIZE)
        .iter()
        .any(|shown| shown.text.replace(['\u{2068}', '\u{2069}'], "") == *title)
}

#[test]
fn select_from_either_window_takes_the_ended_form_off_properties() {
    for picked_in_owner in [true, false] {
        Paused::each(|mut p| {
            let picker = if picked_in_owner { p.owner } else { p.other };
            p.pick_tool_in(picker, Tool::Select);
            p.app.sync_window_panel_ctx_shared();
            for window in [p.owner, p.other] {
                let ctx = p.ctx(window);
                assert!(ctx.annotation.dimension_tool.is_none(), "{:?}", p.tool);
                assert!(shows_document_options(ctx), "{:?} {window:?}", p.tool);
            }
        });
    }
}

#[test]
fn picking_the_same_tool_keeps_the_pause() {
    Paused::each(|mut p| {
        let tool = p.tool;
        p.pick_tool_in(p.owner, tool);
        assert!(p.is_paused(), "{tool:?}");
        assert_eq!(p.points(), 2);
    });
}
