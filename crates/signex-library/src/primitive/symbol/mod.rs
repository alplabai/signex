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
    /// Closed-loop polygon, outlined or filled — mirrors
    /// `FpGraphicKind::Polygon` (footprint primitive). The vertex
    /// list is closed implicitly (`vertices[N-1]` connects back to
    /// `vertices[0]` at render / hit-test time); callers never store
    /// a duplicated closing vertex.
    Polygon {
        vertices: Vec<[f64; 2]>,
    },
}

/// Normalise an `Arc`'s `start_deg`/`end_deg` into this codebase's
/// counter-clockwise-wraparound sweep convention: `start..end` sweeps
/// CCW (increasing angle) from `start`, wrapping through a full turn
/// when `end < start` — the convention the symbol canvas's arc
/// hit-test, its Arc rotate transform, and the GPU arc shader all
/// already assume. Returns the pair with a swap applied when needed
/// (see below), then both endpoints reduced into a canonical
/// `[0, 360)` range.
///
/// Why a swap and not just reducing each field into range: reducing
/// `start_deg`/`end_deg` independently only ever changes each by a
/// whole multiple of 360°, so it can't change `end_deg - start_deg`
/// by anything but a multiple of 360° either — the CCW-wraparound
/// sweep would stay exactly what it was. When the pair actually
/// represents a clockwise-signed drag rather than an intentionally
/// wrapped arc, that unchanged sweep is the WRONG one — the
/// complement of the arc that was meant. Swapping instead re-orders
/// the pair so its wraparound sweep becomes the complement
/// (`360° - old_sweep`), which is the short arc that was intended.
///
/// Used by two call sites that can't share a dependency edge: this
/// crate's own [`SymbolFile::from_toml_str`] (migrating legacy
/// `.snxsym` files saved by builds that stored a clockwise drag's
/// raw, unswapped pair) and `signex_app`'s Place Arc placement-commit
/// handler (the tool that can hand this a raw, possibly-negative pair
/// from a live drag). Lives here — signex-library must not depend on
/// signex-app — with signex-app calling into it, not the reverse.
pub fn normalize_arc_endpoints_deg(start_deg: f64, end_deg: f64) -> (f64, f64) {
    let (start_deg, end_deg) = if end_deg < start_deg {
        (end_deg, start_deg)
    } else {
        (start_deg, end_deg)
    };
    (start_deg.rem_euclid(360.0), end_deg.rem_euclid(360.0))
}

/// One graphic on the symbol body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolGraphic {
    pub kind: SymbolGraphicKind,
    /// Stroke width in mm (0.0 = use renderer default).
    #[serde(default)]
    pub stroke_width: f64,
    /// Optional solid fill colour (RGBA, 0–255). `None` (default) =
    /// unfilled outline-only shape; `Some(rgba)` = filled interior.
    /// Only meaningful for closed shapes (Rectangle / Circle /
    /// Polygon); ignored for Line / Arc / Text. Additive and back-
    /// compatible: files written before this field load as `None` and
    /// render exactly as before, so no `.snxsym` format-token bump is
    /// required.
    #[serde(default)]
    pub fill: Option<[u8; 4]>,
    /// Which sub-part (unit) this graphic belongs to. `0` (default)
    /// means "shared" — drawn on every part, matching pre-C1 files
    /// where body geometry carried no part scoping. `1..=N` scope the
    /// graphic to a specific unit. Additive and back-compatible:
    /// files written before this field load as `0` and render exactly
    /// as before, so no `.snxsym` format-token bump is required.
    #[serde(default)]
    pub part_number: u8,
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
    /// Number of sub-parts (Altium "parts" / units) this symbol
    /// declares. First-class so an empty unit persists across save —
    /// unit count is no longer derived from pins alone. Defaults to
    /// `1` for new symbols and legacy `.snxsym` files missing the
    /// field; loaders reconcile it upward against the highest pin
    /// `part_number` so old multi-part files stay intact.
    #[serde(default = "default_part_count")]
    pub part_count: u8,
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

fn default_part_count() -> u8 {
    1
}

impl Symbol {
    /// Empty symbol scaffold used by New Symbol flows.
    pub fn empty(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::now_v7(),
            name: name.into(),
            anchor: [0.0, 0.0],
            pins: Vec::new(),
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
            part_count: 1,
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
    /// Schema sentinel. [`SymbolFile::to_toml_string`] writes
    /// [`SYMBOL_FILE_FORMAT_TOKEN_V2`] ("snxsym/v2") when any symbol
    /// in the file contains a [`SymbolGraphicKind::Polygon`] graphic,
    /// else [`SYMBOL_FILE_FORMAT_TOKEN`] ("snxsym/v1") — see that
    /// constant's doc comment for why a new tagged variant needs a
    /// conditional bump when an additive field wouldn't.
    /// [`SymbolFile::from_toml_str`] accepts either token.
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

/// Written instead of [`SYMBOL_FILE_FORMAT_TOKEN`] when a file
/// contains a [`SymbolGraphicKind::Polygon`] graphic.
///
/// A `#[serde(default)]` FIELD addition (`SymbolGraphic::fill`,
/// `::part_number`, `Symbol::part_count`, …) is bidirectional-compat
/// and needs no token bump: an old build reading a new file just
/// hits the field's serde default, and a new build reading an old
/// file does the same — both directions parse cleanly, so nothing
/// about the format token needs to change.
///
/// A new tagged enum VARIANT is a different shape of change. serde's
/// externally-tagged `#[serde(tag = "kind")]` representation has no
/// concept of "unknown variant, use a default" — an old build that
/// doesn't know about `Polygon` hits a hard, opaque
/// "unknown variant `polygon`" deserialize error the moment it reads
/// one, not the clean [`SymbolFileError::UnsupportedFormat`] guard a
/// mismatched format token gives it. The compatibility is one-way: a
/// v2 file (which may contain a `Polygon`) is unreadable by an old
/// build, but a v1 file is still perfectly readable by a new one (all
/// the pre-Polygon variants round-trip unchanged) — so the bump only
/// needs to happen when a `Polygon` is actually present.
/// `to_toml_string` computes this per save, from content, not from
/// whatever token the file happened to load with — a file that later
/// loses its only `Polygon` (e.g. deleted, or joined back into lines)
/// self-heals down to v1 on its next save, maximising how many files
/// stay readable by builds that predate this variant.
///
/// The planned multi-unit format bump (epic #289, C1) can fold into
/// this same v2 token rather than minting a v3 — one conditional
/// token per actually-incompatible addition, not one per PR.
const SYMBOL_FILE_FORMAT_TOKEN_V2: &str = "snxsym/v2";

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
    #[serde(default = "default_part_count")]
    part_count: u8,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
}

/// Load-time self-heal for two pre-normalization `Arc`-authoring bugs.
/// Non-`Arc` kinds pass through untouched.
///
/// 1. **Full-turn arcs vanish.** A raw span (`end_deg - start_deg`)
///    that's an exact, nonzero multiple of 360° — legitimate legacy
///    full-circle authoring (`0 -> 360`), or a placement/rotation
///    drift that happened to land on exactly one full turn — computes
///    a zero CCW-wraparound sweep and draws nothing. A 360° arc IS a
///    circle, so it's converted to one: [`SymbolGraphicKind::Circle`]
///    with the same `center`/`radius`. Checked, and applied, BEFORE
///    rule 2 below: an exact-360° span also happens to satisfy rule
///    2's discriminator (e.g. `30 -> -330`), and running it through
///    the swap-and-`rem_euclid` there would collapse it to
///    `start == end` — the exact invisible-point degenerate this
///    conversion exists to avoid — instead of the correct circle.
///
/// 2. **Legacy CW-signed arcs render as their complement.** Before
///    this codebase's CPU canvas draw path adopted the CCW-wraparound
///    sweep convention (matching what hit-test / the GPU shader
///    always assumed), the Place Arc tool could commit a clockwise
///    drag's raw, unswapped pair — e.g. `start: 30, end: -60` — which
///    the old (signed-sweep) CPU draw rendered as the short 90° arc
///    the user actually saw and clicked to place. Reading that same
///    stored pair under the CCW-wraparound convention sweeps the
///    270° complement instead. The discriminator distinguishes this
///    from a pre-existing, INTENTIONALLY wrapped pair (which
///    `rotation.rs`'s Arc rotate transform has always been able to
///    produce, e.g. rotating a 0°-crossing arc): a negative or
///    unwrapped-past-a-full-turn endpoint can only come from the
///    placement tool's raw drag delta, never from `rotation.rs`,
///    which always normalizes both endpoints into `[0, 360)`. A
///    wrapped pair with BOTH endpoints already in `[0, 360)` is left
///    unchanged — that's the rotation-produced, already-correct
///    wraparound form.
fn migrate_legacy_arc(kind: SymbolGraphicKind) -> SymbolGraphicKind {
    let SymbolGraphicKind::Arc {
        center,
        radius,
        start_deg,
        end_deg,
    } = kind
    else {
        return kind;
    };
    let raw_span = end_deg - start_deg;
    if raw_span.abs() > 1e-9 && raw_span.rem_euclid(360.0).abs() < 1e-6 {
        return SymbolGraphicKind::Circle { center, radius };
    }
    if end_deg < start_deg && (end_deg < 0.0 || start_deg < 0.0 || raw_span <= -360.0) {
        let (start_deg, end_deg) = normalize_arc_endpoints_deg(start_deg, end_deg);
        return SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        };
    }
    SymbolGraphicKind::Arc {
        center,
        radius,
        start_deg,
        end_deg,
    }
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

    /// Parse the TOML+TSV wire format. The format-token check accepts
    /// either [`SYMBOL_FILE_FORMAT_TOKEN`] or
    /// [`SYMBOL_FILE_FORMAT_TOKEN_V2`]; any other token surfaces
    /// [`SymbolFileError::UnsupportedFormat`].
    ///
    /// Every loaded `Arc` graphic passes through
    /// [`migrate_legacy_arc`], which self-heals two pre-normalization
    /// authoring bugs in stored `start_deg`/`end_deg` pairs — see that
    /// function's doc comment. This runs on load (not just for v1
    /// files) so every consumer of a `Symbol` gets already-migrated
    /// data regardless of entry point, and the next save re-emits the
    /// corrected values, healing the file on disk too.
    pub fn from_toml_str(text: &str) -> Result<Self, SymbolFileError> {
        let wire: SymbolFileWire = toml::from_str(text)?;
        if wire.format != SYMBOL_FILE_FORMAT_TOKEN && wire.format != SYMBOL_FILE_FORMAT_TOKEN_V2 {
            return Err(SymbolFileError::UnsupportedFormat { got: wire.format });
        }
        let mut symbols = Vec::with_capacity(wire.symbols.len());
        for sw in wire.symbols {
            let pins = pins_from_tsv(&sw.pins_tsv)?;
            // Reconcile the declared part count upward against the
            // highest pin part so legacy multi-part files (written
            // before `part_count` existed, defaulting to 1) keep all
            // their units instead of collapsing to one.
            let pin_max = pins.iter().map(|p| p.part_number).max().unwrap_or(1).max(1);
            let graphics = sw
                .graphics
                .into_iter()
                .map(|g| SymbolGraphic {
                    kind: migrate_legacy_arc(g.kind),
                    ..g
                })
                .collect();
            symbols.push(Symbol {
                uuid: sw.uuid,
                name: sw.name,
                anchor: sw.anchor,
                pins,
                graphics,
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
                part_count: sw.part_count.max(pin_max),
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
    ///
    /// The written `format` token is computed fresh from content —
    /// [`SYMBOL_FILE_FORMAT_TOKEN_V2`] iff any symbol contains a
    /// [`SymbolGraphicKind::Polygon`] graphic, else
    /// [`SYMBOL_FILE_FORMAT_TOKEN`] — not copied from `self.format`
    /// (which may be stale, e.g. right after loading a v1 file that
    /// has since gained a `Polygon` in this session, or a v2 file
    /// whose only `Polygon` was just deleted). See
    /// [`SYMBOL_FILE_FORMAT_TOKEN_V2`]'s doc comment for why this
    /// stays maximally backward-compatible.
    pub fn to_toml_string(&self) -> Result<String, SymbolFileError> {
        let format = if self
            .symbols
            .iter()
            .flat_map(|s| &s.graphics)
            .any(|g| matches!(g.kind, SymbolGraphicKind::Polygon { .. }))
        {
            SYMBOL_FILE_FORMAT_TOKEN_V2.to_string()
        } else {
            SYMBOL_FILE_FORMAT_TOKEN.to_string()
        };
        let mut tsv_payloads: Vec<String> = Vec::with_capacity(self.symbols.len());
        let mut wire_symbols: Vec<SymbolWire> = Vec::with_capacity(self.symbols.len());
        for (idx, sym) in self.symbols.iter().enumerate() {
            let payload = pins_to_tsv(&sym.pins)?;
            // MD-27: refuse to emit a TSV payload that contains the
            // post-emit-replace sentinel — `String::replace` is
            // non-anchored, so a payload byte sequence matching the
            // placeholder would get a SECOND replacement applied,
            // corrupting the output. Mirrors the guard in `SimFile`.
            if payload.contains(PINS_TSV_PLACEHOLDER_PREFIX) {
                return Err(SymbolFileError::InvalidPinsTsv { symbol_index: idx });
            }
            tsv_payloads.push(payload);
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
                part_count: sym.part_count,
                created: sym.created,
                updated: sym.updated,
            });
        }
        let wire = SymbolFileWire {
            format,
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
    #[error(
        "unsupported .snxsym format token {got:?}; this build supports \"snxsym/v1\" or \"snxsym/v2\""
    )]
    UnsupportedFormat { got: String },
    #[error(
        "TSV cell in column {column:?} contains a tab, newline, or triple-quote: \
         {value:?}; cells must be free of \\t, \\n, and the literal \"'''\""
    )]
    InvalidTsvCell { column: &'static str, value: String },
    #[error("pins_tsv block is empty (no header row)")]
    EmptyPinsTsv,
    #[error("pins_tsv header does not match the expected schema; got columns {got:?}")]
    PinsTsvSchemaMismatch { got: Vec<String> },
    #[error("pins_tsv row {row_index} has {got} cells; header declares {expected}")]
    PinsTsvCellCountMismatch {
        row_index: usize,
        got: usize,
        expected: usize,
    },
    #[error("unknown {kind} token {got:?} in pins_tsv cell")]
    UnknownEnumToken { kind: &'static str, got: String },
    #[error("invalid numeric cell in column {column:?}: {value:?}")]
    InvalidNumericCell { column: &'static str, value: String },
    #[error(
        "invalid boolean cell in column {column:?}: {value:?} (expected \"true\" or \"false\")"
    )]
    InvalidBoolCell { column: &'static str, value: String },
    /// MD-27: pin TSV payload contains the placeholder sentinel that
    /// `to_toml_string` uses for the post-emit `String::replace` pass.
    /// Mirrors `SimFileError::InvalidBody` — output corruption guard.
    #[error("symbol {symbol_index} pin TSV contains the placeholder sentinel")]
    InvalidPinsTsv { symbol_index: usize },
}

mod chain;
#[cfg(test)]
mod chain_tests;
mod serde_tsv;
#[cfg(test)]
mod tests;
mod to_lib_symbol;
#[cfg(test)]
mod to_lib_symbol_tests;

pub use chain::{
    CHAIN_ARC_SAMPLES, CHAIN_ENDPOINT_EPSILON_MM, ChainError, ChainSegment,
    chain_into_closed_contour,
};
pub(crate) use serde_tsv::{pins_from_tsv, pins_to_tsv};
