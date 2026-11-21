//! The Grid and snap drawing does not follow the pointer.
//!
//! Driven as the app runs: the real General tab through `UserInterface`,
//! keeping its cache between frames, with every message applied through
//! `Signex::update`. Calling `canvas::Program::update` alone cannot see a
//! highlight that jumps between fields while the pointer crosses a drawing.

use iced::mouse::Event as MouseEvent;
use iced::{Point, Size, Widget};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_i18n::ids;
use signex_widgets::property_illustration::IllustrationMsg;

use super::super::{PROPERTY_ROW_PAD_X, PanelMsg};
use super::document_options::DocumentOptionsMsg;
use super::general::view_properties_general;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::layout_probe::{find, headless_renderer, lay_out_texts};

/// Wide enough that the drawing's 264-unit view box maps one to one, so a
/// view x is a window x after the row padding.
const WINDOW: Size = Size::new(264.0 + 2.0 * PROPERTY_ROW_PAD_X as f32, 900.0);
/// The second dot column: the walk crosses a dot in every row.
const DOT_COLUMN_X: f32 = 40.0 + PROPERTY_ROW_PAD_X as f32;

struct PanelLoop {
    app: Signex,
    renderer: iced::Renderer,
    cache: Option<Cache>,
    published: Vec<IllustrationMsg>,
}

impl PanelLoop {
    fn new() -> Self {
        let (mut app, _boot) = Signex::new();
        let ctx = &mut app.document_state.panel_ctx;
        ctx.visible_grid_mm = 2.54;
        ctx.grid_size_mm = 1.27;
        ctx.grid_visible = true;
        ctx.snap_enabled = true;
        ctx.snap_hotspots = true;
        Self {
            app,
            renderer: headless_renderer(),
            cache: None,
            published: Vec::new(),
        }
    }

    /// The top of the Units row, the first row under the drawing.
    fn drawing_end(&self) -> f32 {
        let ctx = &self.app.document_state.panel_ctx;
        let units = ctx.i18n.message_plain(ids::PANELS_PROPERTIES_UNITS, &[]);
        let view = view_properties_general(ctx, iced::Color::BLACK).boxed();
        find(&lay_out_texts(view, WINDOW), &units).bounds.y
    }

    /// One frame: the pointer moves to `at`, messages go through `update`.
    fn move_to(&mut self, at: Point) {
        let ctx = &self.app.document_state.panel_ctx;
        let view = view_properties_general(ctx, iced::Color::BLACK).boxed();
        let cache = self.cache.take().unwrap_or_default();
        let mut ui = UserInterface::build(view, WINDOW, cache, &mut self.renderer);
        let mut bus = iced::advanced::shell::Bus::new();
        let waker = iced::advanced::shell::Waker::noop();
        let moved = iced::Event::Mouse(MouseEvent::CursorMoved { position: at });
        let redraw = iced::Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        ));
        let _ = ui.update(
            &iced::window::Headless,
            &waker,
            &[moved, redraw],
            iced::mouse::Cursor::Available(at),
            &mut self.renderer,
            &mut bus,
        );
        self.cache = Some(ui.into_cache());
        let messages: Vec<PanelMsg> = bus.drain().map(|(message, _)| message).collect();
        for message in messages {
            if let PanelMsg::DocumentOptions(DocumentOptionsMsg::Illustration(hover)) = &message {
                self.published.push(hover.clone());
            }
            let _ = self.app.update(Message::Dock(DockMessage::Panel(message)));
        }
    }

    fn is_lit(&self) -> bool {
        let hover = &self.app.document_state.panel_ctx.document_options.hover;
        hover.hovered().is_some()
    }
}

#[test]
fn walking_down_the_grid_drawing_lights_no_field() {
    let mut panel = PanelLoop::new();
    let end = panel.drawing_end().floor() as i32;
    for y in 0..end {
        panel.move_to(Point::new(DOT_COLUMN_X, y as f32));
        assert!(!panel.is_lit(), "a field lit at y = {y}");
    }
    assert!(panel.published.is_empty(), "{:?}", panel.published);
}
