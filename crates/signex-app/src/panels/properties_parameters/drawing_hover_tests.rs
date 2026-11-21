//! The Grid and snap drawing holds one steady highlight under the pointer.
//!
//! Driven as the app runs: the real General tab through `UserInterface`,
//! keeping its cache between frames, with every message applied through
//! `Signex::update`. Calling `canvas::Program::update` alone cannot see this
//! bug, which lives in the hit regions the pointer crosses.
//!
//! Before the fix each dot was its own small target with a dead gap around
//! it, and the labels touched the ticks, so a walk down the drawing lit and
//! unlit Visible Grid once per dot row and a one-pixel jitter on any edge
//! flipped the highlight. Both assertions below fail on that code.

use iced::mouse::Event as MouseEvent;
use iced::{Point, Size, Widget};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_i18n::ids;
use signex_widgets::property_illustration::{FieldId, IllustrationMsg};

use super::super::{PROPERTY_ROW_PAD_X, PanelMsg};
use super::document_options::DocumentOptionsMsg;
use super::general::view_properties_general;
use super::grid_illustration::{SNAP_GRID, VISIBLE_GRID};
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

    fn lit(&self) -> Option<FieldId> {
        let hover = &self.app.document_state.panel_ctx.document_options.hover;
        hover.hovered().cloned()
    }
}

#[test]
fn walking_down_the_grid_drawing_lights_each_callout_once_and_holds_it() {
    let mut panel = PanelLoop::new();
    let end = panel.drawing_end().floor() as i32;
    let mut lit = None;
    let mut sequence = Vec::new();
    for y in 0..end {
        let at = Point::new(DOT_COLUMN_X, y as f32);
        panel.move_to(at);
        let now = panel.lit();
        if now == lit {
            continue;
        }
        if let Some(field) = &now {
            sequence.push(field.clone());
        }
        lit = now;
        // A one-pixel jitter over the edge just crossed changes nothing.
        let before = panel.published.len();
        for jitter in [y - 1, y, y - 1, y, y - 1, y] {
            panel.move_to(Point::new(DOT_COLUMN_X, jitter as f32));
            assert_eq!(panel.lit(), lit, "the highlight flipped at y = {jitter}");
        }
        assert_eq!(panel.published.len(), before, "jitter at y = {y} published");
    }
    assert_eq!(
        sequence,
        vec![VISIBLE_GRID, SNAP_GRID],
        "each callout lights once, top to bottom"
    );
    assert_eq!(
        panel.published.len(),
        4,
        "one enter and one leave per callout: {:?}",
        panel.published
    );
}
