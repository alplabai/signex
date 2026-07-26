//! Stable command ids for the schematic Active Bar's actions.
//!
//! One table rather than 59 match arms in [`super::bridge`], so an id and
//! the action it names sit on the same line, and a test can check the
//! pairing in both directions.
//!
//! **Not every `ActiveBarAction` is here.** The parameterised families are
//! deliberately absent — each enumerates a value domain and wants one
//! command plus an argument rather than N frozen ids, which needs the
//! `CommandArgs` boundary (#367 / #366):
//!   - `ActiveBarAction::InsideArea`
//!   - `ActiveBarAction::OutsideArea`
//!   - `ActiveBarAction::TouchingRectangle`
//!   - `ActiveBarAction::TouchingLine`
//!   - `ActiveBarAction::PlacePowerGND`
//!   - `ActiveBarAction::PlacePowerVCC`
//!   - `ActiveBarAction::PlacePowerPlus12`
//!   - `ActiveBarAction::PlacePowerPlus5`
//!   - `ActiveBarAction::PlacePowerMinus5`
//!   - `ActiveBarAction::PlacePowerArrow`
//!   - `ActiveBarAction::PlacePowerWave`
//!   - `ActiveBarAction::PlacePowerBar`
//!   - `ActiveBarAction::PlacePowerCircle`
//!   - `ActiveBarAction::PlacePowerSignalGND`
//!   - `ActiveBarAction::PlacePowerEarth`
//!   - `ActiveBarAction::NetColorBlue`
//!   - `ActiveBarAction::NetColorLightGreen`
//!   - `ActiveBarAction::NetColorLightBlue`
//!   - `ActiveBarAction::NetColorRed`
//!   - `ActiveBarAction::NetColorFuchsia`
//!   - `ActiveBarAction::NetColorYellow`
//!   - `ActiveBarAction::NetColorDarkGreen`
//!   - `ActiveBarAction::NetColorCustom`
//!
//! Two more are absent because [`super::bridge`] already maps their ids to
//! a different message: `select_all` goes through `SelectionRequest` and
//! `place_net_label` through the tool picker. Listing them here would give
//! one id two meanings.
//!
//! See `docs/audit/command-registry-action-surface-2026-07-25.md` §4.

use crate::active_bar::ActiveBarAction;

/// `(command id, action)` for every Active Bar action with a stable id.
/// Sorted by id; the lookups are linear scans over ~59 entries, which
/// is nothing next to a keystroke.
pub(crate) const ACTIVE_BAR_COMMANDS: &[(&str, ActiveBarAction)] = &[
    ("align_bottom", ActiveBarAction::AlignBottom),
    (
        "align_horizontal_centers",
        ActiveBarAction::AlignHorizontalCenters,
    ),
    ("align_left", ActiveBarAction::AlignLeft),
    ("align_right", ActiveBarAction::AlignRight),
    ("align_to_grid", ActiveBarAction::AlignToGrid),
    ("align_top", ActiveBarAction::AlignTop),
    (
        "align_vertical_centers",
        ActiveBarAction::AlignVerticalCenters,
    ),
    ("bring_to_front", ActiveBarAction::BringToFront),
    ("bring_to_front_of", ActiveBarAction::BringToFrontOf),
    ("clear_all_net_colors", ActiveBarAction::ClearAllNetColors),
    ("clear_net_color", ActiveBarAction::ClearNetColor),
    (
        "distribute_horizontally",
        ActiveBarAction::DistributeHorizontally,
    ),
    (
        "distribute_vertically",
        ActiveBarAction::DistributeVertically,
    ),
    ("drag", ActiveBarAction::Drag),
    ("drag_selection", ActiveBarAction::DragSelection),
    ("draw_arc", ActiveBarAction::DrawArc),
    ("draw_bezier", ActiveBarAction::DrawBezier),
    ("draw_bus", ActiveBarAction::DrawBus),
    ("draw_ellipse", ActiveBarAction::DrawEllipse),
    ("draw_elliptical_arc", ActiveBarAction::DrawEllipticalArc),
    ("draw_full_circle", ActiveBarAction::DrawFullCircle),
    ("draw_line", ActiveBarAction::DrawLine),
    ("draw_polygon", ActiveBarAction::DrawPolygon),
    ("draw_rectangle", ActiveBarAction::DrawRectangle),
    ("draw_round_rectangle", ActiveBarAction::DrawRoundRectangle),
    ("draw_wire", ActiveBarAction::DrawWire),
    ("flip_selected_x", ActiveBarAction::FlipSelectedX),
    ("flip_selected_y", ActiveBarAction::FlipSelectedY),
    ("lasso_select", ActiveBarAction::LassoSelect),
    ("move_selection", ActiveBarAction::MoveSelection),
    ("move_selection_xy", ActiveBarAction::MoveSelectionXY),
    ("move_to_front", ActiveBarAction::MoveToFront),
    ("place_blanket", ActiveBarAction::PlaceBlanket),
    ("place_bus_entry", ActiveBarAction::PlaceBusEntry),
    ("place_compile_mask", ActiveBarAction::PlaceCompileMask),
    ("place_component", ActiveBarAction::PlaceComponent),
    (
        "place_device_sheet_symbol",
        ActiveBarAction::PlaceDeviceSheetSymbol,
    ),
    ("place_diff_pair", ActiveBarAction::PlaceDiffPair),
    ("place_graphic", ActiveBarAction::PlaceGraphic),
    (
        "place_harness_connector",
        ActiveBarAction::PlaceHarnessConnector,
    ),
    ("place_harness_entry", ActiveBarAction::PlaceHarnessEntry),
    ("place_no_erc", ActiveBarAction::PlaceNoERC),
    ("place_note", ActiveBarAction::PlaceNote),
    (
        "place_off_sheet_connector",
        ActiveBarAction::PlaceOffSheetConnector,
    ),
    ("place_parameter_set", ActiveBarAction::PlaceParameterSet),
    ("place_port", ActiveBarAction::PlacePort),
    ("place_reuse_block", ActiveBarAction::PlaceReuseBlock),
    ("place_sheet_entry", ActiveBarAction::PlaceSheetEntry),
    ("place_sheet_symbol", ActiveBarAction::PlaceSheetSymbol),
    ("place_signal_harness", ActiveBarAction::PlaceSignalHarness),
    ("place_text_frame", ActiveBarAction::PlaceTextFrame),
    ("place_text_string", ActiveBarAction::PlaceTextString),
    ("rotate_selection", ActiveBarAction::RotateSelection),
    ("rotate_selection_cw", ActiveBarAction::RotateSelectionCW),
    ("select_connection", ActiveBarAction::SelectConnection),
    ("send_to_back", ActiveBarAction::SendToBack),
    ("send_to_back_of", ActiveBarAction::SendToBackOf),
    ("toggle_selection", ActiveBarAction::ToggleSelection),
    ("tool_select", ActiveBarAction::ToolSelect),
];

/// The action a command id names, if it is an Active Bar command.
///
/// Clones rather than copies: `ActiveBarAction` is not `Copy`, and a
/// clone here is a keystroke-scale cost.
pub(crate) fn action_for_id(id: &str) -> Option<ActiveBarAction> {
    ACTIVE_BAR_COMMANDS
        .iter()
        .find(|(command, _)| *command == id)
        .map(|(_, action)| action.clone())
}

/// Every id this table carries. The bridge's coverage guard folds these
/// into the set of ids `core_to_message` can resolve.
///
/// Test-gated because the guard is its only caller today. It comes out
/// of `cfg(test)` the moment a production consumer needs it — the
/// command palette listing Active Bar commands (#366) is the obvious one.
#[cfg(test)]
pub(crate) fn command_ids() -> impl Iterator<Item = &'static str> {
    ACTIVE_BAR_COMMANDS.iter().map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::keymap::{AppCommandId, metadata_for};

    /// Actions deliberately left without an id until `CommandArgs` exists.
    const PARAMETERISED: &[ActiveBarAction] = &[
        ActiveBarAction::InsideArea,
        ActiveBarAction::OutsideArea,
        ActiveBarAction::TouchingRectangle,
        ActiveBarAction::TouchingLine,
        ActiveBarAction::PlacePowerGND,
        ActiveBarAction::PlacePowerVCC,
        ActiveBarAction::PlacePowerPlus12,
        ActiveBarAction::PlacePowerPlus5,
        ActiveBarAction::PlacePowerMinus5,
        ActiveBarAction::PlacePowerArrow,
        ActiveBarAction::PlacePowerWave,
        ActiveBarAction::PlacePowerBar,
        ActiveBarAction::PlacePowerCircle,
        ActiveBarAction::PlacePowerSignalGND,
        ActiveBarAction::PlacePowerEarth,
        ActiveBarAction::NetColorBlue,
        ActiveBarAction::NetColorLightGreen,
        ActiveBarAction::NetColorLightBlue,
        ActiveBarAction::NetColorRed,
        ActiveBarAction::NetColorFuchsia,
        ActiveBarAction::NetColorYellow,
        ActiveBarAction::NetColorDarkGreen,
        ActiveBarAction::NetColorCustom,
    ];

    /// Actions whose id maps to a DIFFERENT message in `bridge.rs`, so
    /// they must not appear in this table.
    const MAPPED_ELSEWHERE: &[ActiveBarAction] =
        &[ActiveBarAction::PlaceNetLabel, ActiveBarAction::SelectAll];

    /// Every id in the table must resolve in the command catalog —
    /// otherwise it is a name the Keyboard Shortcuts pane cannot label and
    /// a profile cannot meaningfully bind.
    #[test]
    fn every_active_bar_command_id_is_in_the_catalog() {
        let orphans: Vec<&str> = command_ids()
            .filter(|id| {
                AppCommandId::new(*id)
                    .ok()
                    .and_then(|command| metadata_for(&command))
                    .is_none()
            })
            .collect();
        assert!(
            orphans.is_empty(),
            "Active Bar command ids with no CommandMetadata entry — add them \
             to keymap/catalog/active_bar.rs: {orphans:?}"
        );
    }

    /// Ids must be unique, and no action may be listed twice.
    #[test]
    fn the_table_has_no_duplicates() {
        let ids: HashSet<&str> = command_ids().collect();
        assert_eq!(
            ids.len(),
            ACTIVE_BAR_COMMANDS.len(),
            "duplicate command id in ACTIVE_BAR_COMMANDS"
        );
        let actions: HashSet<String> = ACTIVE_BAR_COMMANDS
            .iter()
            .map(|(_, action)| format!("{action:?}"))
            .collect();
        assert_eq!(
            actions.len(),
            ACTIVE_BAR_COMMANDS.len(),
            "one ActiveBarAction is listed under two ids"
        );
    }

    /// Round-trip: the lookup finds every id the table carries, and finds
    /// nothing for an id it does not.
    #[test]
    fn action_lookup_round_trips() {
        for (id, action) in ACTIVE_BAR_COMMANDS {
            assert_eq!(
                action_for_id(id).map(|found| format!("{found:?}")),
                Some(format!("{action:?}")),
                "`{id}` did not round-trip"
            );
        }
        assert!(action_for_id("definitely_not_a_command").is_none());
    }

    /// Ratchet in the other direction: every `ActiveBarAction` must have an
    /// id, be a parameterised family awaiting `CommandArgs`, or be mapped
    /// elsewhere. Adding a variant without deciding which fails here rather
    /// than silently leaving it unaddressable.
    ///
    /// Counted rather than matched exhaustively because `ActiveBarAction`
    /// is not `Hash` and the crate has no variant iterator; the total is
    /// asserted explicitly so a new variant cannot slip past.
    #[test]
    fn every_active_bar_action_is_accounted_for() {
        let accounted = ACTIVE_BAR_COMMANDS.len() + PARAMETERISED.len() + MAPPED_ELSEWHERE.len();
        assert_eq!(
            accounted,
            84,
            "ActiveBarAction accounted for {accounted} actions but held 84 \
             when this table was written ({} with ids, {} parameterised, {} \
             mapped elsewhere) — a new variant needs an id here, a slot in \
             PARAMETERISED, or a slot in MAPPED_ELSEWHERE",
            ACTIVE_BAR_COMMANDS.len(),
            PARAMETERISED.len(),
            MAPPED_ELSEWHERE.len()
        );
    }
}
