//! What the Active Bar can do: every action of its buttons and dropdown
//! rows, and when one of them is clickable.

use super::is_planned;

/// Whether `action` needs at least one selected item to make sense.
/// Transform / align / distribute family — Altium greys these out when
/// the selection is empty. Net-colour picks are excluded because the
/// NetColor flow is "arm-then-apply", not "act on selection".
pub fn requires_selection(action: &ActiveBarAction) -> bool {
    use ActiveBarAction::*;
    matches!(
        action,
        Drag | DragSelection
            | MoveSelection
            | MoveSelectionXY
            | MoveToFront
            | RotateSelection
            | RotateSelectionCW
            | FlipSelectedX
            | FlipSelectedY
            | BringToFront
            | BringToFrontOf
            | SendToBack
            | SendToBackOf
            | AlignLeft
            | AlignRight
            | AlignTop
            | AlignBottom
            | AlignHorizontalCenters
            | AlignVerticalCenters
            | AlignToGrid
            | DistributeHorizontally
            | DistributeVertically
    )
}

/// Whether `action` only makes sense when at least one net carries a
/// custom colour override. The Clear / Clear-All Net Color actions go
/// here; the seven NetColor pickers and Custom Color stay always-on
/// (they're the "arm" phase that paints colours onto nets).
pub fn requires_net_color(action: &ActiveBarAction) -> bool {
    use ActiveBarAction::*;
    matches!(action, ClearNetColor | ClearAllNetColors)
}

/// What the Directives group button does on a left click before any
/// directive has been used. Must be an enabled action (#1248: it was the
/// planned Parameter Set, leaving the whole button dead).
pub(crate) const DIRECTIVES_DEFAULT_ACTION: ActiveBarAction = ActiveBarAction::PlaceNoERC;

/// Whether `action` is clickable given the current selection and
/// net-colour state.
///
/// #633 — these two facts used to reach here through a pair of
/// thread-locals that `view_bar` set for the duration of one render.
/// Their reset value was `true`, so any path reaching a bar helper
/// outside `view_bar` rendered a fully-enabled bar regardless of what
/// was selected — a fallback that silently produced the wrong UI
/// instead of failing. They are arguments now, so the compiler asks.
pub(crate) fn action_enabled(
    action: &ActiveBarAction,
    has_selection: bool,
    has_net_colors: bool,
) -> bool {
    if is_planned(action) {
        return false;
    }
    if requires_selection(action) && !has_selection {
        return false;
    }
    if requires_net_color(action) && !has_net_colors {
        return false;
    }
    true
}

/// All actions available from Active Bar buttons and dropdown items.
///
/// `PartialEq` so `app::command::active_bar` can look an action up in its
/// id table by value (#271). The ambiguity check in `action_label` only
/// formats `Debug` when that list is non-empty, so a row render does not
/// pay for it today.
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveBarAction {
    // Selection modes
    ToolSelect,
    LassoSelect,
    InsideArea,
    OutsideArea,
    TouchingRectangle,
    TouchingLine,
    SelectAll,
    SelectConnection,
    ToggleSelection,
    // Move/Transform
    Drag,
    /// Interactive click-click move of one picked object (#271).
    Move,
    /// Interactive click-click move of the current selection (#271).
    MoveSelection,
    MoveSelectionXY,
    DragSelection,
    MoveToFront,
    RotateSelection,
    RotateSelectionCW,
    FlipSelectedX,
    FlipSelectedY,
    BringToFront,
    SendToBack,
    BringToFrontOf,
    SendToBackOf,
    // Sheet symbols
    PlaceSheetSymbol,
    PlaceSheetEntry,
    PlaceDeviceSheetSymbol,
    PlaceReuseBlock,
    // Align
    AlignLeft,
    AlignRight,
    AlignHorizontalCenters,
    DistributeHorizontally,
    AlignTop,
    AlignBottom,
    AlignVerticalCenters,
    DistributeVertically,
    AlignToGrid,
    // Wiring
    DrawWire,
    DrawBus,
    PlaceBusEntry,
    PlaceNetLabel,
    PlaceJunction,
    // Power
    PlacePowerGND,
    PlacePowerVCC,
    PlacePowerPlus12,
    PlacePowerPlus5,
    PlacePowerMinus5,
    PlacePowerArrow,
    PlacePowerWave,
    PlacePowerBar,
    PlacePowerCircle,
    PlacePowerSignalGND,
    PlacePowerEarth,
    // Harness
    PlaceSignalHarness,
    PlaceHarnessConnector,
    PlaceHarnessEntry,
    // Port
    PlacePort,
    PlaceOffSheetConnector,
    // Directives
    PlaceParameterSet,
    PlaceNoERC,
    PlaceDiffPair,
    PlaceBlanket,
    PlaceCompileMask,
    // Text
    PlaceTextString,
    PlaceTextFrame,
    PlaceNote,
    // Shapes
    DrawArc,
    DrawFullCircle,
    DrawEllipticalArc,
    DrawEllipse,
    DrawLine,
    DrawRectangle,
    DrawRoundRectangle,
    DrawPolygon,
    DrawBezier,
    PlaceGraphic,
    // Net Color
    NetColorBlue,
    NetColorLightGreen,
    NetColorLightBlue,
    NetColorRed,
    NetColorFuchsia,
    NetColorYellow,
    NetColorDarkGreen,
    /// Open the custom-colour picker modal for net-colour arming.
    NetColorCustom,
    ClearNetColor,
    ClearAllNetColors,
    // Component
    PlaceComponent,
}
