//! `Symbol` primitive — schematic-side reusable shape.
//!
//! Per `v0.9-refactor-2-plan.md` §2.1, a `Symbol` carries:
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
pub enum PinDirection {
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
    pub electrical: PinDirection,
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
    /// Multi-part component support — which sub-part this pin belongs
    /// to. `1` is the default (single-part components); `0` is the
    /// special "Part Zero" Altium uses for pins that appear on every
    /// part (typically power / ground). Higher numbers (`2..=N`)
    /// scope the pin to a specific part. The canvas + SCH Library
    /// panel will honour this in a future commit; for now every pin
    /// is rendered regardless of part_number.
    #[serde(default = "default_part_number")]
    pub part_number: u8,
}

fn default_visibility_true() -> bool {
    true
}

fn default_part_number() -> u8 {
    1
}

impl SymbolPin {
    /// Convenience constructor for plumb-default tests + scaffolding.
    pub fn new(number: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            name: name.into(),
            electrical: PinDirection::Unspecified,
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
            part_number: 1,
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

/// Altium "Component Type" — drives BOM rules and schematic
/// behaviour. `Standard` is the normal electrical component.
/// `#[non_exhaustive]` because Altium ships a handful of niche types
/// (Standard No BOM, Net Tie, etc.) — we add as needed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComponentType {
    #[default]
    Standard,
    Mechanical,
    Graphical,
    NetTie,
    StandardNoBom,
    Jumper,
}

/// Reusable schematic primitive. Bound by a `Component::symbol_ref`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub uuid: Uuid,
    /// Human-facing name ("OPAMP-DUAL-8") — independent of the binding
    /// component's `internal_pn`. Altium calls this "Design Item ID".
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
    /// Altium "Designator" — placeholder string used at schematic
    /// place-time (e.g. `"U?"`, `"R?"`). The `?` is replaced by an
    /// instance number during annotation.
    #[serde(default = "default_designator")]
    pub designator: String,
    /// Altium "Comment" — passes through to the placed component's
    /// Comment field. Often `*` (placeholder) or a value like
    /// `"100k"` for a fixed resistor symbol.
    #[serde(default = "default_comment")]
    pub comment: String,
    /// Free-text component description — surfaced in tooltips and
    /// the Properties panel.
    #[serde(default)]
    pub description: String,
    /// Altium "Component Type" — Standard / Mechanical / Graphical /
    /// Net Tie / Standard (No BOM) / Jumper.
    #[serde(default)]
    pub component_type: ComponentType,
    /// Whether the symbol is mirrored on the canvas (Altium
    /// "Graphical ▸ Mirrored" toggle). Default false.
    #[serde(default)]
    pub mirrored: bool,
    /// Optional per-symbol fill colour override (Altium "Local
    /// Colors ▸ Fills"). `None` = inherit from theme.
    #[serde(default)]
    pub local_fill_color: Option<[u8; 4]>,
    /// Optional per-symbol line/stroke colour override.
    #[serde(default)]
    pub local_line_color: Option<[u8; 4]>,
    /// Optional per-symbol pin colour override.
    #[serde(default)]
    pub local_pin_color: Option<[u8; 4]>,
    /// Semver-style revision string (`X.Y.Z`). Stage 14 of
    /// `v0.9-snxlib-as-file-plan.md`: every Symbol carries its own
    /// version independent of the bound ComponentRow's version. In
    /// `Personal` workflow mode every save patch-bumps automatically;
    /// in `Team` mode released symbols require an explicit bump
    /// dialog. Defaults to `"0.0.1"` for new symbols and old files
    /// missing the field (back-compat: pre-Stage-14 `.snxsym` files
    /// load cleanly via serde default).
    #[serde(default = "default_version")]
    pub version: String,
    /// When `true`, the symbol is locked from edit-in-place — saves
    /// open the bump dialog (Team mode only). Defaults to `false` so
    /// new + legacy symbols stay editable until the user explicitly
    /// marks them released.
    #[serde(default)]
    pub released: bool,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

fn default_version() -> String {
    "0.0.1".to_string()
}

fn default_designator() -> String {
    "U?".to_string()
}

fn default_comment() -> String {
    "*".to_string()
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
            designator: default_designator(),
            comment: default_comment(),
            description: String::new(),
            component_type: ComponentType::default(),
            mirrored: false,
            local_fill_color: None,
            local_line_color: None,
            local_pin_color: None,
            version: default_version(),
            released: false,
            created: now,
            updated: now,
        }
    }
}

/// Multi-symbol `.snxsym` container — Altium SchLib parity. One file
/// holds many symbols; each symbol still has its own UUID for
/// `PrimitiveRef` resolution.
///
/// Wire format (v0.18.4): TOML manifest header + one `[[symbols]]`
/// array entry per Symbol. Each entry's bulk pin list is embedded as
/// a TSV literal multi-line string (`pins_tsv = '''…'''`) — line-
/// diffable in git, editable in any spreadsheet. Graphics, parameter
/// maps, and per-symbol metadata stay as inline TOML since they're
/// either variant-shaped or sparse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolFile {
    /// Schema sentinel — current emitters write `"snxsym/v1"`.
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

const SYMBOL_FILE_FORMAT_TOKEN: &str = "snxsym/v1";

/// Stable column layout for the per-symbol `pins_tsv` block. Adding
/// or reordering columns is a wire-format break — bump
/// [`SYMBOL_FILE_FORMAT_TOKEN`].
const PIN_TSV_COLUMNS: &[&str] = &[
    "number",
    "name",
    "electrical",
    "pos_x",
    "pos_y",
    "orientation",
    "length",
    "description",
    "function",
    "pin_package_length",
    "propagation_delay_ns",
    "designator_visible",
    "name_visible",
    "inside_symbol",
    "inside_edge_symbol",
    "outside_edge_symbol",
    "outside_symbol",
    "hidden",
    "locked",
    "part_number",
];

/// Sentinel string substituted for each symbol's `pins_tsv` field
/// before TOML serialise; replaced post-emit with the literal multi-
/// line `'''…'''` block. The long random suffix prevents collision
/// with any plausible pin field text.
const PINS_TSV_PLACEHOLDER_PREFIX: &str = "__SIGNEX_PINS_TSV_a1b2c3d4_";

fn default_format() -> String {
    SYMBOL_FILE_FORMAT_TOKEN.to_string()
}

/// On-disk wire shape. Mirrors [`SymbolFile`] but each [`Symbol`]'s
/// `pins` Vec is replaced with a `pins_tsv: String` carrying the TSV-
/// encoded payload.
#[derive(Serialize, Deserialize)]
struct SymbolFileWire {
    format: String,
    file_uuid: Uuid,
    #[serde(default)]
    display_name: String,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    #[serde(default)]
    symbols: Vec<SymbolWire>,
}

#[derive(Serialize, Deserialize)]
struct SymbolWire {
    uuid: Uuid,
    name: String,
    #[serde(default)]
    anchor: [f64; 2],
    /// TSV-encoded pin list — header row + one row per pin.
    pins_tsv: String,
    #[serde(default)]
    graphics: Vec<SymbolGraphic>,
    #[serde(default)]
    schematic_params: ParamMap,
    #[serde(default = "default_designator")]
    designator: String,
    #[serde(default = "default_comment")]
    comment: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    component_type: ComponentType,
    #[serde(default)]
    mirrored: bool,
    #[serde(default)]
    local_fill_color: Option<[u8; 4]>,
    #[serde(default)]
    local_line_color: Option<[u8; 4]>,
    #[serde(default)]
    local_pin_color: Option<[u8; 4]>,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    released: bool,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
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

    /// Decode bytes as UTF-8 and parse via [`SymbolFile::from_toml_str`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SymbolFileError> {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(SymbolFileError::Empty);
        }
        let text = std::str::from_utf8(bytes)?;
        Self::from_toml_str(text)
    }

    /// Parse the TOML+TSV wire format. The format-token check pins us
    /// to [`SYMBOL_FILE_FORMAT_TOKEN`]; mismatched tokens surface
    /// [`SymbolFileError::UnsupportedFormat`].
    pub fn from_toml_str(text: &str) -> Result<Self, SymbolFileError> {
        let wire: SymbolFileWire = toml::from_str(text)?;
        if wire.format != SYMBOL_FILE_FORMAT_TOKEN {
            return Err(SymbolFileError::UnsupportedFormat { got: wire.format });
        }
        let mut symbols = Vec::with_capacity(wire.symbols.len());
        for sw in wire.symbols {
            let pins = pins_from_tsv(&sw.pins_tsv)?;
            symbols.push(Symbol {
                uuid: sw.uuid,
                name: sw.name,
                anchor: sw.anchor,
                pins,
                graphics: sw.graphics,
                schematic_params: sw.schematic_params,
                designator: sw.designator,
                comment: sw.comment,
                description: sw.description,
                component_type: sw.component_type,
                mirrored: sw.mirrored,
                local_fill_color: sw.local_fill_color,
                local_line_color: sw.local_line_color,
                local_pin_color: sw.local_pin_color,
                version: sw.version,
                released: sw.released,
                created: sw.created,
                updated: sw.updated,
            });
        }
        Ok(SymbolFile {
            format: wire.format,
            file_uuid: wire.file_uuid,
            display_name: wire.display_name,
            created: wire.created,
            updated: wire.updated,
            symbols,
        })
    }

    /// Serialise to canonical TOML+TSV. Pin lists become
    /// `pins_tsv = '''\n<header>\n<rows>\n'''` literal multi-line
    /// strings so the bulk data is line-diffable in git output.
    pub fn to_toml_string(&self) -> Result<String, SymbolFileError> {
        let mut tsv_payloads: Vec<String> = Vec::with_capacity(self.symbols.len());
        let mut wire_symbols: Vec<SymbolWire> = Vec::with_capacity(self.symbols.len());
        for (idx, sym) in self.symbols.iter().enumerate() {
            tsv_payloads.push(pins_to_tsv(&sym.pins)?);
            wire_symbols.push(SymbolWire {
                uuid: sym.uuid,
                name: sym.name.clone(),
                anchor: sym.anchor,
                pins_tsv: format!("{PINS_TSV_PLACEHOLDER_PREFIX}{idx}__"),
                graphics: sym.graphics.clone(),
                schematic_params: sym.schematic_params.clone(),
                designator: sym.designator.clone(),
                comment: sym.comment.clone(),
                description: sym.description.clone(),
                component_type: sym.component_type,
                mirrored: sym.mirrored,
                local_fill_color: sym.local_fill_color,
                local_line_color: sym.local_line_color,
                local_pin_color: sym.local_pin_color,
                version: sym.version.clone(),
                released: sym.released,
                created: sym.created,
                updated: sym.updated,
            });
        }
        let wire = SymbolFileWire {
            format: self.format.clone(),
            file_uuid: self.file_uuid,
            display_name: self.display_name.clone(),
            created: self.created,
            updated: self.updated,
            symbols: wire_symbols,
        };
        let mut out = toml::to_string_pretty(&wire).map_err(SymbolFileError::TomlSerialize)?;
        for (idx, payload) in tsv_payloads.iter().enumerate() {
            let needle = format!("\"{PINS_TSV_PLACEHOLDER_PREFIX}{idx}__\"");
            let replacement = format!("'''\n{payload}'''");
            out = out.replace(&needle, &replacement);
        }
        Ok(out)
    }
}

// ---- Pin TSV codec --------------------------------------------------

fn pin_direction_token(d: PinDirection) -> &'static str {
    match d {
        PinDirection::Input => "Input",
        PinDirection::Output => "Output",
        PinDirection::Bidirectional => "Bidirectional",
        PinDirection::Power => "Power",
        PinDirection::Passive => "Passive",
        PinDirection::OpenCollector => "OpenCollector",
        PinDirection::OpenEmitter => "OpenEmitter",
        PinDirection::NotConnected => "NotConnected",
        PinDirection::Tristate => "Tristate",
        PinDirection::Unspecified => "Unspecified",
    }
}

fn pin_direction_from_token(s: &str) -> Result<PinDirection, SymbolFileError> {
    Ok(match s {
        "Input" => PinDirection::Input,
        "Output" => PinDirection::Output,
        "Bidirectional" => PinDirection::Bidirectional,
        "Power" => PinDirection::Power,
        "Passive" => PinDirection::Passive,
        "OpenCollector" => PinDirection::OpenCollector,
        "OpenEmitter" => PinDirection::OpenEmitter,
        "NotConnected" => PinDirection::NotConnected,
        "Tristate" => PinDirection::Tristate,
        "Unspecified" => PinDirection::Unspecified,
        other => return Err(SymbolFileError::UnknownEnumToken {
            kind: "PinDirection",
            got: other.to_string(),
        }),
    })
}

fn pin_orientation_token(o: PinOrientation) -> &'static str {
    match o {
        PinOrientation::Up => "Up",
        PinOrientation::Down => "Down",
        PinOrientation::Left => "Left",
        PinOrientation::Right => "Right",
    }
}

fn pin_orientation_from_token(s: &str) -> Result<PinOrientation, SymbolFileError> {
    Ok(match s {
        "Up" => PinOrientation::Up,
        "Down" => PinOrientation::Down,
        "Left" => PinOrientation::Left,
        "Right" => PinOrientation::Right,
        other => return Err(SymbolFileError::UnknownEnumToken {
            kind: "PinOrientation",
            got: other.to_string(),
        }),
    })
}

fn pin_symbol_kind_token(k: PinSymbolKind) -> &'static str {
    match k {
        PinSymbolKind::None => "None",
        PinSymbolKind::Dot => "Dot",
        PinSymbolKind::ClockEdge => "ClockEdge",
        PinSymbolKind::ActiveLowInput => "ActiveLowInput",
        PinSymbolKind::ActiveLowOutput => "ActiveLowOutput",
        PinSymbolKind::SchmittTrigger => "SchmittTrigger",
        PinSymbolKind::Analog => "Analog",
        PinSymbolKind::Digital => "Digital",
        PinSymbolKind::ShiftRight => "ShiftRight",
        PinSymbolKind::ShiftLeft => "ShiftLeft",
        PinSymbolKind::Pi => "Pi",
        PinSymbolKind::Sigma => "Sigma",
        PinSymbolKind::OpenCollector => "OpenCollector",
        PinSymbolKind::OpenEmitter => "OpenEmitter",
        PinSymbolKind::HiZ => "HiZ",
    }
}

fn pin_symbol_kind_from_token(s: &str) -> Result<PinSymbolKind, SymbolFileError> {
    Ok(match s {
        "None" => PinSymbolKind::None,
        "Dot" => PinSymbolKind::Dot,
        "ClockEdge" => PinSymbolKind::ClockEdge,
        "ActiveLowInput" => PinSymbolKind::ActiveLowInput,
        "ActiveLowOutput" => PinSymbolKind::ActiveLowOutput,
        "SchmittTrigger" => PinSymbolKind::SchmittTrigger,
        "Analog" => PinSymbolKind::Analog,
        "Digital" => PinSymbolKind::Digital,
        "ShiftRight" => PinSymbolKind::ShiftRight,
        "ShiftLeft" => PinSymbolKind::ShiftLeft,
        "Pi" => PinSymbolKind::Pi,
        "Sigma" => PinSymbolKind::Sigma,
        "OpenCollector" => PinSymbolKind::OpenCollector,
        "OpenEmitter" => PinSymbolKind::OpenEmitter,
        "HiZ" => PinSymbolKind::HiZ,
        other => return Err(SymbolFileError::UnknownEnumToken {
            kind: "PinSymbolKind",
            got: other.to_string(),
        }),
    })
}

/// Format an `f64` for a TSV cell. `0.0` emits literally as `"0"` so
/// the most common default is short; non-zero values use the
/// shortest precision-preserving form via `Display`. Cells must be
/// re-parseable by `f64::from_str`.
fn fmt_f64(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else {
        format!("{v}")
    }
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(fmt_f64).unwrap_or_default()
}

fn parse_f64_cell(col: &'static str, s: &str) -> Result<f64, SymbolFileError> {
    s.parse().map_err(|_| SymbolFileError::InvalidNumericCell {
        column: col,
        value: s.to_string(),
    })
}

fn parse_opt_f64_cell(col: &'static str, s: &str) -> Result<Option<f64>, SymbolFileError> {
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_f64_cell(col, s)?))
    }
}

fn parse_bool_cell(col: &'static str, s: &str) -> Result<bool, SymbolFileError> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(SymbolFileError::InvalidBoolCell {
            column: col,
            value: s.to_string(),
        }),
    }
}

fn pin_to_tsv_row(pin: &SymbolPin) -> Result<String, SymbolFileError> {
    let function_str = pin.function.join("|");
    let cells: [String; 20] = [
        pin.number.clone(),
        pin.name.clone(),
        pin_direction_token(pin.electrical).to_string(),
        fmt_f64(pin.position[0]),
        fmt_f64(pin.position[1]),
        pin_orientation_token(pin.orientation).to_string(),
        fmt_f64(pin.length),
        pin.description.clone(),
        function_str,
        fmt_opt_f64(pin.pin_package_length),
        fmt_opt_f64(pin.propagation_delay_ns),
        pin.designator_visible.to_string(),
        pin.name_visible.to_string(),
        pin_symbol_kind_token(pin.inside_symbol).to_string(),
        pin_symbol_kind_token(pin.inside_edge_symbol).to_string(),
        pin_symbol_kind_token(pin.outside_edge_symbol).to_string(),
        pin_symbol_kind_token(pin.outside_symbol).to_string(),
        pin.hidden.to_string(),
        pin.locked.to_string(),
        pin.part_number.to_string(),
    ];
    for (col, cell) in PIN_TSV_COLUMNS.iter().zip(cells.iter()) {
        if cell.contains('\t') || cell.contains('\n') || cell.contains("'''") {
            return Err(SymbolFileError::InvalidTsvCell {
                column: col,
                value: cell.clone(),
            });
        }
    }
    Ok(cells.join("\t"))
}

/// Encode a slice of pins as TSV — header row first, then one row
/// per pin. Empty slice still emits the header row so the round-trip
/// produces a parseable block.
pub(crate) fn pins_to_tsv(pins: &[SymbolPin]) -> Result<String, SymbolFileError> {
    let mut out = String::new();
    out.push_str(&PIN_TSV_COLUMNS.join("\t"));
    out.push('\n');
    for pin in pins {
        out.push_str(&pin_to_tsv_row(pin)?);
        out.push('\n');
    }
    Ok(out)
}

/// Parse a `pins_tsv` payload back into `Vec<SymbolPin>`. The first
/// non-empty line is the header and must equal [`PIN_TSV_COLUMNS`];
/// each subsequent line is a pin row.
pub(crate) fn pins_from_tsv(tsv: &str) -> Result<Vec<SymbolPin>, SymbolFileError> {
    let trimmed = tsv.trim_matches('\n');
    if trimmed.is_empty() {
        return Err(SymbolFileError::EmptyPinsTsv);
    }
    let mut lines = trimmed.split('\n');
    let header = lines.next().ok_or(SymbolFileError::EmptyPinsTsv)?;
    let header_cols: Vec<&str> = header.split('\t').collect();
    if header_cols.len() != PIN_TSV_COLUMNS.len()
        || header_cols
            .iter()
            .zip(PIN_TSV_COLUMNS.iter())
            .any(|(g, e)| g != e)
    {
        return Err(SymbolFileError::PinsTsvSchemaMismatch {
            got: header_cols.iter().map(|s| (*s).to_string()).collect(),
        });
    }
    let mut pins = Vec::new();
    for (row_idx, line) in lines.enumerate() {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != PIN_TSV_COLUMNS.len() {
            return Err(SymbolFileError::PinsTsvCellCountMismatch {
                row_index: row_idx,
                got: cells.len(),
                expected: PIN_TSV_COLUMNS.len(),
            });
        }
        pins.push(pin_from_tsv_row(&cells)?);
    }
    Ok(pins)
}

fn pin_from_tsv_row(cells: &[&str]) -> Result<SymbolPin, SymbolFileError> {
    let part_number_raw = cells[19];
    let part_number: u8 =
        part_number_raw
            .parse()
            .map_err(|_| SymbolFileError::InvalidNumericCell {
                column: "part_number",
                value: part_number_raw.to_string(),
            })?;
    Ok(SymbolPin {
        number: cells[0].to_string(),
        name: cells[1].to_string(),
        electrical: pin_direction_from_token(cells[2])?,
        position: [
            parse_f64_cell("pos_x", cells[3])?,
            parse_f64_cell("pos_y", cells[4])?,
        ],
        orientation: pin_orientation_from_token(cells[5])?,
        length: parse_f64_cell("length", cells[6])?,
        description: cells[7].to_string(),
        function: if cells[8].is_empty() {
            Vec::new()
        } else {
            cells[8].split('|').map(str::to_string).collect()
        },
        pin_package_length: parse_opt_f64_cell("pin_package_length", cells[9])?,
        propagation_delay_ns: parse_opt_f64_cell("propagation_delay_ns", cells[10])?,
        designator_visible: parse_bool_cell("designator_visible", cells[11])?,
        name_visible: parse_bool_cell("name_visible", cells[12])?,
        inside_symbol: pin_symbol_kind_from_token(cells[13])?,
        inside_edge_symbol: pin_symbol_kind_from_token(cells[14])?,
        outside_edge_symbol: pin_symbol_kind_from_token(cells[15])?,
        outside_symbol: pin_symbol_kind_from_token(cells[16])?,
        hidden: parse_bool_cell("hidden", cells[17])?,
        locked: parse_bool_cell("locked", cells[18])?,
        part_number,
    })
}

/// Error variants for [`SymbolFile`] parsers + serialisers.
#[derive(Debug, thiserror::Error)]
pub enum SymbolFileError {
    #[error("empty .snxsym file")]
    Empty,
    #[error("invalid UTF-8 in TOML payload: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("TOML deserialise failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("TOML serialise failed: {0}")]
    TomlSerialize(toml::ser::Error),
    #[error("unsupported .snxsym format token {got:?}; this build supports \"snxsym/v1\"")]
    UnsupportedFormat { got: String },
    #[error(
        "TSV cell in column {column:?} contains a tab, newline, or triple-quote: \
         {value:?}; cells must be free of \\t, \\n, and the literal \"'''\""
    )]
    InvalidTsvCell {
        column: &'static str,
        value: String,
    },
    #[error("pins_tsv block is empty (no header row)")]
    EmptyPinsTsv,
    #[error(
        "pins_tsv header does not match the expected schema; got columns {got:?}"
    )]
    PinsTsvSchemaMismatch { got: Vec<String> },
    #[error(
        "pins_tsv row {row_index} has {got} cells; header declares {expected}"
    )]
    PinsTsvCellCountMismatch {
        row_index: usize,
        got: usize,
        expected: usize,
    },
    #[error("unknown {kind} token {got:?} in pins_tsv cell")]
    UnknownEnumToken {
        kind: &'static str,
        got: String,
    },
    #[error("invalid numeric cell in column {column:?}: {value:?}")]
    InvalidNumericCell {
        column: &'static str,
        value: String,
    },
    #[error("invalid boolean cell in column {column:?}: {value:?} (expected \"true\" or \"false\")")]
    InvalidBoolCell {
        column: &'static str,
        value: String,
    },
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
                electrical: PinDirection::Output,
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
                part_number: 1,
            }],
            graphics: vec![SymbolGraphic {
                kind: SymbolGraphicKind::Rectangle {
                    from: [-2.5, -2.5],
                    to: [2.5, 2.5],
                },
                stroke_width: 0.15,
            }],
            schematic_params: ParamMap::new(),
            designator: "U?".into(),
            comment: "*".into(),
            description: String::new(),
            component_type: ComponentType::Standard,
            mirrored: false,
            local_fill_color: None,
            local_line_color: None,
            local_pin_color: None,
            version: "0.0.1".into(),
            released: false,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
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

    // ---- v0.18.4 — SymbolFile TOML+TSV round-trip + pin TSV codec ----

    #[test]
    fn symbol_file_toml_round_trip_empty_symbol() {
        // `Symbol::empty` carries one default pin — exercises the
        // header-plus-one-row TSV path.
        let s = Symbol::empty("Test");
        let original = SymbolFile::from_symbol(s.clone());
        let toml_text = original.to_toml_string().expect("serialise");
        let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.symbols.len(), 1);
        assert_eq!(back.symbols[0].name, "Test");
        assert_eq!(back.symbols[0].pins.len(), 1);
        assert_eq!(back.format, "snxsym/v1");
        assert_eq!(back.file_uuid, original.file_uuid);
    }

    #[test]
    fn symbol_file_toml_round_trip_multi() {
        let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
        file.symbols.push(Symbol::empty("B"));
        file.symbols.push(Symbol::empty("C"));
        let toml_text = file.to_toml_string().expect("serialise");
        let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.symbols.len(), 3);
        let names: Vec<&str> = back.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B", "C"]);
    }

    #[test]
    fn symbol_file_from_bytes_decodes_toml_envelope() {
        let mut file = SymbolFile::from_symbol(Symbol::empty("TOML-A"));
        file.symbols.push(Symbol::empty("TOML-B"));
        let toml_bytes = file.to_toml_string().unwrap().into_bytes();
        let back = SymbolFile::from_bytes(&toml_bytes).expect("parse");
        assert_eq!(back.symbols.len(), 2);
    }

    #[test]
    fn symbol_file_from_bytes_rejects_empty_payload() {
        match SymbolFile::from_bytes(b"   \n  \t\n") {
            Err(SymbolFileError::Empty) => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    /// All-fields round-trip — every SymbolPin field gets a non-default
    /// value so the TSV cell encoders / decoders are exercised end-to-end.
    #[test]
    fn symbol_file_round_trip_with_full_pin_payload() {
        let pin = SymbolPin {
            number: "VCC".into(),
            name: "Power".into(),
            electrical: PinDirection::Power,
            position: [-3.81, 5.08],
            orientation: PinOrientation::Up,
            length: 2.54,
            description: "main rail".into(),
            function: vec!["VDD".into(), "VCC_3V3".into()],
            pin_package_length: Some(1.5),
            propagation_delay_ns: Some(0.25),
            designator_visible: false,
            name_visible: true,
            inside_symbol: PinSymbolKind::Dot,
            inside_edge_symbol: PinSymbolKind::ClockEdge,
            outside_edge_symbol: PinSymbolKind::ActiveLowInput,
            outside_symbol: PinSymbolKind::SchmittTrigger,
            hidden: true,
            locked: true,
            part_number: 2,
        };
        let mut sym = Symbol::empty("PWR");
        sym.pins = vec![pin.clone()];
        let file = SymbolFile::from_symbol(sym);
        let toml_text = file.to_toml_string().expect("serialise");
        let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.symbols[0].pins.len(), 1);
        assert_eq!(back.symbols[0].pins[0], pin);
    }

    #[test]
    fn symbol_file_to_toml_emits_pins_as_literal_multiline() {
        // Output must contain the `pins_tsv = '''` opener — placeholder
        // post-processing landed.
        let s = Symbol::empty("Demo");
        let toml_text = SymbolFile::from_symbol(s).to_toml_string().unwrap();
        assert!(
            toml_text.contains("pins_tsv = '''"),
            "expected literal multi-line opener; got:\n{toml_text}"
        );
        // ... and no leftover placeholder string.
        assert!(
            !toml_text.contains(PINS_TSV_PLACEHOLDER_PREFIX),
            "placeholder should be fully replaced; got:\n{toml_text}"
        );
    }

    #[test]
    fn pins_to_tsv_empty_emits_header_only() {
        let tsv = pins_to_tsv(&[]).expect("serialise");
        // Header row terminated by a newline, no data rows.
        assert_eq!(tsv, format!("{}\n", PIN_TSV_COLUMNS.join("\t")));
    }

    #[test]
    fn pins_to_tsv_rejects_tab_in_cell() {
        let mut pin = SymbolPin::new("1", "name");
        pin.description = "tab\there".into();
        match pins_to_tsv(std::slice::from_ref(&pin)) {
            Err(SymbolFileError::InvalidTsvCell { column, .. }) => {
                assert_eq!(column, "description");
            }
            other => panic!("expected InvalidTsvCell, got {other:?}"),
        }
    }

    #[test]
    fn pins_to_tsv_rejects_newline_in_cell() {
        let mut pin = SymbolPin::new("1", "multi\nline");
        pin.description = String::new();
        match pins_to_tsv(std::slice::from_ref(&pin)) {
            Err(SymbolFileError::InvalidTsvCell { column, .. }) => {
                assert_eq!(column, "name");
            }
            other => panic!("expected InvalidTsvCell, got {other:?}"),
        }
    }

    #[test]
    fn pins_to_tsv_rejects_triple_quote_in_cell() {
        let mut pin = SymbolPin::new("1", "X");
        pin.description = "smuggle '''".into();
        match pins_to_tsv(std::slice::from_ref(&pin)) {
            Err(SymbolFileError::InvalidTsvCell { column, .. }) => {
                assert_eq!(column, "description");
            }
            other => panic!("expected InvalidTsvCell, got {other:?}"),
        }
    }

    #[test]
    fn pins_from_tsv_rejects_schema_mismatch() {
        // Header naming "wrong" columns triggers PinsTsvSchemaMismatch.
        let bad_tsv = "foo\tbar\tbaz\n1\t2\t3\n";
        match pins_from_tsv(bad_tsv) {
            Err(SymbolFileError::PinsTsvSchemaMismatch { got }) => {
                assert_eq!(got, vec!["foo", "bar", "baz"]);
            }
            other => panic!("expected PinsTsvSchemaMismatch, got {other:?}"),
        }
    }

    #[test]
    fn pins_from_tsv_rejects_cell_count_mismatch() {
        let header = PIN_TSV_COLUMNS.join("\t");
        // 5 cells in a 20-column schema.
        let body = format!("{header}\n1\tname\tInput\t0\t0\n");
        match pins_from_tsv(&body) {
            Err(SymbolFileError::PinsTsvCellCountMismatch {
                row_index,
                got,
                expected,
            }) => {
                assert_eq!(row_index, 0);
                assert_eq!(got, 5);
                assert_eq!(expected, PIN_TSV_COLUMNS.len());
            }
            other => panic!("expected PinsTsvCellCountMismatch, got {other:?}"),
        }
    }

    #[test]
    fn pin_direction_token_round_trip_all_variants() {
        for d in [
            PinDirection::Input,
            PinDirection::Output,
            PinDirection::Bidirectional,
            PinDirection::Power,
            PinDirection::Passive,
            PinDirection::OpenCollector,
            PinDirection::OpenEmitter,
            PinDirection::NotConnected,
            PinDirection::Tristate,
            PinDirection::Unspecified,
        ] {
            let token = pin_direction_token(d);
            let back = pin_direction_from_token(token).unwrap();
            assert_eq!(d, back);
        }
    }

    #[test]
    fn pin_orientation_token_round_trip_all_variants() {
        for o in [
            PinOrientation::Up,
            PinOrientation::Down,
            PinOrientation::Left,
            PinOrientation::Right,
        ] {
            let token = pin_orientation_token(o);
            let back = pin_orientation_from_token(token).unwrap();
            assert_eq!(o, back);
        }
    }

    #[test]
    fn pin_symbol_kind_token_round_trip_all_variants() {
        for k in [
            PinSymbolKind::None,
            PinSymbolKind::Dot,
            PinSymbolKind::ClockEdge,
            PinSymbolKind::ActiveLowInput,
            PinSymbolKind::ActiveLowOutput,
            PinSymbolKind::SchmittTrigger,
            PinSymbolKind::Analog,
            PinSymbolKind::Digital,
            PinSymbolKind::ShiftRight,
            PinSymbolKind::ShiftLeft,
            PinSymbolKind::Pi,
            PinSymbolKind::Sigma,
            PinSymbolKind::OpenCollector,
            PinSymbolKind::OpenEmitter,
            PinSymbolKind::HiZ,
        ] {
            let token = pin_symbol_kind_token(k);
            let back = pin_symbol_kind_from_token(token).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn symbol_file_unsupported_format_token_is_rejected() {
        let bad = r#"
format = "snxsym/99"
file_uuid = "00000000-0000-0000-0000-000000000000"
display_name = ""
created = "2026-05-04T00:00:00Z"
updated = "2026-05-04T00:00:00Z"
symbols = []
"#;
        match SymbolFile::from_toml_str(bad) {
            Err(SymbolFileError::UnsupportedFormat { got }) => {
                assert_eq!(got, "snxsym/99");
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn pin_electrical_type_round_trip_all_variants() {
        for t in [
            PinDirection::Input,
            PinDirection::Output,
            PinDirection::Bidirectional,
            PinDirection::Power,
            PinDirection::Passive,
            PinDirection::OpenCollector,
            PinDirection::OpenEmitter,
            PinDirection::NotConnected,
            PinDirection::Tristate,
            PinDirection::Unspecified,
        ] {
            let json = serde_json::to_string(&t).unwrap();
            let back: PinDirection = serde_json::from_str(&json).unwrap();
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
