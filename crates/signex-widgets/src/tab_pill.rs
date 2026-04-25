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

/// Visual state that drives bg + border colour. Built as a struct so
/// callers don't have to care about the exact derivation rules
/// (e.g. drag-tinted active = mix-of-accent-and-fill).
#[derive(Debug, Clone, Copy)]
pub struct TabPillStyle {
    pub fill: Color,
    pub border: Color,
    pub accent: Color,
    pub is_active: bool,
    /// Draw the right edge. Set `false` for every tab except the
    /// last one in a row so adjacent tabs share their L/R borders
    /// (left of tab N+1 acts as the divider). Without this, every
    /// pair of adjacent tabs shows a 2-px-wide black band where
    /// their R+L borders stack.
    pub is_last: bool,
}

const TOP_RADIUS: f32 = 4.0;
const ACCENT_HEIGHT: f32 = 2.0;
const BORDER_WIDTH: f32 = 1.0;

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
        let child = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, limits);
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

        // Bg fill — covers the full widget area. Inactive pills get
        // a flat fill with no borders so the tab bar reads as a
        // bare strip of labels (Altium parity); only the active
        // tab gets the 3-sided border + accent stripe that lifts
        // it out of the strip.
        renderer.fill_quad(
            Quad {
                bounds,
                border: Border {
                    width: 0.0,
                    radius: iced::border::Radius::default()
                        .top_left(TOP_RADIUS)
                        .top_right(TOP_RADIUS),
                    color: Color::TRANSPARENT,
                },
                ..Quad::default()
            },
            Background::Color(self.style.fill),
        );

        // Three-sided border — top edge + left edge + (right edge
        // only on the last tab). Drawn on every tab so each pill
        // is framed (Altium parity); the right edge is suppressed
        // on non-last tabs because adjacent tabs share the divider
        // (left of tab N+1 = right of tab N). The top edge is
        // inset by TOP_RADIUS on each side so the rounded corners
        // show as just bg fill at the curve, with no straight
        // border line cutting across them.
        let border_color = self.style.border;
        let top_inset = TOP_RADIUS;
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: bounds.x + top_inset,
                    y: bounds.y,
                    width: (bounds.width - 2.0 * top_inset).max(0.0),
                    height: BORDER_WIDTH,
                },
                ..Quad::default()
            },
            Background::Color(border_color),
        );
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y: bounds.y + TOP_RADIUS,
                    width: BORDER_WIDTH,
                    height: bounds.height - TOP_RADIUS,
                },
                ..Quad::default()
            },
            Background::Color(border_color),
        );
        if self.style.is_last {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: bounds.x + bounds.width - BORDER_WIDTH,
                        y: bounds.y + TOP_RADIUS,
                        width: BORDER_WIDTH,
                        height: bounds.height - TOP_RADIUS,
                    },
                    ..Quad::default()
                },
                Background::Color(border_color),
            );
        }
        if self.style.is_active {
            // Accent strip — drawn inside the pill at the bottom
            // 2 px on the active tab only. Sits below the borders
            // (renders last) so it visually replaces the strip
            // baseline at the active tab's position. Other tabs let
            // the strip baseline show through.
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y: bounds.y + bounds.height - ACCENT_HEIGHT,
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
