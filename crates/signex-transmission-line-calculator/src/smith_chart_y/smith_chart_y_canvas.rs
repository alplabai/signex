use iced::widget::canvas;
use iced::{Rectangle, Renderer, Theme, mouse};

use crate::tool::{SmithChartCanvas, SmithChartMessage};

#[derive(Debug, Clone)]
pub(crate) struct SmithChartYCanvas {
    chart: SmithChartCanvas,
}

impl SmithChartYCanvas {
    pub(crate) fn new(mut chart: SmithChartCanvas) -> Self {
        chart.admittance_chart = true;
        chart.show_admittance = false;
        Self { chart }
    }
}

impl canvas::Program<SmithChartMessage> for SmithChartYCanvas {
    type State = ();

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

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        canvas::Program::mouse_interaction(&self.chart, state, bounds, cursor)
    }
}
