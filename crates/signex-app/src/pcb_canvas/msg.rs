//! What the PCB canvas reports to the app (#2191). Points are board mm.

use pcb_geom::ClickMode;
use signex_types::pcb::Point;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PcbCanvasMsg {
    /// The pointer moved over the canvas. `zoom_pct` feeds the status
    /// readout; `tolerance_mm` is the hover reach at the current zoom.
    PointerMoved {
        at: Point,
        zoom_pct: f64,
        tolerance_mm: f64,
    },
    /// The left button was pressed and released in place.
    Clicked {
        at: Point,
        tolerance_mm: f64,
        mode: ClickMode,
    },
    /// The left button was released after dragging a box. Left to right
    /// is a window, right to left a crossing.
    BoxSelected {
        from: Point,
        to: Point,
        mode: ClickMode,
    },
    /// A press on a selected footprint is being dragged from `from` to
    /// `to` (#2240). The board is untouched until the drop.
    FootprintDragged { from: Point, to: Point },
    /// That drag was released at `to`: the footprints move as one step.
    FootprintDropped { from: Point, to: Point },
    /// The pointer left the canvas mid-drag: nothing moves.
    FootprintDragCancelled,
}
