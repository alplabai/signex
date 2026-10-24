//! Custom tab pill widget — three-sided border (top + sides) plus a
//! 2-px accent strip below for the active marker.
//!
//! Built as a real `iced::Widget` because iced 0.14's standard `Border`
//! is uniform on all four sides — we can't draw "top + sides only" via
//! the built-in container style. Trying to fake it with a stacked
//! accent-bg outer + rounded-top inner caused the accent colour to
//! bleed through the rounded corners; the widget below paints only the
//! three edges + accent strip directly via `renderer.fill_quad`.
//!
//! Geometry:
//!
//! ```text
//!  ┌─────────────┐  ← top + side borders (1 px, `border` colour)
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

use iced::advanced::layout::{Limits, Node};
use iced::advanced::renderer::Quad;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Background, Border, Color, Element, Event, Length, Rectangle, Size, Vector};

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

pub struct TabPill<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    style: TabPillStyle,
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
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for TabPill<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        let child = self.content.as_widget().size();
        Size {
            width: child.width,
            height: child.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        // Pass parent limits through to the child unchanged. The
        // accent strip overlays the bottom 2 px IN PLACE — it's a
        // visual element on top of the bg fill, not a separate
        // layout band. Inactive tabs and active tabs occupy the
        // exact same footprint; the only difference is whether
        // the borders + accent are drawn.
        let child = self.content.as_widget_mut().layout(tree, renderer, limits);
        let total_size = child.size();
        Node::with_children(total_size, vec![child])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(child_layout) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, child_layout, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(child_layout) = layout.children().next() {
            self.content.as_widget_mut().update(
                tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map(|child_layout| {
                self.content.as_widget().mouse_interaction(
                    tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        // Bg fill + bordered outline with per-corner radius via
        // iced's Border. Document tabs round the TOP corners (hang
        // from above); panel tabs round the BOTTOM corners (hang
        // from below). Either way the opposite edge merges with
        // the strip's baseline (same colour, same y) so there's
        // no visible double-line at the join.
        let radius = match self.style.accent_position {
            AccentPosition::Bottom => iced::border::Radius::default()
                .top_left(TOP_RADIUS)
                .top_right(TOP_RADIUS),
            AccentPosition::Top => iced::border::Radius::default()
                .bottom_left(TOP_RADIUS)
                .bottom_right(TOP_RADIUS),
        };
        renderer.fill_quad(
            Quad {
                bounds: pill_quad_bounds(bounds, &self.style),
                border: Border {
                    width: BORDER_WIDTH,
                    radius,
                    color: self.style.border,
                },
                ..Quad::default()
            },
            Background::Color(self.style.fill),
        );
        if self.style.is_active {
            // Accent strip — drawn inside the pill at the edge
            // opposite the rounded corners on the active tab only.
            // Sits over the bordered outline so it visually
            // replaces the strip baseline at the active tab's
            // position. Inactive tabs let the strip baseline show
            // through.
            let accent_y = match self.style.accent_position {
                AccentPosition::Bottom => bounds.y + bounds.height - ACCENT_HEIGHT,
                AccentPosition::Top => bounds.y,
            };
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y: accent_y,
                        width: bounds.width,
                        height: ACCENT_HEIGHT,
                    },
                    ..Quad::default()
                },
                Background::Color(self.style.accent),
            );
        }

        // Child content draws last so any text / icons paint over the
        // bg fill we just laid down.
        if let Some(child_layout) = layout.children().next() {
            self.content.as_widget().draw(
                tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let mut children = layout.children();
        let child_layout = children.next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, child_layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<TabPill<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(w: TabPill<'a, Message, Theme, Renderer>) -> Self {
        Self::new(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(is_last: bool) -> TabPillStyle {
        TabPillStyle {
            fill: Color::BLACK,
            border: Color::WHITE,
            accent: Color::WHITE,
            is_active: false,
            is_last,
            accent_position: AccentPosition::Bottom,
        }
    }

    fn bounds() -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 0.0,
            width: 80.0,
            height: 24.0,
        }
    }

    #[test]
    fn last_tab_quad_matches_its_layout_bounds() {
        assert_eq!(pill_quad_bounds(bounds(), &style(true)), bounds());
    }

    #[test]
    fn non_last_tab_right_border_lands_under_the_next_tab_left_border() {
        let quad = pill_quad_bounds(bounds(), &style(false));
        let next_left_border_x = bounds().x + bounds().width;
        // The quad's right border spans [right - BORDER_WIDTH, right]; it must
        // be exactly the next tab's left border column, not a second column.
        assert_eq!(quad.x + quad.width - BORDER_WIDTH, next_left_border_x);
        assert_eq!(quad.x, bounds().x);
    }
}
