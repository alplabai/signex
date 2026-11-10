//! Custom tab pill widget — a 1-px outlined, half-rounded quad plus a
//! 2-px accent strip for the active marker.
//!
//! Built as a widget so the pill owns its bg fill and can overlap the
//! next pill by one border width (shared divider) and overlay the strip
//! baseline with the accent. The outline uses the fork's per-side
//! `border::Side` API (see `pill_border`).
//!
//! Geometry:
//!
//! ```text
//!  ┌─────────────┐  ← 1 px outline, `border` colour
//!  │             │
//!  │   content   │  ← caller-supplied child (text + icons + ...)
//!  │             │
//!  └─────────────┘  ← bottom edge has NO border
//!  ▓▓▓▓▓▓▓▓▓▓▓▓▓   ← 2 px accent strip when `is_active`
//! ```
//!
//! The widget owns the bg fill so adjacent pills can sit flush with
//! zero spacing and still read as discrete tabs (the bg + side
//! borders provide the divider).

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::renderer::Quad;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Background, Border, Color, Element, Event, Length, Rectangle, Size, Vector, border};

/// Where the accent stripe sits on the pill — `Bottom` for document
/// tabs that hang from above (rounded top corners, accent line at
/// bottom replacing the strip baseline) and `Top` for panel tabs
/// that hang from a strip below (rounded bottom corners, accent line
/// at top replacing the strip top-line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentPosition {
    Bottom,
    Top,
}

/// Visual state that drives bg + border colour. Built as a struct so
/// callers don't have to care about the exact derivation rules
/// (e.g. drag-tinted active = mix-of-accent-and-fill).
#[derive(Debug, Clone, Copy)]
pub struct TabPillStyle {
    pub fill: Color,
    pub border: Color,
    pub accent: Color,
    pub is_active: bool,
    /// `false` for every tab except the last one in a row, so adjacent
    /// tabs share one divider: the pill's right border is pushed
    /// [`BORDER_WIDTH`] under the next tab, whose left border paints
    /// over it. Without this, every pair of adjacent tabs shows a
    /// 2-px-wide band where their R+L borders stack. Only valid for a
    /// row with no spacing between the pills.
    pub is_last: bool,
    pub accent_position: AccentPosition,
}

const TOP_RADIUS: f32 = 4.0;
const ACCENT_HEIGHT: f32 = 2.0;
const BORDER_WIDTH: f32 = 1.0;

mod square;

/// Bounds of the bordered quad for a pill laid out in `bounds`.
///
/// A non-last pill extends one border width to the right so its right
/// border sits exactly under the next pill's left border (which paints
/// later and covers it), leaving a single 1-px divider.
fn pill_quad_bounds(bounds: Rectangle, style: &TabPillStyle) -> Rectangle {
    if style.is_last {
        bounds
    } else {
        Rectangle {
            width: bounds.width + BORDER_WIDTH,
            ..bounds
        }
    }
}

/// Border of the pill quad: 1 px in the `border` colour on every side
/// (the edge opposite the rounded corners merges with the strip
/// baseline, which has the same colour and y), with the two corners that
/// face away from the strip rounded.
///
/// Per-side widths are set explicitly through the fork's `border::Side`
/// so the intent is visible at the call site. A per-side accent edge
/// (width 2, accent colour) was evaluated and rejected: the mitred
/// corner joins of a 1 px side next to a 2 px side anti-alias differently
/// from the flat strip drawn today, so the strip stays a separate quad.
fn pill_border(style: &TabPillStyle) -> Border {
    let radius = match style.accent_position {
        AccentPosition::Bottom => border::Radius::default()
            .top_left(TOP_RADIUS)
            .top_right(TOP_RADIUS),
        AccentPosition::Top => border::Radius::default()
            .bottom_left(TOP_RADIUS)
            .bottom_right(TOP_RADIUS),
    };
    let side = border::Side::default()
        .color(style.border)
        .width(BORDER_WIDTH);
    Border {
        color: style.border,
        width: BORDER_WIDTH,
        radius,
        top: side,
        right: side,
        bottom: side,
        left: side,
    }
}

/// Every quad the pill paints, in paint order.
fn pill_quads(bounds: Rectangle, style: &TabPillStyle) -> Vec<(Quad, Background)> {
    let mut quads = vec![(
        Quad {
            bounds: pill_quad_bounds(bounds, style),
            border: pill_border(style),
            ..Quad::default()
        },
        Background::Color(style.fill),
    )];
    if style.is_active {
        // Accent strip at the edge opposite the rounded corners; it
        // overlays the outline there (and the strip baseline beyond).
        let accent_y = match style.accent_position {
            AccentPosition::Bottom => bounds.y + bounds.height - ACCENT_HEIGHT,
            AccentPosition::Top => bounds.y,
        };
        quads.push((
            Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y: accent_y,
                    width: bounds.width,
                    height: ACCENT_HEIGHT,
                },
                ..Quad::default()
            },
            Background::Color(style.accent),
        ));
    }
    quads
}

pub struct TabPill<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    style: TabPillStyle,
    square: bool,
}

impl<'a, Message, Theme, Renderer> TabPill<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        style: TabPillStyle,
    ) -> Self {
        Self {
            content: content.into(),
            style,
            square: false,
        }
    }

    /// Opt in to the square variant: no rounding, no bottom border, a 2 px
    /// accent on top, and `style.fill` as the (caller-chosen) active fill.
    /// `style.accent_position` is ignored. Off by default.
    pub fn square(mut self, square: bool) -> Self {
        self.square = square;
        self
    }
}

impl<Message, Theme, Renderer> widget::Meta for TabPill<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for TabPill<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.diff(tree);
    }

    fn size(&self) -> Size<Length> {
        let child = self.content.size();
        Size {
            width: child.width,
            height: child.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        // Pass parent limits through to the child unchanged. The
        // accent strip overlays the bottom 2 px IN PLACE — it's a
        // visual element on top of the bg fill, not a separate
        // layout band. Inactive tabs and active tabs occupy the
        // exact same footprint; the only difference is whether
        // the borders + accent are drawn. The pill shares the
        // child's tree (see `tag`/`diff`), so the child's laid-out
        // size is already the pill's size.
        self.content.layout(tree, renderer, limits);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .operate(tree, layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        // Pill quads (bg + outline, then the accent strip on the active
        // tab) come from `pill_quads` so the geometry is testable
        // without a renderer.
        let quads = if self.square {
            square::square_pill_quads(layout.bounds(), &self.style)
        } else {
            pill_quads(layout.bounds(), &self.style)
        };
        for (quad, background) in quads {
            renderer.fill_quad(quad, background);
        }

        // Child content draws last so any text / icons paint over the
        // bg fill we just laid down.
        self.content
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content
            .overlay(tree, layout, renderer, viewport, translation, window)
    }
}

#[cfg(test)]
mod tests;
