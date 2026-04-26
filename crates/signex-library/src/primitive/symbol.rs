//! `Symbol` primitive — schematic-side reusable shape.
//!
//! Per `v0.9-library-refactor-plan.md` §2.1, a `Symbol` carries:
//! - typed pin list (no more opaque `(symbol …)` blob),
//! - drawing primitives (lines/rects/arcs/text),
//! - default schematic parameters that flow onto a binding `Component`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::param::ParamMap;

/// Electrical role of a pin — drives ERC and BOM rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PinElectricalType {
    Input,
    Output,
    Bidirectional,
    Power,
    Passive,
    OpenCollector,
    OpenEmitter,
    NotConnected,
    /// Tri-state — high-impedance is a valid output.
    Tristate,
    /// Unspecified — the symbol author hasn't picked yet (default for new pins).
    #[default]
    Unspecified,
}

/// Pin orientation — which direction the pin extends from the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PinOrientation {
    Up,
    Down,
    Left,
    #[default]
    Right,
}

/// One symbol pin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolPin {
    /// Pin number — the binding key for `PinPadOverride` mapping ("1", "VCC").
    pub number: String,
    /// Display label drawn next to the pin ("IN+", "VCC").
    pub name: String,
    pub electrical: PinElectricalType,
    /// Position of the pin's connection point in symbol-local mm coordinates.
    pub position: [f64; 2],
    pub orientation: PinOrientation,
    /// Length of the pin's drawn stub from the connection point inward.
    pub length: f64,
}

impl SymbolPin {
    /// Convenience constructor for plumb-default tests + scaffolding.
    pub fn new(number: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            name: name.into(),
            electrical: PinElectricalType::Unspecified,
            position: [0.0, 0.0],
            orientation: PinOrientation::Right,
            length: 2.54,
        }
    }
}

/// Drawing primitive kinds — the geometry of the symbol body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SymbolGraphicKind {
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Rectangle {
        from: [f64; 2],
        to: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Text {
        position: [f64; 2],
        content: String,
        size: f64,
    },
}

/// One graphic on the symbol body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolGraphic {
    pub kind: SymbolGraphicKind,
    /// Stroke width in mm (0.0 = use renderer default).
    #[serde(default)]
    pub stroke_width: f64,
}

/// Reusable schematic primitive. Bound by a `Component::symbol_ref`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub uuid: Uuid,
    /// Human-facing name ("OPAMP-DUAL-8") — independent of the binding
    /// component's `internal_pn`.
    pub name: String,
    /// Anchor point in symbol-local mm coordinates (typically `[0, 0]`).
    #[serde(default)]
    pub anchor: [f64; 2],
    pub pins: Vec<SymbolPin>,
    #[serde(default)]
    pub graphics: Vec<SymbolGraphic>,
    /// Default parameter values that flow to a binding component on first save.
    #[serde(default)]
    pub schematic_params: ParamMap,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

impl Symbol {
    /// Empty symbol with one default pin — what the New Component flow seeds.
    pub fn empty(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::now_v7(),
            name: name.into(),
            anchor: [0.0, 0.0],
            pins: vec![SymbolPin::new("1", "1")],
            graphics: Vec::new(),
            schematic_params: ParamMap::new(),
            created: now,
            updated: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_json_roundtrip() {
        let s = Symbol {
            uuid: Uuid::now_v7(),
            name: "OPAMP-DUAL-8".into(),
            anchor: [0.0, 0.0],
            pins: vec![SymbolPin {
                number: "1".into(),
                name: "OUT_A".into(),
                electrical: PinElectricalType::Output,
                position: [0.0, 2.54],
                orientation: PinOrientation::Right,
                length: 2.54,
            }],
            graphics: vec![SymbolGraphic {
                kind: SymbolGraphicKind::Rectangle {
                    from: [-2.5, -2.5],
                    to: [2.5, 2.5],
                },
                stroke_width: 0.15,
            }],
            schematic_params: ParamMap::new(),
            created: Utc::now(),
            updated: Utc::now(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn pin_electrical_type_round_trip_all_variants() {
        for t in [
            PinElectricalType::Input,
            PinElectricalType::Output,
            PinElectricalType::Bidirectional,
            PinElectricalType::Power,
            PinElectricalType::Passive,
            PinElectricalType::OpenCollector,
            PinElectricalType::OpenEmitter,
            PinElectricalType::NotConnected,
            PinElectricalType::Tristate,
            PinElectricalType::Unspecified,
        ] {
            let json = serde_json::to_string(&t).unwrap();
            let back: PinElectricalType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn pin_orientation_round_trip_all_variants() {
        for o in [
            PinOrientation::Up,
            PinOrientation::Down,
            PinOrientation::Left,
            PinOrientation::Right,
        ] {
            let json = serde_json::to_string(&o).unwrap();
            let back: PinOrientation = serde_json::from_str(&json).unwrap();
            assert_eq!(o, back);
        }
    }

    #[test]
    fn symbol_graphic_kind_round_trip_each_variant() {
        let cases = [
            SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [1.0, 1.0],
            },
            SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [1.0, 1.0],
            },
            SymbolGraphicKind::Circle {
                center: [0.0, 0.0],
                radius: 1.0,
            },
            SymbolGraphicKind::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start_deg: 0.0,
                end_deg: 90.0,
            },
            SymbolGraphicKind::Text {
                position: [0.0, 0.0],
                content: "U1".into(),
                size: 1.27,
            },
        ];
        for k in cases {
            let json = serde_json::to_string(&k).unwrap();
            let back: SymbolGraphicKind = serde_json::from_str(&json).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn empty_symbol_carries_one_default_pin() {
        let s = Symbol::empty("test");
        assert_eq!(s.name, "test");
        assert_eq!(s.pins.len(), 1);
        assert_eq!(s.pins[0].number, "1");
    }
}
