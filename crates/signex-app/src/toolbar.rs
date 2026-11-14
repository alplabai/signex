//! Tool-selection messages shared by the canvas and the app update loop.

use crate::app::Tool;

#[derive(Debug, Clone)]
pub enum ToolMessage {
    SelectTool(Tool),
    /// On TAB during placement: commit the ghost at the cursor as a
    /// real object (or pause a footprint-pad placement).
    PrePlacementTab,
    /// Resume placement after TAB paused it — clears `pre_placement`
    /// and `placement_paused`. Wired to the on-canvas "Resume" overlay.
    ResumePlacement,
    /// Step the wire/bus corner mode: 90°, 45° start, 45° end, any angle.
    CycleDrawMode,
    /// The wire/bus corner was asked to flip: horizontal-first becomes
    /// vertical-first and back (#2372).
    CornerFlipRequested,
    /// Cancel an in-progress wire/bus draw and reset to the select tool.
    CancelDrawing,
    /// Backspace/Delete: drop the pending wire vertex while drawing,
    /// otherwise delete the selection.
    RemoveLastVertex,
}
