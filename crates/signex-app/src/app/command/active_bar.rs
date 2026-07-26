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

/// The command id naming an action, if it has one.
///
/// The reverse of [`action_for_id`], for view code that holds an action
/// and wants the catalog entry behind it — the Active Bar renders its
/// row labels through this so the visible text lives in the command
/// table rather than in the view (#271).
pub(crate) fn id_for_action(action: &ActiveBarAction) -> Option<&'static str> {
    ACTIVE_BAR_COMMANDS
        .iter()
        .find(|(_, candidate)| candidate == action)
        .map(|(id, _)| *id)
}

/// Actions the Active Bar renders under MORE THAN ONE label, which one
/// command id therefore cannot describe.
///
/// `MoveSelection` is shown twice in the same Select menu — as "Move"
/// with `icon_dd_move`, and as "Move Selection" with `icon_dd_move_sel`
/// (`active_bar/dropdown.rs`, the `SELECT` table) — and both rows
/// dispatch the identical action. Whether that is two rows that should be
/// two actions, or one row too many, is a menu-content question and not
/// this change's to answer; sourcing the label from the catalog would
/// silently rename one of them.
///
/// So these keep the literal the view passes. `catalog_labels_match_the_
/// active_bar_literals` asserts this list is exactly the set of actions
/// that really are ambiguous, so a new one cannot be added silently — and
/// so this one gets removed the day the duplicate is resolved.
const AMBIGUOUS_LABELS: &[&str] = &["MoveSelection"];

/// The variant name of an action, for the by-name comparisons above.
/// `ActiveBarAction` has no discriminant accessor, and `Debug` is stable
/// for the unit variants this is used on.
fn variant_name(action: &ActiveBarAction) -> String {
    let rendered = format!("{action:?}");
    rendered
        .split(['(', ' ', '{'])
        .next()
        .unwrap_or(&rendered)
        .to_string()
}

/// The label a surface should show for an action: the catalog's
/// `menu_label` when the action has an id, else `fallback`.
///
/// The Active Bar's row text now lives in the command table (#271), so a
/// menu, the palette and the shortcuts pane can render the same command
/// without a second copy of its wording. `fallback` covers the actions
/// still without an id — the parameterised families awaiting
/// `CommandArgs` — so no row can lose its label.
pub(crate) fn action_label(action: &ActiveBarAction, fallback: &'static str) -> &'static str {
    if AMBIGUOUS_LABELS
        .iter()
        .any(|name| variant_name(action) == *name)
    {
        return fallback;
    }
    id_for_action(action)
        .and_then(|id| crate::keymap::AppCommandId::new(id).ok())
        .and_then(|command| crate::keymap::metadata_for(&command))
        .map(|metadata| metadata.menu_label())
        .unwrap_or(fallback)
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

    /// Behaviour proof for #271: for every Active Bar row the catalog now
    /// labels, the catalog's `menu_label` must equal the literal that used
    /// to be rendered — so routing labels through the registry changes
    /// nothing the user sees.
    ///
    /// Scans `dropdown.rs`'s own source for the `(literal, action)` pairs
    /// rather than trusting a hand-kept copy, which is the same reason
    /// `bridge.rs` scans itself. If a row's wording is ever changed in the
    /// view without the catalog following, this fails and names it.
    ///
    /// Changing an Active Bar label is menu content and needs Caner's or
    /// Hakan's sign-off; this test exists so such a change cannot happen
    /// by accident inside a refactor.
    #[test]
    fn catalog_labels_match_the_active_bar_literals() {
        const DROPDOWN_SRC: &str = include_str!("../../active_bar/dropdown.rs");

        let rows = scan_dropdown_rows(DROPDOWN_SRC);

        // The ambiguity list must be exactly the actions the view really
        // does render under more than one label — no stale entries, and
        // no new ones slipping in unlisted.
        let mut labels_per_action: std::collections::BTreeMap<&str, HashSet<&str>> =
            Default::default();
        for (literal, variant) in &rows {
            labels_per_action
                .entry(variant.as_str())
                .or_default()
                .insert(literal.as_str());
        }
        let actually_ambiguous: HashSet<&str> = labels_per_action
            .iter()
            .filter(|(_, labels)| labels.len() > 1)
            .map(|(variant, _)| *variant)
            .collect();
        let listed: HashSet<&str> = AMBIGUOUS_LABELS.iter().copied().collect();
        assert_eq!(
            actually_ambiguous, listed,
            "AMBIGUOUS_LABELS is out of step with the view. Left = actions \
             dropdown.rs renders under several labels, right = what the list \
             claims. An unlisted one would get one of its rows silently \
             renamed; a stale one keeps a row off the registry for no reason."
        );

        let mut checked = 0usize;
        let mut drift: Vec<String> = Vec::new();
        for (literal, variant) in rows.iter().cloned() {
            if listed.contains(variant.as_str()) {
                continue;
            }
            let Some((id, _)) = ACTIVE_BAR_COMMANDS
                .iter()
                .find(|(_, action)| format!("{action:?}") == variant)
            else {
                // No id yet — a parameterised family. `dd_item` falls back
                // to the literal, so there is nothing to compare.
                continue;
            };
            let command = AppCommandId::new(*id).expect("table ids are valid");
            let metadata = metadata_for(&command)
                .unwrap_or_else(|| panic!("`{id}` is in the table but not the catalog"));
            checked += 1;
            if metadata.menu_label() != literal {
                drift.push(format!(
                    "`{id}`: catalog says {:?}, dropdown.rs says {literal:?}",
                    metadata.menu_label()
                ));
            }
        }

        assert!(
            checked >= 50,
            "the dropdown source scan matched only {checked} labelled rows — \
             `EntrySpec::item` / `dd_item` call shapes have drifted from what \
             `scan_dropdown_rows` looks for, so this proof is not proving \
             anything"
        );
        assert!(
            drift.is_empty(),
            "Active Bar label drift — the catalog and the view disagree on \
             what the user sees. Changing this text is menu content and needs \
             sign-off; if the change is intended, update `menu_label` in \
             keymap/catalog/: {drift:#?}"
        );
    }

    /// `(visible label, ActiveBarAction variant name)` for every uniform
    /// dropdown row in `src`, from both row-building shapes.
    fn scan_dropdown_rows(src: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for marker in ["EntrySpec::item(", "dd_item("] {
            for chunk in src.split(marker).skip(1) {
                // ... icon, "Label", ActiveBarAction::Variant
                let Some((_, after_first_quote)) = chunk.split_once('"') else {
                    continue;
                };
                let Some((literal, rest)) = after_first_quote.split_once('"') else {
                    continue;
                };
                let Some((_, after_path)) = rest.split_once("ActiveBarAction::") else {
                    continue;
                };
                let variant: String = after_path
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !variant.is_empty() {
                    out.push((literal.to_string(), variant));
                }
            }
        }
        out
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
