//! Esc and Enter in the paused dimension form (mockup row 157), through the
//! real view: Tab's task really runs, keys go to the widgets first, and what
//! no widget captured goes to `update` as the keyboard subscription sends it.

use iced::Rectangle;
use iced::Task;
use iced::advanced::shell::{Bus, Waker};
use iced::advanced::widget::Id;
use iced::advanced::widget::operation::focusable::{self, Focusable};
use iced::advanced::widget::operation::{Operation, Outcome, TextInput};
use iced::futures::{FutureExt as _, StreamExt as _};
use iced::keyboard::{self, Key, key::Named};
use iced_runtime::user_interface::Cache;
use iced_runtime::{Action, UserInterface};
use signex_types::schematic::{SchDimension, SchDrawing};

use super::placement_tests::{app_with_tool, click, doc};
use crate::app::sch_tables::Axis;
use crate::app::state::WindowKind;
use crate::app::{InputMsg, Message, Signex, Tool, ToolMessage};
use crate::panels::{
    AnnotationField, AnnotationPanelMsg, DIMENSION_OVERRIDE_INPUT, PanelKind, PanelMsg,
};

const SIZE: iced::Size = iced::Size::new(1600.0, 1000.0);

/// One window of a running app: its widget state survives between keys.
struct Session {
    app: Signex,
    window: iced::window::Id,
    renderer: iced::Renderer,
    cache: Cache,
    pending: Vec<Task<Message>>,
}

/// Moves the focus to the first text box that is not Override.
#[derive(Default)]
struct FocusAnotherBox {
    is_at_box: bool,
    is_done: bool,
}

impl Operation for FocusAnotherBox {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn text_input(&mut self, id: Option<&Id>, _bounds: Rectangle, _state: &mut dyn TextInput) {
        self.is_at_box = !self.is_done && id != Some(&Id::new(DIMENSION_OVERRIDE_INPUT));
    }

    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if std::mem::take(&mut self.is_at_box) {
            state.focus();
            self.is_done = true;
        } else {
            state.unfocus();
        }
    }
}

/// Run one widget operation to its end, following its chain as the runtime
/// does.
fn run_operation(
    mut operation: Box<dyn Operation>,
    ui: &mut UserInterface<'_, Message, iced::Theme, iced::Renderer>,
    renderer: &iced::Renderer,
) {
    loop {
        ui.operate(renderer, operation.as_mut());
        match operation.finish() {
            Outcome::Chain(next) => operation = next,
            _ => return,
        }
    }
}

/// Run the widget operations `task` asks for.
fn run_operations(
    task: Task<Message>,
    ui: &mut UserInterface<'_, Message, iced::Theme, iced::Renderer>,
    renderer: &iced::Renderer,
) {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    while let Some(Some(action)) = stream.next().now_or_never() {
        if let Action::Widget(operation) = action {
            run_operation(operation, ui, renderer);
        }
    }
}

fn key_event(key: Key, text: Option<&str>) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyJ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: text.map(Into::into),
        repeat: false,
    }
}

impl Session {
    /// The tool armed with two points down and paused by Tab, in `window`.
    fn paused_in(mut app: Signex, window: iced::window::Id) -> Self {
        let _ = app.update(Message::Tool(ToolMessage::SelectTool(
            Tool::LinearDimension,
        )));
        click(&mut app, (0.0, 0.0));
        click(&mut app, (12.7, 0.0));
        let tab = app.update(Message::Tool(ToolMessage::PrePlacementTab));
        // The bare test sheet has no render snapshot to set this from.
        app.document_state.panel_ctx.has_schematic = true;
        Self {
            app,
            window,
            renderer: crate::layout_probe::headless_renderer(),
            cache: Cache::default(),
            pending: vec![tab],
        }
    }

    fn docked() -> Self {
        let mut app = app_with_tool(Tool::LinearDimension);
        let main = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        Self::paused_in(app, main)
    }

    fn in_detached_properties() -> Self {
        let mut app = app_with_tool(Tool::LinearDimension);
        app.ui_state.main_window_id = Some(iced::window::Id::unique());
        let window = iced::window::Id::unique();
        app.ui_state
            .windows
            .insert(window, WindowKind::DetachedPanel(PanelKind::Properties));
        Self::paused_in(app, window)
    }

    /// Press a key; returns whether a widget captured it.
    fn press(&mut self, key: Key, text: Option<&str>) -> bool {
        self.press_after(key, text, |_, _| {})
    }

    /// Press a key after `prepare` ran on the live widgets.
    fn press_after(
        &mut self,
        key: Key,
        text: Option<&str>,
        prepare: impl FnOnce(
            &mut UserInterface<'_, Message, iced::Theme, iced::Renderer>,
            &iced::Renderer,
        ),
    ) -> bool {
        let event = key_event(key, text);
        let mut published = Bus::new();
        let cache = std::mem::take(&mut self.cache);
        let tasks = std::mem::take(&mut self.pending);
        let view = self.app.view(self.window);
        let mut ui = UserInterface::build(view, SIZE, cache, &mut self.renderer);
        for task in tasks {
            run_operations(task, &mut ui, &self.renderer);
        }
        prepare(&mut ui, &self.renderer);
        let (_, statuses) = ui.update(
            &iced::window::Headless,
            &Waker::noop(),
            &[iced::Event::Keyboard(event.clone())],
            iced::mouse::Cursor::Unavailable,
            &mut self.renderer,
            &mut published,
        );
        let captured = statuses[0] == iced::event::Status::Captured;
        self.cache = ui.into_cache();
        for message in published {
            self.pending.push(self.app.update(message));
        }
        if !captured {
            let window = self.window;
            let routed = Message::Input(InputMsg::KeyInput { window, event });
            self.pending.push(self.app.update(routed));
        }
        captured
    }

    fn type_j(&mut self) -> bool {
        self.press(Key::Character("j".into()), Some("j"))
    }

    fn escape(&mut self) -> bool {
        self.press(Key::Named(Named::Escape), None)
    }

    fn enter(&mut self) -> bool {
        self.press(Key::Named(Named::Enter), None)
    }

    fn is_paused(&self) -> bool {
        self.app.interaction_state.active_canvas().placement_paused
    }

    fn override_text(&self) -> Option<&str> {
        let tool = &self.app.interaction_state.annotation_tool;
        tool.dimension.override_text.as_deref()
    }

    fn placed(&self) -> Vec<&SchDimension> {
        doc(&self.app)
            .drawings
            .iter()
            .filter_map(|d| match d {
                SchDrawing::Dimension(d) => Some(d),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn the_task_tab_returns_really_focuses_the_override_box() {
    let mut session = Session::docked();
    assert!(session.type_j(), "the box took the key");
    assert_eq!(session.override_text(), Some("j"));
}

#[test]
fn one_esc_in_the_override_box_drops_the_edits_and_resumes_placing() {
    let mut session = Session::docked();
    session.type_j();
    session.escape();
    assert!(!session.is_paused(), "one Esc resumes");
    assert_eq!(session.override_text(), None, "the edit is dropped");
    assert_eq!(
        session.app.interaction_state.current_tool,
        Tool::LinearDimension,
        "the one Esc does not also leave the tool"
    );
    click(&mut session.app, (5.08, 5.08));
    assert_eq!(session.placed()[0].override_text, None);
}

#[test]
fn one_esc_in_a_detached_properties_window_resumes_placing() {
    let mut session = Session::in_detached_properties();
    assert!(session.type_j(), "the detached box took the key");
    session.escape();
    assert!(!session.is_paused(), "one Esc resumes");
    assert_eq!(session.override_text(), None, "the edit is dropped");
    assert_eq!(
        session.app.interaction_state.current_tool,
        Tool::LinearDimension
    );
}

#[test]
fn esc_in_another_box_of_a_detached_form_keeps_the_pause() {
    let mut session = Session::in_detached_properties();
    // Tab's focus is replaced by the form's first box, a point coordinate.
    let j = Key::Character("j".into());
    let took_it = session.press_after(j, Some("j"), |ui, renderer| {
        run_operation(Box::new(FocusAnotherBox::default()), ui, renderer);
    });
    assert!(took_it, "another box holds the keys");
    assert_eq!(session.override_text(), None, "and it is not Override");
    assert!(!session.escape(), "that box leaves Esc alone");
    assert!(session.is_paused(), "only the Override box resumes");
}

#[test]
fn esc_with_the_override_box_not_focused_is_left_to_the_app() {
    let mut session = Session::docked();
    let captured = session.press_after(Key::Named(Named::Escape), None, |ui, renderer| {
        ui.operate(renderer, &mut focusable::unfocus::<()>());
    });
    assert!(!captured, "the box answers only while it has the keys");
    assert!(!session.is_paused(), "the usual Esc of a paused tool");
}

#[test]
fn enter_in_the_override_box_commits_once_and_resumes_placing() {
    let mut session = Session::docked();
    session.type_j();
    assert!(session.enter(), "the box took Enter");
    assert!(!session.is_paused(), "Enter resumes");
    assert_eq!(session.override_text(), Some("j"), "the value is kept");

    click(&mut session.app, (5.08, 5.08));
    click(&mut session.app, (0.0, 0.0));
    click(&mut session.app, (12.7, 2.54));
    click(&mut session.app, (5.08, 5.08));
    let placed = session.placed();
    assert_eq!(placed[0].override_text.as_deref(), Some("j"));
    assert_eq!(placed[1].override_text, None, "the override is one-shot");
}

#[test]
fn enter_on_a_point_that_does_not_read_keeps_the_pause() {
    let mut session = Session::docked();
    let field = AnnotationField::DimensionPoint(false, Axis::X);
    for msg in [
        AnnotationPanelMsg::FieldTyped(field, "abc".into()),
        AnnotationPanelMsg::FieldSubmitted(field),
    ] {
        let _ = session
            .app
            .update(Message::Dock(crate::dock::DockMessage::Panel(
                PanelMsg::Annotation(msg),
            )));
    }
    assert!(session.is_paused(), "nothing was committed");
}

fn type_in_from_x(session: &mut Session, typed: &str) -> AnnotationField {
    let field = AnnotationField::DimensionPoint(false, Axis::X);
    let typed = AnnotationPanelMsg::FieldTyped(field, typed.into());
    let _ = session
        .app
        .update(Message::Dock(crate::dock::DockMessage::Panel(
            PanelMsg::Annotation(typed),
        )));
    field
}

/// A dialog open in the main window is not on screen in the detached one.
#[test]
fn esc_in_a_detached_override_box_leaves_a_main_window_dialog_open() {
    let mut session = Session::in_detached_properties();
    session.app.ui_state.preferences_open = true;
    assert!(session.type_j(), "the detached box took the key");
    assert!(session.escape(), "the box answered the Esc");
    assert!(session.app.ui_state.preferences_open, "the dialog stays");
    assert!(!session.is_paused(), "the one Esc resumes");
    assert_eq!(session.override_text(), None, "the edit is dropped");
    assert_eq!(
        session.app.interaction_state.current_tool,
        Tool::LinearDimension
    );
}

#[test]
fn enter_in_override_keeps_the_pause_while_another_box_does_not_read() {
    let mut session = Session::docked();
    let field = type_in_from_x(&mut session, "abc");
    assert!(session.enter(), "the Override box took Enter");
    assert!(session.is_paused(), "the refused point holds the pause");
    let draft = &session.app.document_state.panel_ctx.annotation.draft;
    assert!(
        matches!(draft, Some((id, f, text)) if id.is_nil() && *f == field && text == "abc"),
        "the refused text stays in its box: {draft:?}"
    );
}

#[test]
fn enter_in_override_resumes_when_another_box_reads() {
    let mut session = Session::docked();
    type_in_from_x(&mut session, "2.54");
    assert!(session.enter(), "the Override box took Enter");
    assert!(!session.is_paused(), "a readable point does not hold it");
}

#[test]
fn after_enter_the_override_box_lets_go_of_the_keys() {
    let mut session = Session::docked();
    session.type_j();
    session.enter();
    assert!(!session.type_j(), "the next key reaches the app");
}

#[test]
fn after_esc_the_override_box_lets_go_of_the_keys() {
    let mut session = Session::docked();
    session.type_j();
    session.escape();
    assert!(!session.type_j(), "the next key reaches the app");
}

#[test]
fn after_esc_in_a_detached_window_the_box_lets_go_of_the_keys() {
    let mut session = Session::in_detached_properties();
    session.type_j();
    session.escape();
    assert!(!session.type_j(), "the next key reaches the app");
}
