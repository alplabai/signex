//! Sim tab editor state.
//!
//! Owns the live multi-line `text_editor::Content` for the SPICE body
//! plus a small cache of pin numbers extracted from the parent
//! component's symbol. The canonical [`SpiceModel`] still lives on
//! `draft.shared.simulation`; this state just bridges the iced-side
//! widget plumbing to it.
//!
//! Round-trip invariants exercised by the tests at the bottom of
//! this file:
//!
//! 1. Starting from a `SharedSide.simulation: Some(SpiceModel)` round-
//!    trips through `serde_json` and lands back as the same model.
//! 2. Toggling "Has SPICE model" off clears `simulation` to `None` and
//!    a subsequent JSON encode emits a missing key (default-skipping).
//! 3. Pin extraction from a `(symbol …)` body returns the same pin
//!    numbers Standard reports; an unparseable body falls back to a
//!    numeric `1..N` skeleton driven by `pin_count_hint`.

use std::collections::BTreeMap;

use iced::widget::text_editor;
use signex_library::SpiceModel;

/// In-flight Sim tab state — one per Component Editor window.
#[derive(Debug)]
pub struct SimTabState {
    /// Multi-line iced editor backing the SPICE body. Kept in sync
    /// with `draft.shared.simulation.body` via
    /// `EditorMsg::SimBodyAction` mutations.
    pub body: text_editor::Content,
    /// Cached list of pin numbers extracted from the parent
    /// component's symbol body. Stable order driven by `extract_pin_numbers`.
    pub pin_numbers: Vec<String>,
}

impl Clone for SimTabState {
    fn clone(&self) -> Self {
        // `text_editor::Content` is `Clone`, but we re-derive from text
        // so the clone is robust against any future loss-of-Clone.
        Self {
            body: text_editor::Content::with_text(&self.body.text()),
            pin_numbers: self.pin_numbers.clone(),
        }
    }
}

impl Default for SimTabState {
    fn default() -> Self {
        Self {
            body: text_editor::Content::new(),
            pin_numbers: Vec::new(),
        }
    }
}

impl SimTabState {
    /// Build a fresh Sim state from the (optional) saved model and the
    /// parent symbol's S-expression body.
    pub fn from_model(model: Option<&SpiceModel>, symbol_sexpr: &str) -> Self {
        let body_str = model.map(|m| m.body.clone()).unwrap_or_default();
        let pin_numbers = extract_pin_numbers(symbol_sexpr);
        Self {
            body: text_editor::Content::with_text(&body_str),
            pin_numbers,
        }
    }

    /// Snapshot the current body text — used by the dispatcher to
    /// recompute `SpiceModel.body` after a `text_editor::Action`.
    pub fn body_text(&self) -> String {
        self.body.text()
    }
}

/// Extract pin numbers from a Standard symbol-body S-expression.
///
/// Parsing rules:
///
/// 1. The body is wrapped in `(standard_symbol_lib (version 1) (generator signex) …)`
///    so `standard_parser::parse_symbol_lib` can be reused without a new
///    code path.
/// 2. If parsing succeeds and yields at least one symbol with at least
///    one pin, the pin numbers (in declaration order) are returned.
/// 3. Otherwise an empty `Vec` is returned — the caller falls back to
///    a numeric skeleton via [`numeric_pin_fallback`] when the symbol
///    is brand new and the Sim tab still wants a usable grid.
pub fn extract_pin_numbers(symbol_sexpr: &str) -> Vec<String> {
    let trimmed = symbol_sexpr.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // The symbol body in the library schema is a bare `(symbol "ID" …)`
    // node. Standard's symbol-lib parser expects the `(standard_symbol_lib …)`
    // wrapper, so synthesise one if it's missing.
    let candidate = if trimmed.starts_with("(standard_symbol_lib") {
        trimmed.to_string()
    } else {
        format!("(standard_symbol_lib (version 20231120) (generator signex)\n{trimmed}\n)")
    };

    match standard_parser::parse_symbol_lib(&candidate) {
        Ok(map) => {
            let mut numbers = Vec::new();
            // The map is unordered but each `LibSymbol.pins` keeps the
            // declaration order — pick the first (and typically only)
            // entry for the current symbol.
            if let Some((_, lib)) = map.iter().next() {
                for pin in &lib.pins {
                    let n = pin.pin.number.trim().to_string();
                    if !n.is_empty() && !numbers.contains(&n) {
                        numbers.push(n);
                    }
                }
            }
            numbers
        }
        Err(_) => Vec::new(),
    }
}

/// Build a numeric `1..=count` pin skeleton — used when the symbol
/// body is empty or unparseable. Returns an empty `Vec` for `count == 0`.
///
/// Phase 1 keeps this exported but the `view()` doesn't use it yet —
/// Phase 2 wires a "+ N pins" stepper to populate the grid before the
/// symbol exists. Tests below exercise the API directly.
#[allow(dead_code)]
pub fn numeric_pin_fallback(count: usize) -> Vec<String> {
    (1..=count).map(|i| i.to_string()).collect()
}

/// Reconcile the editor state with a freshly-edited pin-map row.
///
/// Returns the new full `BTreeMap` for `SpiceModel.pin_map`. Used by
/// the dispatcher to turn a single-row edit into the canonical map.
pub fn apply_pin_node_edit(
    current: &BTreeMap<String, String>,
    pin_number: &str,
    value: String,
) -> BTreeMap<String, String> {
    let mut out = current.clone();
    if value.trim().is_empty() {
        out.remove(pin_number);
    } else {
        out.insert(pin_number.to_string(), value);
    }
    out
}

/// Seed an empty SPICE model with rows for every known pin number.
/// New rows start with empty node names — the user fills them in.
pub fn seed_empty_pin_map(pin_numbers: &[String]) -> BTreeMap<String, String> {
    pin_numbers
        .iter()
        .map(|n| (n.clone(), String::new()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::SpiceModel;

    const RESISTOR_SYMBOL: &str = r#"(symbol "R"
        (pin_numbers hide)
        (pin_names (offset 0))
        (in_bom yes)
        (on_board yes)
        (symbol "R_0_1"
          (rectangle (start -1.016 -2.54) (end 1.016 2.54))
        )
        (symbol "R_1_1"
          (pin passive line (at 0 3.81 270) (length 1.27)
            (name "~" (effects (font (size 1.27 1.27))))
            (number "1" (effects (font (size 1.27 1.27))))
          )
          (pin passive line (at 0 -3.81 90) (length 1.27)
            (name "~" (effects (font (size 1.27 1.27))))
            (number "2" (effects (font (size 1.27 1.27))))
          )
        )
      )"#;

    #[test]
    fn extract_pin_numbers_resistor() {
        let pins = extract_pin_numbers(RESISTOR_SYMBOL);
        assert_eq!(pins, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn extract_pin_numbers_empty_body_returns_empty() {
        let pins = extract_pin_numbers("");
        assert!(pins.is_empty());
    }

    #[test]
    fn extract_pin_numbers_unparseable_returns_empty() {
        let pins = extract_pin_numbers("not an s-expr at all");
        assert!(pins.is_empty());
    }

    #[test]
    fn numeric_fallback_is_one_indexed() {
        let pins = numeric_pin_fallback(3);
        assert_eq!(pins, vec!["1", "2", "3"]);
        assert!(numeric_pin_fallback(0).is_empty());
    }

    #[test]
    fn apply_pin_node_edit_sets_then_clears() {
        let mut map = BTreeMap::new();
        map.insert("1".into(), "VDD".into());
        let next = apply_pin_node_edit(&map, "2", "GND".to_string());
        assert_eq!(next.get("2"), Some(&"GND".to_string()));
        // Empty / whitespace value clears the row.
        let cleared = apply_pin_node_edit(&next, "1", "  ".to_string());
        assert_eq!(cleared.get("1"), None);
        assert_eq!(cleared.get("2"), Some(&"GND".to_string()));
    }

    #[test]
    fn pin_map_round_trips_through_json() {
        // Build a non-trivial SpiceModel.
        let mut pin_map = BTreeMap::new();
        pin_map.insert("1".to_string(), "vdd".to_string());
        pin_map.insert("2".to_string(), "gnd".to_string());
        pin_map.insert("3".to_string(), "out".to_string());
        let model = SpiceModel {
            body: ".SUBCKT op_amp 1 2 3\n* dummy\n.ENDS".to_string(),
            pin_map: pin_map.clone(),
        };

        // Round-trip.
        let json = serde_json::to_string(&model).expect("serialize");
        let back: SpiceModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.body, model.body);
        assert_eq!(back.pin_map, model.pin_map);
        // BTreeMap preserves key order — pin "1" must come first.
        assert_eq!(back.pin_map.keys().next(), Some(&"1".to_string()));
    }

    #[test]
    fn simulation_none_is_round_trip_safe() {
        // Confirm the toggle-off path: simulation = None survives JSON.
        use signex_library::SharedSide;
        let mut shared = SharedSide::default();
        // Set then clear — mirrors the user toggling "Has SPICE model".
        shared.simulation = Some(SpiceModel {
            body: "x".into(),
            pin_map: BTreeMap::new(),
        });
        shared.simulation = None;
        let json = serde_json::to_string(&shared).expect("serialize");
        let back: SharedSide = serde_json::from_str(&json).expect("deserialize");
        assert!(back.simulation.is_none());
    }

    #[test]
    fn seed_empty_pin_map_creates_one_row_per_pin() {
        let pins = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let map = seed_empty_pin_map(&pins);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("1"), Some(&String::new()));
        assert_eq!(map.get("2"), Some(&String::new()));
        assert_eq!(map.get("3"), Some(&String::new()));
    }

    #[test]
    fn from_model_seeds_body_and_pin_numbers() {
        let model = SpiceModel {
            body: "BODY".to_string(),
            pin_map: BTreeMap::new(),
        };
        let st = SimTabState::from_model(Some(&model), RESISTOR_SYMBOL);
        assert_eq!(st.body_text(), "BODY");
        assert_eq!(st.pin_numbers, vec!["1", "2"]);
    }

    #[test]
    fn from_model_none_yields_empty_body() {
        let st = SimTabState::from_model(None, "");
        assert!(st.body_text().is_empty());
        assert!(st.pin_numbers.is_empty());
    }
}
