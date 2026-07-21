//! Pure conversion: a library-authored [`Symbol`] (`.snxsym`) into a
//! schematic [`LibSymbol`] (`signex_types::schematic`).
//!
//! This is **part 1 of 2** of issue #365 — no I/O, no app dependency, and
//! nothing here wires the result into the place flow or `PlaceSymbol`
//! (that is part 2). `LibSymbol::id` is a caller-supplied argument; this
//! module does not invent an id scheme.
//!
//! ## Coordinates — no y-flip here
//!
//! `.snxsym` positions are y-up mm (`crates/signex-app/src/library/editor/
//! symbol/canvas/geometry.rs`), and `LibPin.pin.position` / `LibGraphic`
//! points are already y-up library space too — the flip to y-down
//! schematic space happens later, only inside
//! [`signex_types::schematic::SymbolTransform::apply`]. Every position
//! here carries over unchanged.
//!
//! ## Pin rotation convention (load-bearing — the two sides disagree in y)
//!
//! `.snxsym`'s [`PinOrientation`] is the tip→body direction as a CCW angle
//! from +x in that same y-up space: `Right = 0°`, `Up = +90°`,
//! `Left = 180°`, `Down = -90°` — see `PinRenderGeometry::compute` in
//! `crates/signex-app/src/library/editor/symbol/canvas/pins.rs`, where
//! `tip = pin.position` and `body_end = tip + unit(angle_rad) * length`.
//!
//! `LibPin.pin.rotation` extends the pin stub using the OPPOSITE sign in
//! y: `crates/signex-widgets/src/symbol_preview.rs`'s `bounds()` computes
//! the far end as `0° => (+len, 0)`, `90° => (0, -len)`, `180° => (-len,
//! 0)`, `270° => (0, +len)` — i.e. its `90°` extends toward **-y**, not
//! +y. Since positions are not flipped (previous section), naively
//! copying the source angle (`Up -> 90`) would draw the pin extending
//! downward instead of up. The x-axis orientations already agree between
//! the two conventions (`Right -> 0°`, `Left -> 180°` extend the same way
//! in both), so only the y-axis orientations are swapped: `Up -> 270°`,
//! `Down -> 90°`. [`to_lib_symbol_tests`] pins all four.
//!
//! ## Pin direction — total, no panic
//!
//! [`PinDirection`] (10 variants, symbol-editor-curated) and
//! `signex_types::schematic::PinDirection` (14 variants, independently
//! curated) are two separate sets with no exact 1:1 twin for several
//! source variants. [`pin_direction`] documents every non-obvious choice
//! inline; every source variant is pinned by a test.
//!
//! ## Pin glyphs — lossy by construction
//!
//! `.snxsym` carries four independent glyph slots per pin
//! (`inside_symbol`, `inside_edge_symbol`, `outside_edge_symbol`,
//! `outside_symbol`), each a 15-variant [`PinSymbolKind`]. The schematic
//! side has one flat 7-variant `PinShapeStyle` per pin. `outside_edge_symbol`
//! is chosen as the authoritative slot — see [`pin_shape_style`].
//!
//! ## Fill — colour is dropped
//!
//! `SymbolGraphic::fill: Option<[u8; 4]>` becomes `FillType`, which
//! carries no colour: `None -> FillType::None`, `Some(_) ->
//! FillType::Background`. The RGBA value itself has nowhere to land.

use signex_types::schematic::{
    FillType, Graphic, HAlign, LibGraphic, LibPin, LibSymbol, Pin,
    PinDirection as SchematicPinDirection, PinShapeStyle, Point, VAlign,
};

use super::{
    PinDirection, PinOrientation, PinSymbolKind, Symbol, SymbolGraphic, SymbolGraphicKind,
    SymbolPin,
};

impl Symbol {
    /// Convert this library `Symbol` into a schematic `LibSymbol`.
    ///
    /// `id` is caller-supplied (e.g. a library-relative lookup key) —
    /// this function does not invent an id scheme; that is part 2's job.
    ///
    /// Header fields: `designator -> reference`, `comment -> value`,
    /// `description -> description`. Fields `LibSymbol` has that `Symbol`
    /// carries no equivalent for (`footprint`, `datasheet`, `keywords`,
    /// `fp_filters`, the `in_bom`/`on_board`/`in_pos_files` visibility
    /// flags, `duplicate_pin_numbers_are_jumpers`, the pin-number/name
    /// display toggles, `pin_name_offset`) take `LibSymbol`'s own
    /// defaults — this Symbol type simply has no source data for them.
    pub fn to_lib_symbol(&self, id: impl Into<String>) -> LibSymbol {
        LibSymbol {
            id: id.into(),
            reference: self.designator.clone(),
            value: self.comment.clone(),
            footprint: String::new(),
            datasheet: String::new(),
            description: self.description.clone(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: self.graphics.iter().map(lib_graphic_from).collect(),
            pins: self.pins.iter().map(lib_pin_from).collect(),
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        }
    }
}

/// `SymbolPin.part_number` (Altium-style: `0` = Part Zero, shared across
/// every unit) maps straight onto `LibPin.unit` — consumers already read
/// `unit == 0` as "common to all units"
/// (`crates/signex-net/src/build/mod.rs`, `lp.unit != 0 && lp.unit !=
/// sym.unit`).
fn lib_pin_from(pin: &SymbolPin) -> LibPin {
    LibPin {
        unit: u32::from(pin.part_number),
        body_style: 1,
        pin: Pin {
            direction: pin_direction(pin.electrical),
            shape_style: pin_shape_style(pin.outside_edge_symbol),
            position: Point::new(pin.position[0], pin.position[1]),
            rotation: pin_rotation_deg(pin.orientation),
            length: pin.length,
            name: pin.name.clone(),
            number: pin.number.clone(),
            visible: !pin.hidden,
            name_visible: pin.name_visible,
            number_visible: pin.designator_visible,
        },
    }
}

/// See the module doc's "Pin rotation convention" section for the full
/// derivation of why `Up`/`Down` swap while `Right`/`Left` pass through.
///
/// `PinOrientation` is `#[non_exhaustive]` for downstream crates, but
/// this match lives in the crate that defines it, so it stays exhaustive
/// with no wildcard arm — a future variant added to the enum fails this
/// match at compile time instead of silently falling through.
fn pin_rotation_deg(orientation: PinOrientation) -> f64 {
    match orientation {
        PinOrientation::Right => 0.0,
        PinOrientation::Up => 270.0,
        PinOrientation::Left => 180.0,
        PinOrientation::Down => 90.0,
    }
}

/// Total, panic-free map from the library's 10-variant [`PinDirection`]
/// to the schematic's independently-curated 14-variant
/// `signex_types::schematic::PinDirection`. Every source variant is
/// pinned by a test in [`super::to_lib_symbol_tests`].
///
/// `PinDirection` is `#[non_exhaustive]` for downstream crates, but this
/// match lives in the crate that defines it, so it stays exhaustive with
/// no wildcard arm — a future variant fails this match at compile time
/// instead of silently defaulting.
fn pin_direction(source: PinDirection) -> SchematicPinDirection {
    match source {
        PinDirection::Input => SchematicPinDirection::Input,
        PinDirection::Output => SchematicPinDirection::Output,
        PinDirection::Bidirectional => SchematicPinDirection::Bidirectional,
        // `Power` doesn't distinguish supply direction the way the
        // schematic side does (PowerInput / PowerOutput / GroundReference).
        // The overwhelming common case for a symbol-authored "Power" pin
        // is a supply INPUT (VCC/VDD on an IC), not a supply source
        // (regulator/battery output) or a dedicated ground reference, so
        // PowerInput is the safe default.
        PinDirection::Power => SchematicPinDirection::PowerInput,
        PinDirection::Passive => SchematicPinDirection::Passive,
        // Open-collector is an active-low, open-drain-style output.
        PinDirection::OpenCollector => SchematicPinDirection::OpenDrainLow,
        // Open-emitter is the active-high counterpart.
        PinDirection::OpenEmitter => SchematicPinDirection::OpenDrainHigh,
        PinDirection::NotConnected => SchematicPinDirection::DoNotConnect,
        PinDirection::Tristate => SchematicPinDirection::ThreeStatable,
        PinDirection::Unspecified => SchematicPinDirection::Unclassified,
    }
}

/// `outside_edge_symbol` is the chosen authoritative glyph slot: its own
/// doc comment (`SymbolPin::outside_edge_symbol`) already calls it out as
/// the slot that "most commonly carries the inverted-pin dot" — the
/// modifier that matters electrically (bubble / clock edge). The other
/// three slots (`inside_symbol`, `inside_edge_symbol`, `outside_symbol`)
/// are dropped entirely; they only ever layer decorative glyphs the flat
/// `PinShapeStyle` has no room for anyway.
///
/// `PinSymbolKind` is `#[non_exhaustive]` for downstream crates, but this
/// match lives in the crate that defines it, so it stays exhaustive with
/// no wildcard arm — a future glyph fails this match at compile time
/// instead of silently degrading.
fn pin_shape_style(outside_edge_symbol: PinSymbolKind) -> PinShapeStyle {
    match outside_edge_symbol {
        PinSymbolKind::None => PinShapeStyle::Plain,
        PinSymbolKind::Dot => PinShapeStyle::InvertedBubble,
        PinSymbolKind::ClockEdge => PinShapeStyle::ClockTriangle,
        // Chevron markers carry the same active-low/active-high SEMANTIC
        // as the dot bubble; the target style set has no distinct chevron
        // shape, so both fold into the closest available glyph.
        PinSymbolKind::ActiveLowInput | PinSymbolKind::ActiveLowOutput => {
            PinShapeStyle::InvertedBubble
        }
        PinSymbolKind::SchmittTrigger => PinShapeStyle::Schmitt,
        // No matching shape for the rest in the 7-variant target set.
        // Where an electrical meaning exists (open-collector/-emitter,
        // tri-state) it already survives via `pin_direction`
        // (`OpenDrainLow`/`OpenDrainHigh`/`ThreeStatable`), so the glyph
        // itself safely degrades to `Plain` rather than picking an
        // unrelated shape.
        PinSymbolKind::Analog
        | PinSymbolKind::Digital
        | PinSymbolKind::ShiftRight
        | PinSymbolKind::ShiftLeft
        | PinSymbolKind::Pi
        | PinSymbolKind::Sigma
        | PinSymbolKind::OpenCollector
        | PinSymbolKind::OpenEmitter
        | PinSymbolKind::HiZ => PinShapeStyle::Plain,
    }
}

/// `SymbolGraphic.part_number` maps straight onto `LibGraphic.unit` — see
/// [`lib_pin_from`]'s doc comment for the shared `unit == 0` = "common to
/// every unit" contract.
fn lib_graphic_from(graphic: &SymbolGraphic) -> LibGraphic {
    LibGraphic {
        unit: u32::from(graphic.part_number),
        body_style: 1,
        graphic: to_graphic(graphic),
    }
}

fn fill_type(fill: Option<[u8; 4]>) -> FillType {
    // `FillType` is flag-only (`None` / `Outline` / `Background`) with no
    // colour channel, so the RGBA value itself is dropped here — comment
    // the loss rather than inventing a colour field `FillType` doesn't
    // have. `Outline` is never produced by this conversion: `.snxsym`'s
    // `fill: Option<[u8; 4]>` only ever distinguishes unfilled vs filled,
    // so `Some(_)` always becomes `Background`.
    match fill {
        None => FillType::None,
        Some(_) => FillType::Background,
    }
}

/// Point on a circle at angle `deg`, `center + radius * (cos, sin)` —
/// the same "standard math convention, no axis flips" this crate's own
/// `chain.rs` documents and relies on for `SymbolGraphicKind::Arc`.
fn point_at_deg(center: [f64; 2], radius: f64, deg: f64) -> Point {
    let rad = deg.to_radians();
    Point::new(
        center[0] + radius * rad.cos(),
        center[1] + radius * rad.sin(),
    )
}

/// `SymbolGraphicKind::Arc`'s `start_deg..end_deg` always sweeps
/// counter-clockwise, wrapping a full turn when `end_deg < start_deg`
/// (the same convention `chain.rs` and `normalize_arc_endpoints_deg`
/// document/enforce). `Graphic::Arc` has no angle fields at all — just
/// three Cartesian points — so the midpoint angle along that same CCW
/// sweep is what stands in for the missing "which way does it bulge"
/// information.
fn arc_points(
    center: [f64; 2],
    radius: f64,
    start_deg: f64,
    end_deg: f64,
) -> (Point, Point, Point) {
    let sweep = (end_deg - start_deg).rem_euclid(360.0);
    let mid_deg = start_deg + sweep / 2.0;
    (
        point_at_deg(center, radius, start_deg),
        point_at_deg(center, radius, mid_deg),
        point_at_deg(center, radius, end_deg),
    )
}

fn to_graphic(graphic: &SymbolGraphic) -> Graphic {
    let width = graphic.stroke_width;
    let fill = fill_type(graphic.fill);
    match &graphic.kind {
        SymbolGraphicKind::Line { from, to } => Graphic::Polyline {
            points: vec![Point::new(from[0], from[1]), Point::new(to[0], to[1])],
            width,
            fill,
        },
        SymbolGraphicKind::Rectangle { from, to } => Graphic::Rectangle {
            start: Point::new(from[0], from[1]),
            end: Point::new(to[0], to[1]),
            width,
            fill,
        },
        SymbolGraphicKind::Circle { center, radius } => Graphic::Circle {
            center: Point::new(center[0], center[1]),
            radius: *radius,
            width,
            fill,
        },
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let (start, mid, end) = arc_points(*center, *radius, *start_deg, *end_deg);
            Graphic::Arc {
                start,
                mid,
                end,
                width,
                fill,
            }
        }
        SymbolGraphicKind::Text {
            position,
            content,
            size,
        } => Graphic::Text {
            text: content.clone(),
            position: Point::new(position[0], position[1]),
            rotation: 0.0,
            font_size: *size,
            bold: false,
            italic: false,
            justify_h: HAlign::default(),
            justify_v: VAlign::default(),
        },
        // Not in the issue's explicit kind list (added to `Symbol` after
        // that list was written) but `SymbolGraphicKind` is NOT
        // `#[non_exhaustive]`, so this match must cover it. A `Polygon`'s
        // vertex ring is closed implicitly (the last vertex connects back
        // to the first, per `SymbolGraphicKind::Polygon`'s own doc
        // comment) while `Graphic::Polyline` draws only the segments
        // between the points it's given — so the first vertex is
        // repeated at the end to make the closing edge explicit.
        SymbolGraphicKind::Polygon { vertices } => {
            let mut points: Vec<Point> = vertices.iter().map(|v| Point::new(v[0], v[1])).collect();
            if let Some(&first) = vertices.first() {
                points.push(Point::new(first[0], first[1]));
            }
            Graphic::Polyline {
                points,
                width,
                fill,
            }
        }
    }
}
