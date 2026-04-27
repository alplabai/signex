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

/// Decorative IEEE-style modifier glyph attached to a pin's symbol
/// body. Altium splits these across four placement zones (Inside,
/// Inside Edge, Outside Edge, Outside) so a pin can carry multiple
/// modifiers (e.g. dot + clock for an inverted clock input). The
/// enum is `#[non_exhaustive]` because Altium ships 30+ IEEE glyphs
/// and we add them as needed — `None` is the default for legacy /
/// fresh pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PinSymbolKind {
    /// No modifier glyph in this slot.
    #[default]
    None,
    /// Small filled circle — "active low" / inverted polarity bubble.
    Dot,
    /// Right-pointing triangle — clock edge marker.
    ClockEdge,
    /// Inward chevron — active-low input.
    ActiveLowInput,
    /// Outward chevron — active-low output.
    ActiveLowOutput,
    /// Hysteresis curve — Schmitt-trigger input.
    SchmittTrigger,
    /// Analog-signal indicator (≈).
    Analog,
    /// Digital-signal indicator (square wave).
    Digital,
    /// "Right-arrow" group glyph (IEEE shift-right).
    ShiftRight,
    /// "Left-arrow" group glyph (IEEE shift-left).
    ShiftLeft,
    /// Pi (π) glyph — analog ratio / pi-network indicator.
    Pi,
    /// Sigma (Σ) glyph — summation point.
    Sigma,
    /// Open-collector output indicator (downward open square).
    OpenCollector,
    /// Open-emitter output indicator (upward open square).
    OpenEmitter,
    /// Hi-Z (tri-state) output indicator.
    HiZ,
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
    /// Free-text description shown in the Properties panel and
    /// surfaced on tooltips. Defaults to empty.
    #[serde(default)]
    pub description: String,
    /// Alternative pin names (Altium "Function" — multi-function pins
    /// carry several names like `MOSI/PA7`). Empty by default.
    #[serde(default)]
    pub function: Vec<String>,
    /// Optional pin-package length in mm — physical lead length on
    /// the package (distinct from `length` which is the schematic
    /// stub length). Used for SI / propagation models.
    #[serde(default)]
    pub pin_package_length: Option<f64>,
    /// Optional propagation delay in nanoseconds for this pin —
    /// flows into timing analysis when present.
    #[serde(default)]
    pub propagation_delay_ns: Option<f64>,
    /// Whether the designator (pin number) text is drawn next to
    /// the pin. Defaults to `true` to match legacy files; the
    /// Altium "eye" toggle flips this.
    #[serde(default = "default_visibility_true")]
    pub designator_visible: bool,
    /// Whether the name text is drawn next to the pin.
    #[serde(default = "default_visibility_true")]
    pub name_visible: bool,
    /// IEEE-style glyph drawn inside the symbol body at this pin's
    /// stub end. Default `None`.
    #[serde(default)]
    pub inside_symbol: PinSymbolKind,
    /// IEEE-style glyph drawn at the inside edge of the symbol body
    /// (right where the pin stub meets the body). Default `None`.
    #[serde(default)]
    pub inside_edge_symbol: PinSymbolKind,
    /// IEEE-style glyph drawn at the outside edge of the symbol
    /// body (right where the pin emerges). Default `None`. Most
    /// commonly carries the inverted-pin dot.
    #[serde(default)]
    pub outside_edge_symbol: PinSymbolKind,
    /// IEEE-style glyph drawn outside the symbol body, attached to
    /// the pin's free end. Default `None`.
    #[serde(default)]
    pub outside_symbol: PinSymbolKind,
    /// Hidden pins are not drawn on the schematic but still
    /// participate in netlists (Altium "Pin Hide"). Default false.
    #[serde(default)]
    pub hidden: bool,
    /// Locked pins refuse drag / delete via the canvas — must be
    /// edited through the Properties panel. Default false.
    #[serde(default)]
    pub locked: bool,
}

fn default_visibility_true() -> bool {
    true
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
            description: String::new(),
            function: Vec::new(),
            pin_package_length: None,
            propagation_delay_ns: None,
            designator_visible: true,
            name_visible: true,
            inside_symbol: PinSymbolKind::None,
            inside_edge_symbol: PinSymbolKind::None,
            outside_edge_symbol: PinSymbolKind::None,
            outside_symbol: PinSymbolKind::None,
            hidden: false,
            locked: false,
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

/// Multi-symbol `.snxsym` container — Altium SchLib parity. One file
/// holds many symbols; each symbol still has its own UUID for
/// `PrimitiveRef` resolution. The `format` field is a sentinel so
/// future schema bumps can be detected without breaking older
/// readers.
///
/// Backcompat: legacy single-symbol `.snxsym` files (a bare `Symbol`
/// JSON, written before v0.9 phase 2) deserialize via the
/// [`SymbolFileOnDisk`] enum's untagged variant — see [`SymbolFile::from_json`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolFile {
    /// Schema sentinel — current emitters write `"snxsym/v1"`. Older
    /// files (legacy single-symbol form) don't carry this field; the
    /// loader handles them via `SymbolFileOnDisk`.
    #[serde(default = "default_format")]
    pub format: String,
    /// File-level UUID — distinct from any contained symbol's uuid.
    /// Used as the file-rename-stable handle.
    pub file_uuid: Uuid,
    /// Human-facing library name shown in the SCH-Library panel header.
    /// Defaults to the file stem when empty.
    #[serde(default)]
    pub display_name: String,
    /// All symbols in this file. Order is the SCH-Library panel order.
    pub symbols: Vec<Symbol>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

fn default_format() -> String {
    "snxsym/v1".to_string()
}

/// Wire format for `.snxsym` JSON. Untagged enum so the same
/// `serde_json::from_str` call accepts both the new `SymbolFile`
/// container and legacy single-`Symbol` files written before
/// v0.9 phase 2.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum SymbolFileOnDisk {
    /// New multi-symbol container.
    Container(SymbolFile),
    /// Legacy single-symbol blob — wrapped on read.
    Legacy(Symbol),
}

impl SymbolFile {
    /// Build a new container holding a single symbol — what the
    /// `Add New ▸ Symbol` flow seeds.
    pub fn from_symbol(symbol: Symbol) -> Self {
        let now = Utc::now();
        Self {
            format: default_format(),
            file_uuid: Uuid::now_v7(),
            display_name: symbol.name.clone(),
            symbols: vec![symbol],
            created: now,
            updated: now,
        }
    }

    /// Parse `.snxsym` JSON — accepts both the new container format
    /// and legacy single-symbol files. Legacy files are wrapped into
    /// a one-element container at read time so all downstream code
    /// can assume the container shape.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        match serde_json::from_slice::<SymbolFileOnDisk>(bytes)? {
            SymbolFileOnDisk::Container(file) => Ok(file),
            SymbolFileOnDisk::Legacy(sym) => {
                let now = Utc::now();
                Ok(Self {
                    format: default_format(),
                    // Reuse the symbol's uuid as the file uuid for legacy
                    // files — preserves the on-disk filename when the
                    // adapter scheme had been `<uuid>.snxsym`.
                    file_uuid: sym.uuid,
                    display_name: sym.name.clone(),
                    created: sym.created,
                    updated: now,
                    symbols: vec![sym],
                })
            }
        }
    }

    /// Locate a symbol by UUID within this file.
    pub fn get_symbol(&self, uuid: Uuid) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.uuid == uuid)
    }

    /// Locate a symbol by UUID within this file (mutable).
    pub fn get_symbol_mut(&mut self, uuid: Uuid) -> Option<&mut Symbol> {
        self.symbols.iter_mut().find(|s| s.uuid == uuid)
    }

    /// Replace `symbol` in the container — matches by `symbol.uuid`.
    /// Returns `false` when the uuid is not present (caller should
    /// `push` into `symbols` instead).
    pub fn upsert(&mut self, symbol: Symbol) -> bool {
        if let Some(slot) = self.get_symbol_mut(symbol.uuid) {
            *slot = symbol;
            self.updated = Utc::now();
            true
        } else {
            false
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
                description: String::new(),
                function: Vec::new(),
                pin_package_length: None,
                propagation_delay_ns: None,
                designator_visible: true,
                name_visible: true,
                inside_symbol: PinSymbolKind::None,
                inside_edge_symbol: PinSymbolKind::None,
                outside_edge_symbol: PinSymbolKind::None,
                outside_symbol: PinSymbolKind::None,
                hidden: false,
                locked: false,
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
    fn symbol_file_round_trip_with_two_symbols() {
        let s1 = Symbol::empty("ALPHA");
        let s2 = Symbol::empty("BETA");
        let mut file = SymbolFile::from_symbol(s1.clone());
        file.symbols.push(s2.clone());
        let json = serde_json::to_vec(&file).unwrap();
        let back = SymbolFile::from_json(&json).unwrap();
        assert_eq!(back.symbols.len(), 2);
        assert_eq!(back.symbols[0].name, "ALPHA");
        assert_eq!(back.symbols[1].name, "BETA");
        assert_eq!(back.format, "snxsym/v1");
    }

    /// Legacy `.snxsym` files (single Symbol JSON, no container) must
    /// still load — the editor wraps them into one-element
    /// `SymbolFile`s on read so downstream code can assume the
    /// container shape. The wrapped file's uuid mirrors the symbol's
    /// uuid so the on-disk filename `<uuid>.snxsym` is preserved when
    /// the loader rewrites with the new format.
    #[test]
    fn symbol_file_loads_legacy_single_symbol_json() {
        let s = Symbol::empty("LEGACY");
        let bare_symbol_json = serde_json::to_vec(&s).unwrap();
        let file = SymbolFile::from_json(&bare_symbol_json).unwrap();
        assert_eq!(file.symbols.len(), 1);
        assert_eq!(file.symbols[0].name, "LEGACY");
        assert_eq!(file.symbols[0].uuid, s.uuid);
        assert_eq!(file.file_uuid, s.uuid);
        assert_eq!(file.display_name, "LEGACY");
    }

    /// `SymbolFile::upsert` replaces a matching-uuid symbol in-place
    /// and returns true; non-matching uuids return false so the
    /// caller can `push` instead.
    #[test]
    fn symbol_file_upsert_replaces_matching_uuid() {
        let original = Symbol::empty("FIRST");
        let mut file = SymbolFile::from_symbol(original.clone());
        let mut updated = original.clone();
        updated.name = "FIRST_RENAMED".into();
        assert!(file.upsert(updated.clone()));
        assert_eq!(file.symbols.len(), 1);
        assert_eq!(file.symbols[0].name, "FIRST_RENAMED");

        let unrelated = Symbol::empty("OTHER");
        assert!(!file.upsert(unrelated));
        // Caller would push; we just verify upsert didn't accidentally add.
        assert_eq!(file.symbols.len(), 1);
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
