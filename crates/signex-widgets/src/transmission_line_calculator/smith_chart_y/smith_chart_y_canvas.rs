use iced::widget::canvas;
use iced::{Event, Rectangle, Renderer, Theme, mouse};

use crate::transmission_line_calculator::tool::smith_view_navigation::SmithViewNavigationState;
use crate::transmission_line_calculator::tool::{SmithChartCanvas, SmithChartMessage};

/// Wraps the 2D canvas to render the same data in the admittance plane.
#[derive(Debug, Clone)]
pub(crate) struct SmithChartYCanvas {
    chart: SmithChartCanvas,
}

impl SmithChartYCanvas {
    /// Creates an admittance-chart canvas from a 2D impedance-chart canvas.
    pub(crate) fn new(mut chart: SmithChartCanvas) -> Self {
        chart.admittance_chart = true;
        chart.show_admittance = false;
        Self { chart }
    }
}

impl canvas::Program<SmithChartMessage> for SmithChartYCanvas {
    /// Defines the associated `State` type for this implementation.
    type State = SmithViewNavigationState;

    /// Handles an input event and returns the resulting action, if any.
    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<SmithChartMessage>> {
        canvas::Program::update(&self.chart, state, event, bounds, cursor)
    }

    /// Renders the current data into the target drawing surface.
    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        canvas::Program::draw(&self.chart, state, renderer, theme, bounds, cursor)
    }

    /// Returns the mouse interaction appropriate for the current pointer state.
    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        canvas::Program::mouse_interaction(&self.chart, state, bounds, cursor)
    }
}
