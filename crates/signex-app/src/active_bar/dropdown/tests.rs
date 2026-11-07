//! Data-to-view tests (iced-rust skill §10): assert on the pure
//! entry builders — no GPU, no window, no widget tree.
use super::*;

const TID: ThemeId = ThemeId::GraphiteDark;

/// #1248 - a fresh session's Directives button must be clickable.
#[test]
fn default_directives_button_is_enabled() {
    use super::super::{DIRECTIVES_DEFAULT_ACTION, is_planned};
    assert!(!is_planned(&DIRECTIVES_DEFAULT_ACTION));
    assert!(action_enabled(&DIRECTIVES_DEFAULT_ACTION, false, false));
}

fn items(entries: &[DropdownEntry<ActiveBarMsg>]) -> Vec<(String, bool)> {
    entries
        .iter()
        .filter_map(|e| match e {
            DropdownEntry::Item(it) => Some((it.label.clone(), it.disabled)),
            _ => None,
        })
        .collect()
}
fn labels(entries: &[DropdownEntry<ActiveBarMsg>]) -> Vec<String> {
    items(entries).into_iter().map(|(l, _)| l).collect()
}
fn seps(entries: &[DropdownEntry<ActiveBarMsg>]) -> usize {
    entries
        .iter()
        .filter(|e| matches!(e, DropdownEntry::Separator))
        .count()
}
fn customs(entries: &[DropdownEntry<ActiveBarMsg>]) -> usize {
    entries
        .iter()
        .filter(|e| matches!(e, DropdownEntry::Custom(_)))
        .count()
}
fn disabled_of(entries: &[DropdownEntry<ActiveBarMsg>], label: &str) -> bool {
    items(entries)
        .into_iter()
        .find(|(l, _)| l == label)
        .unwrap_or_else(|| panic!("missing row {label:?}"))
        .1
}

#[test]
fn action_enable_predicate() {
    // selection-gated action
    assert!(!action_enabled(
        &ActiveBarAction::MoveSelection,
        false,
        true
    ));
    assert!(action_enabled(&ActiveBarAction::MoveSelection, true, false));
    // net-colour-gated action
    assert!(!action_enabled(
        &ActiveBarAction::ClearNetColor,
        true,
        false
    ));
    assert!(action_enabled(&ActiveBarAction::ClearNetColor, true, true));
    // ungated action is always on
    assert!(action_enabled(&ActiveBarAction::DrawWire, false, false));
}

#[test]
fn wiring_menu_is_five_ungated_items() {
    let e = render(WIRING, TID, false, false);
    assert_eq!(
        labels(&e),
        ["Wire", "Bus", "Bus Entry", "Net Label", "Junction"]
    );
    assert!(items(&e).iter().all(|(_, d)| !d)); // wiring is never gated
    assert_eq!(seps(&e), 0);
}

#[test]
fn disabled_state_flips_but_labels_are_stable() {
    let on = render(SELECT, TID, true, false);
    let off = render(SELECT, TID, false, false);
    // same rows regardless of selection — only the disabled flag moves
    assert_eq!(labels(&on), labels(&off));
    // a selection-family row greys out with no selection
    assert!(!disabled_of(&on, "Move Selection"));
    assert!(disabled_of(&off, "Move Selection"));
}

#[test]
fn select_mode_toggle_sits_after_the_separator() {
    let e = render(SELECT_MODE, TID, true, true);
    assert_eq!(seps(&e), 1);
    assert_eq!(items(&e).last().unwrap().0, "Toggle Selection");
}

#[test]
fn align_and_shapes_have_expected_shape() {
    let a = render(ALIGN, TID, true, true);
    assert_eq!(items(&a).len(), 9);
    assert_eq!(seps(&a), 2);
    let s = render(SHAPES, TID, true, true);
    assert_eq!(items(&s).len(), 10);
    assert_eq!(seps(&s), 2);
}

#[test]
fn net_color_swatches_and_gated_clear_rows() {
    let tokens = signex_types::theme::theme_tokens(TID);
    let with = net_color_entries(&tokens, TID, false, true);
    let without = net_color_entries(&tokens, TID, false, false);
    assert_eq!(customs(&with), 7); // seven colour swatches
    assert_eq!(seps(&with), 2);
    // Clear rows grey out when no net carries a custom colour
    assert!(!disabled_of(&with, "Clear Net Color"));
    assert!(disabled_of(&without, "Clear Net Color"));
    assert!(disabled_of(&without, "Clear All Net Colors"));
    // Custom Color... is the arm phase — always enabled
    assert!(!disabled_of(&with, "Custom Color..."));
}

#[test]
fn filter_menu_is_a_single_custom_entry() {
    let tokens = signex_types::theme::theme_tokens(TID);
    let filters = std::collections::HashSet::new();
    let entry = filter_entry(&tokens, &filters, &[]);
    assert!(matches!(entry, DropdownEntry::Custom(_)));
}

/// Stable serializer for `DropdownEntry` rows, used by the
/// behaviour-proof golden test below (#457) to prove the
/// data-table refactor is byte-for-byte output identical to the
/// pre-refactor per-menu builder functions.
fn describe(entries: &[DropdownEntry<ActiveBarMsg>]) -> Vec<String> {
    entries
        .iter()
        .map(|e| match e {
            DropdownEntry::Separator => "SEP".to_string(),
            DropdownEntry::Header(label) => format!("HEADER:{label}"),
            DropdownEntry::Custom(_) => "CUSTOM".to_string(),
            DropdownEntry::Item(it) => format!(
                "ITEM:{}|disabled={}|icon={}|checked={}|shortcut={:?}",
                it.label,
                it.disabled,
                it.icon.is_some(),
                it.checked,
                it.shortcut
            ),
        })
        .collect()
}

/// GOLDEN (#457): `describe(dropdown_entries(menu, .., sel, nc))` for
/// every `ActiveBarMenu` variant x every `(has_selection,
/// has_net_colors)` combo, captured verbatim from the pre-refactor
/// per-menu builder functions (`select_mode_entries`,
/// `select_entries`, ... `shapes_entries`, plus `filter_entry` /
/// `net_color_entries`) before they were replaced by the `EntrySpec`
/// data table + generic `render`. If this test still passes after
/// the refactor, the refactor is output-identical.
const GOLDEN: &[(ActiveBarMenu, bool, bool, &[&str])] = &[
    (ActiveBarMenu::Filter, false, false, &["CUSTOM"]),
    (ActiveBarMenu::Filter, false, true, &["CUSTOM"]),
    (ActiveBarMenu::Filter, true, false, &["CUSTOM"]),
    (ActiveBarMenu::Filter, true, true, &["CUSTOM"]),
    (
        ActiveBarMenu::SelectMode,
        false,
        false,
        &[
            "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SelectMode,
        false,
        true,
        &[
            "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SelectMode,
        true,
        false,
        &[
            "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SelectMode,
        true,
        true,
        &[
            "ITEM:Lasso Select|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Inside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Outside Area|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Touching Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:All|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Connection|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Toggle Selection|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Select,
        false,
        false,
        &[
            "ITEM:Drag|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Move|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection by X, Y...|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Drag Selection|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Move To Front|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Counter-Clockwise|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Clockwise|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Bring To Front|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Bring To Front Of|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back Of|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Flip Selected Sheet Symbols Along X|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Flip Selected Sheet Symbols Along Y|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Select,
        false,
        true,
        &[
            "ITEM:Drag|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Move|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection by X, Y...|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Drag Selection|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Move To Front|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Counter-Clockwise|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Clockwise|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Bring To Front|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Bring To Front Of|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back Of|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Flip Selected Sheet Symbols Along X|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Flip Selected Sheet Symbols Along Y|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Select,
        true,
        false,
        &[
            "ITEM:Drag|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection by X, Y...|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Drag Selection|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move To Front|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Counter-Clockwise|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Clockwise|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Bring To Front|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bring To Front Of|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back Of|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Flip Selected Sheet Symbols Along X|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Flip Selected Sheet Symbols Along Y|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Select,
        true,
        true,
        &[
            "ITEM:Drag|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move Selection by X, Y...|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Drag Selection|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Move To Front|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Counter-Clockwise|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rotate Selection Clockwise|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Bring To Front|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bring To Front Of|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Send To Back Of|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Flip Selected Sheet Symbols Along X|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Flip Selected Sheet Symbols Along Y|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Align,
        false,
        false,
        &[
            "ITEM:Align Left|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Right|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Horizontal Centers|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Horizontally|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align Top|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Bottom|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Vertical Centers|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Vertically|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align To Grid|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Align,
        false,
        true,
        &[
            "ITEM:Align Left|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Right|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Horizontal Centers|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Horizontally|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align Top|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Bottom|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Align Vertical Centers|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Vertically|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align To Grid|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Align,
        true,
        false,
        &[
            "ITEM:Align Left|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Right|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Horizontal Centers|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Horizontally|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align Top|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Bottom|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Vertical Centers|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Vertically|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align To Grid|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Align,
        true,
        true,
        &[
            "ITEM:Align Left|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Right|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Horizontal Centers|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Horizontally|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align Top|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Bottom|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Align Vertical Centers|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Distribute Vertically|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Align To Grid|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Wiring,
        false,
        false,
        &[
            "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Junction|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Wiring,
        false,
        true,
        &[
            "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Junction|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Wiring,
        true,
        false,
        &[
            "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Junction|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Wiring,
        true,
        true,
        &[
            "ITEM:Wire|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bus Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Net Label|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Junction|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Power,
        false,
        false,
        &[
            "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Power,
        false,
        true,
        &[
            "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Power,
        true,
        false,
        &[
            "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Power,
        true,
        true,
        &[
            "ITEM:Place GND power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place VCC power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +12 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place +5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place -5 power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Arrow style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Wave style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Bar style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Circle style power port|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Place Signal Ground power port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Place Earth power port|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Harness,
        false,
        false,
        &[
            "ITEM:Signal Harness|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Connector|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Entry|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Harness,
        false,
        true,
        &[
            "ITEM:Signal Harness|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Connector|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Entry|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Harness,
        true,
        false,
        &[
            "ITEM:Signal Harness|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Connector|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Entry|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Harness,
        true,
        true,
        &[
            "ITEM:Signal Harness|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Connector|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Harness Entry|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SheetSymbol,
        false,
        false,
        &[
            "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Device Sheet Symbol|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Reuse Block...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SheetSymbol,
        false,
        true,
        &[
            "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Device Sheet Symbol|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Reuse Block...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SheetSymbol,
        true,
        false,
        &[
            "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Device Sheet Symbol|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Reuse Block...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::SheetSymbol,
        true,
        true,
        &[
            "ITEM:Sheet Symbol|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Sheet Entry|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Device Sheet Symbol|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Reuse Block...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Port,
        false,
        false,
        &[
            "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Port,
        false,
        true,
        &[
            "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Port,
        true,
        false,
        &[
            "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Port,
        true,
        true,
        &[
            "ITEM:Port|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Off Sheet Connector|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Directives,
        false,
        false,
        &[
            "ITEM:Parameter Set|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Differential Pair|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Blanket|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Compile Mask|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Directives,
        false,
        true,
        &[
            "ITEM:Parameter Set|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Differential Pair|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Blanket|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Compile Mask|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Directives,
        true,
        false,
        &[
            "ITEM:Parameter Set|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Differential Pair|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Blanket|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Compile Mask|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Directives,
        true,
        true,
        &[
            "ITEM:Parameter Set|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Generic No ERC|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Differential Pair|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Blanket|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Compile Mask|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::TextTools,
        false,
        false,
        &[
            "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::TextTools,
        false,
        true,
        &[
            "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::TextTools,
        true,
        false,
        &[
            "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::TextTools,
        true,
        true,
        &[
            "ITEM:Text String|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Text Frame|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Note|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Shapes,
        false,
        false,
        &[
            "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Elliptical Arc|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Ellipse|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Round Rectangle|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bezier|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Graphic...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Shapes,
        false,
        true,
        &[
            "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Elliptical Arc|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Ellipse|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Round Rectangle|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bezier|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Graphic...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Shapes,
        true,
        false,
        &[
            "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Elliptical Arc|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Ellipse|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Round Rectangle|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bezier|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Graphic...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::Shapes,
        true,
        true,
        &[
            "ITEM:Arc|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Full Circle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Elliptical Arc|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Ellipse|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Line|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Rectangle|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Round Rectangle|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Polygon|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Bezier|disabled=true|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Graphic...|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::NetColor,
        false,
        false,
        &[
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "SEP",
            "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Clear Net Color|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Clear All Net Colors|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::NetColor,
        false,
        true,
        &[
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "SEP",
            "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Clear Net Color|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Clear All Net Colors|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::NetColor,
        true,
        false,
        &[
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "SEP",
            "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Clear Net Color|disabled=true|icon=true|checked=false|shortcut=None",
            "ITEM:Clear All Net Colors|disabled=true|icon=true|checked=false|shortcut=None",
        ],
    ),
    (
        ActiveBarMenu::NetColor,
        true,
        true,
        &[
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "CUSTOM",
            "SEP",
            "ITEM:Custom Color...|disabled=false|icon=true|checked=false|shortcut=None",
            "SEP",
            "ITEM:Clear Net Color|disabled=false|icon=true|checked=false|shortcut=None",
            "ITEM:Clear All Net Colors|disabled=false|icon=true|checked=false|shortcut=None",
        ],
    ),
];

#[test]
fn dropdown_entries_match_pre_refactor_golden() {
    let tokens = signex_types::theme::theme_tokens(TID);
    let filters: std::collections::HashSet<SelectionFilter> = std::collections::HashSet::new();
    let presets: Vec<CustomFilterPreset> = vec![];
    for &(menu, sel, nc, expected) in GOLDEN {
        let entries = dropdown_entries(menu, &tokens, &filters, &presets, TID, sel, nc);
        let actual = describe(&entries);
        let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
        assert_eq!(
            actual_refs.as_slice(),
            expected,
            "menu={menu:?} sel={sel} nc={nc}: dropdown output diverged from the pre-refactor golden"
        );
    }
}
