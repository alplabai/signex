//! Pointer steps on the drawn main window, captured from the same widget
//! state. A menu-bar dropdown or an open pick list lives in the widget tree,
//! so the capture keeps that tree from the first step to the draw.

use crate::render;
use iced::advanced::renderer::Style;
use iced::advanced::shell::{Bus, Waker};
use iced::advanced::widget::operation::{Text, TextInput};
use iced::advanced::widget::{Id, Operation};
use iced::mouse::{self, Button, Cursor};
use iced::{Color, Event, Point, Rectangle, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_app::app::{Message, Signex};

/// One pointer gesture, aimed by what the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerStep {
    /// Click the first text reading exactly this.
    Click(&'static str),
    /// Click this many pixels right of the text field holding this value,
    /// where a measurement field's picker sits.
    ClickRightOfInput(&'static str, u32),
    /// Move to this window point. The vendored menu overlay cannot be walked
    /// by a widget operation while open, so a row inside it is aimed by its
    /// place: 28 px rows, 9 px more per separator, the first row at y 49.
    HoverAt(u32, u32),
}

#[derive(Default)]
struct Labels {
    texts: Vec<(String, Rectangle)>,
    inputs: Vec<(String, Rectangle)>,
}

impl<T> Operation<T> for Labels {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        operate(self);
    }

    fn text(&mut self, _id: Option<&Id>, bounds: Rectangle, text: &mut dyn Text) {
        self.texts.push((text.text().into_owned(), bounds));
    }

    fn text_input(&mut self, _id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
        self.inputs.push((state.text().into_owned(), bounds));
    }
}

fn locate<'a, T: 'a>(
    ui: &mut UserInterface<'a, Message, T, iced::Renderer>,
    r: &iced::Renderer,
    step: PointerStep,
) -> Result<Point, String> {
    let mut labels = Labels::default();
    if !matches!(step, PointerStep::HoverAt(..)) {
        ui.operate(r, &mut labels);
    }
    let named = |list: &[(String, Rectangle)], want: &str| {
        list.iter()
            .find(|(text, _)| text.trim() == want)
            .map(|(_, bounds)| *bounds)
            .ok_or_else(|| format!("nothing on screen reads {want:?}"))
    };
    Ok(match step {
        PointerStep::Click(want) => named(&labels.texts, want)?.center(),
        PointerStep::ClickRightOfInput(want, dx) => {
            let field = named(&labels.inputs, want)?;
            Point::new(field.x + field.width + dx as f32, field.center_y())
        }
        PointerStep::HoverAt(x, y) => Point::new(x as f32, y as f32),
    })
}

fn send<'a, T: 'a>(
    ui: &mut UserInterface<'a, Message, T, iced::Renderer>,
    events: &[Event],
    cursor: Cursor,
    r: &mut iced::Renderer,
) {
    let mut published = Bus::new();
    let _ = ui.update(
        &iced::window::Headless,
        &Waker::noop(),
        events,
        cursor,
        r,
        &mut published,
    );
}

fn redraw_event() -> Event {
    Event::Window(iced::window::Event::RedrawRequested(
        std::time::Instant::now(),
    ))
}

/// Run `steps` on the main window and draw it from the widget state they
/// left. Fails when a label is missing or the steps changed no pixel, so a
/// closed menu is never saved under an open menu's name. `height` is the
/// window height: a menu taller than the usual window needs more.
pub fn draw_after(
    r: &mut iced::Renderer,
    app: &Signex,
    steps: &[PointerStep],
    height: u32,
) -> Result<Vec<u8>, String> {
    let window = app.ui_state.main_window_id.ok_or("no main window")?;
    let before = render::draw_window(r, app, window, (render::WIDTH, height));
    let theme = app.theme(window).ok_or("the main window has no theme")?;
    let size = Size::new(render::WIDTH as f32, height as f32);
    let mut cache = Cache::default();
    let mut cursor = Cursor::Unavailable;
    for step in steps {
        let mut ui = UserInterface::build(app.view(window), size, cache, r);
        send(&mut ui, &[redraw_event()], cursor, r);
        let at = locate(&mut ui, r, *step)?;
        cursor = Cursor::Available(at);
        let moved = Event::Mouse(mouse::Event::CursorMoved { position: at });
        match step {
            PointerStep::HoverAt(..) => send(&mut ui, &[moved], cursor, r),
            PointerStep::Click(_) | PointerStep::ClickRightOfInput(..) => send(
                &mut ui,
                &[
                    moved,
                    Event::Mouse(mouse::Event::ButtonPressed(Button::Left)),
                    Event::Mouse(mouse::Event::ButtonReleased(Button::Left)),
                ],
                cursor,
                r,
            ),
        }
        cache = ui.into_cache();
    }
    let mut ui = UserInterface::build(app.view(window), size, cache, r);
    send(&mut ui, &[redraw_event()], cursor, r);
    ui.draw(
        r,
        &theme,
        &Style {
            text_color: Color::WHITE,
        },
        cursor,
    );
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(
        Size::new(render::WIDTH, height),
        iced::advanced::renderer::Scale::default(),
    );
    let iced::Renderer::Primary(wgpu_renderer) = r else {
        return Err("the audit renderer is not the wgpu one".into());
    };
    let pixels = wgpu_renderer.screenshot(&viewport, theme.palette().background.base.color);
    if pixels == before {
        return Err(format!("the pointer steps {steps:?} changed nothing"));
    }
    Ok(pixels)
}
