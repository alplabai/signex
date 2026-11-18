//! Measure Distance on a board (#2403): the first click marks a point (or
//! the object under it), the second reads the distance from it, and the gap
//! edge to edge when both clicks landed on an object. Points snap like the
//! placement tools'. The ruler is drawn on the canvas, the reading goes to
//! the Messages panel; Esc drops the first point, then the tool.

use pcb_geom::Span;
use signex_i18n::{ids, t};
use signex_types::measure::format_length_mm;
use signex_types::pcb::{PcbBoard, Point};

use crate::app::Signex;
use crate::app::state::pcb_query::{BoardMeasure, BoardMeasurement, MeasureMark};
use crate::pcb_canvas::snap_point;

/// What two Measure clicks read on `board`.
pub(crate) fn measurement(
    board: &PcbBoard,
    first: MeasureMark,
    second: MeasureMark,
) -> BoardMeasurement {
    let gap = match (first.item, second.item) {
        (Some(a), Some(b)) if a != b => pcb_geom::edge_gap(board, a, b),
        _ => None,
    };
    BoardMeasurement {
        span: Span::between(first.at, second.at),
        gap,
    }
}

impl Signex {
    /// Arm Measure on the board in view, ending a placement tool, a Move,
    /// paste or part on the cursor and a guide waiting to be placed.
    pub(crate) fn arm_board_measure(&mut self) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        self.end_pcb_carry();
        self.interaction_state.pcb_tool = None;
        self.interaction_state.pcb_guide_place = None;
        self.interaction_state.pcb_query.measure = Some(BoardMeasure {
            path,
            window: self.interaction_state.acting_window,
            first: None,
            cursor: None,
            result: None,
        });
        crate::diagnostics::log_info(t!(self.ui_state.i18n, ids::BOARD_MEASURE_HINT).into_string());
        self.refresh_pcb_highlight();
    }

    /// Measure armed on the board this window shows.
    pub(crate) fn own_board_measure(&self) -> Option<&BoardMeasure> {
        let path = self.active_tab_path();
        let window = self.interaction_state.acting_window;
        self.interaction_state
            .pcb_query
            .measure
            .as_ref()
            .filter(|m| m.is_for(path.as_deref(), window))
    }

    fn own_board_measure_mut(&mut self) -> Option<&mut BoardMeasure> {
        let path = self.active_tab_path();
        let window = self.interaction_state.acting_window;
        self.interaction_state
            .pcb_query
            .measure
            .as_mut()
            .filter(|m| m.is_for(path.as_deref(), window))
    }

    /// Where a Measure click at `at` lands on the board in view.
    fn measure_point(&self, at: Point) -> Option<Point> {
        let board = self.active_pcb()?;
        Some(snap_point(
            board,
            at,
            &self.ui_state.pcb_snap,
            self.pcb_active_layer(),
        ))
    }

    /// The pointer moved: after the first click the ruler follows it.
    pub(crate) fn board_measure_followed(&mut self, at: Point) {
        let waiting = self.own_board_measure().is_some_and(|m| m.first.is_some());
        let Some(at) = self.measure_point(at).filter(|_| waiting) else {
            return;
        };
        if let Some(measure) = self.own_board_measure_mut()
            && measure.cursor != Some(at)
        {
            measure.cursor = Some(at);
            self.refresh_pcb_highlight();
        }
    }

    /// A click on the board while Measure is armed. Whether it was
    /// Measure's.
    pub(crate) fn board_measure_clicked(&mut self, at: Point, tolerance_mm: f64) -> bool {
        let Some(first) = self.own_board_measure().map(|m| m.first) else {
            return false;
        };
        let Some(snapped) = self.measure_point(at) else {
            return false;
        };
        // A designator or comment is not an object to measure to (#2368).
        let item = self
            .with_pcb_selection(|sel, board| sel.part_at(board, at, tolerance_mm))
            .flatten();
        let mark = MeasureMark { at: snapped, item };
        let reading = match first {
            None => None,
            Some(first) => self
                .active_pcb()
                .map(|board| measurement(board, first, mark)),
        };
        if let Some(reading) = &reading {
            self.report_board_measurement(reading);
        }
        if let Some(measure) = self.own_board_measure_mut() {
            if reading.is_some() {
                measure.first = None;
                measure.cursor = None;
                measure.result = reading;
            } else {
                measure.first = Some(mark);
                measure.cursor = Some(snapped);
                measure.result = None;
            }
        }
        self.refresh_pcb_highlight();
        true
    }

    /// Esc: drop the first point, or end Measure when none is set. Whether
    /// Measure took the key.
    pub(crate) fn step_back_board_measure(&mut self) -> bool {
        let Some(measure) = self.own_board_measure_mut() else {
            return false;
        };
        if measure.first.is_some() {
            measure.first = None;
            measure.cursor = None;
        } else {
            self.interaction_state.pcb_query.measure = None;
        }
        self.refresh_pcb_highlight();
        true
    }

    /// The ruler the canvas draws: from the first point to the pointer,
    /// else the last reading (edge to edge when it has one).
    pub(crate) fn board_measure_line(&self) -> Option<(Point, Point)> {
        let measure = self.own_board_measure()?;
        if let (Some(first), Some(cursor)) = (measure.first, measure.cursor) {
            return Some((first.at, cursor));
        }
        let result = measure.result?;
        let span = result.gap.unwrap_or(result.span);
        Some((span.from, span.to))
    }

    /// The reading, in the Messages panel, in the display unit.
    fn report_board_measurement(&self, reading: &BoardMeasurement) {
        let unit = self.ui_state.unit;
        let i18n = &self.ui_state.i18n;
        let length = |mm: f64| format_length_mm(mm, unit);
        let span = &reading.span;
        let line = t!(
            i18n,
            ids::BOARD_MEASURE_DISTANCE,
            "distance" => length(span.distance_mm).as_str(),
            "dx" => length(span.dx_mm).as_str(),
            "dy" => length(span.dy_mm).as_str()
        );
        crate::diagnostics::log_info(line.into_string());
        if let Some(gap) = &reading.gap {
            let line = t!(i18n, ids::BOARD_MEASURE_GAP, "gap" => length(gap.distance_mm).as_str());
            crate::diagnostics::log_info(line.into_string());
        }
    }
}
