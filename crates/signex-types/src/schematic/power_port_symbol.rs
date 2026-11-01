//! Built-in body of a power port that carries no library entry.
//!
//! The Active Bar registers [`power_port_lib_symbol`] on the sheet when it
//! places a port, but older files and pasted ports have none.
//! [`SchematicSheet::resolve_lib_symbol`] gives every reader (netlist, ERC,
//! canvas, SVG/PDF) that same built-in entry for such a port, so it connects
//! and draws the same whether or not the sheet carries the entry.

use std::borrow::Cow;

use super::{
    FillType, Graphic, HAlign, LibGraphic, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle,
    Point, SCHEMATIC_TEXT_MM, SchematicSheet, Symbol, TextProp, VAlign,
};

/// Length of the stem from the connection point to the symbol head, mm.
const STEM_MM: f64 = 1.27;
/// Half width of the head, mm.
const HEAD_HALF_MM: f64 = 0.635;

/// Style of a power port, read from its `lib_id`. This one classifier is
/// shared by the built-in body below and the Properties panel's Style row,
/// so the style the panel shows is always the shape that is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerPortStyle {
    Bar,
    Arrow,
    Wave,
    Circle,
    PowerGround,
    SignalGround,
    Earth,
}

impl PowerPortStyle {
    /// Every style, in the order the Properties panel lists them.
    pub const ALL: [Self; 7] = [
        Self::Bar,
        Self::Arrow,
        Self::Wave,
        Self::Circle,
        Self::PowerGround,
        Self::SignalGround,
        Self::Earth,
    ];

    /// The style a `lib_id` names. Anything unrecognised, such as
    /// `power:+5V` or `power:VCC`, is a bar.
    #[must_use]
    pub fn of(lib_id: &str) -> Self {
        let lid = lib_id.to_lowercase();
        if lid.contains("gndref") {
            Self::SignalGround
        } else if lid.contains("earth") {
            Self::Earth
        } else if lid.contains("gnd") || lid.contains("ground") {
            Self::PowerGround
        } else if lid.contains("arrow") {
            Self::Arrow
        } else if lid.contains("wave") {
            Self::Wave
        } else if lid.contains("circle") {
            Self::Circle
        } else {
            Self::Bar
        }
    }

    /// The name the Properties panel shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Bar => "Bar",
            Self::Arrow => "Arrow",
            Self::Wave => "Wave",
            Self::Circle => "Circle",
            Self::PowerGround => "Power Ground",
            Self::SignalGround => "Signal Ground",
            Self::Earth => "Earth",
        }
    }

    /// The `power:<tag>` keyword that selects this style.
    #[must_use]
    pub fn lib_tag(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Arrow => "arrow",
            Self::Wave => "wave",
            Self::Circle => "circle",
            Self::PowerGround => "GND",
            Self::SignalGround => "GNDREF",
            Self::Earth => "Earth",
        }
    }

    /// Whether the body hangs below the connection point (the grounds)
    /// rather than rising above it.
    #[must_use]
    pub fn hangs_down(self) -> bool {
        matches!(self, Self::PowerGround | Self::SignalGround | Self::Earth)
    }
}

fn line(a: (f64, f64), b: (f64, f64)) -> Graphic {
    Graphic::Polyline {
        points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
        width: 0.0,
        fill: FillType::None,
    }
}

/// Library-space graphics of the power port style named by `lib_id`: the stem
/// from the connection point plus the head.
#[must_use]
pub fn power_port_graphics(lib_id: &str) -> Vec<Graphic> {
    let style = PowerPortStyle::of(lib_id);
    let dir = if style.hangs_down() { -1.0 } else { 1.0 };
    let mut out = vec![line((0.0, 0.0), (0.0, dir * STEM_MM))];
    out.extend(power_port_head(style));
    out
}

/// The head of a style, without the stem (the stem is the library pin).
fn power_port_head(style: PowerPortStyle) -> Vec<Graphic> {
    let top = STEM_MM;
    match style {
        PowerPortStyle::PowerGround => {
            let mut out = Vec::new();
            for (i, half) in [0.9525, 0.6, 0.25].into_iter().enumerate() {
                let y = -top - 0.5 * i as f64;
                out.push(line((-half, y), (half, y)));
            }
            out
        }
        PowerPortStyle::Bar => vec![line((-HEAD_HALF_MM, top), (HEAD_HALF_MM, top))],
        PowerPortStyle::Arrow => vec![Graphic::Polyline {
            points: vec![
                Point::new(-HEAD_HALF_MM, top),
                Point::new(HEAD_HALF_MM, top),
                Point::new(0.0, top + 1.0),
                Point::new(-HEAD_HALF_MM, top),
            ],
            width: 0.0,
            fill: FillType::Outline,
        }],
        PowerPortStyle::Circle => vec![Graphic::Circle {
            center: Point::new(0.0, top + HEAD_HALF_MM),
            radius: HEAD_HALF_MM,
            width: 0.0,
            fill: FillType::None,
        }],
        PowerPortStyle::SignalGround => vec![Graphic::Polyline {
            points: vec![
                Point::new(-0.9525, -top),
                Point::new(0.9525, -top),
                Point::new(0.0, -top - 1.0),
                Point::new(-0.9525, -top),
            ],
            width: 0.0,
            fill: FillType::None,
        }],
        PowerPortStyle::Earth => {
            let mut out = vec![line((-0.9525, -top), (0.9525, -top))];
            for x in [-0.9525, 0.0, 0.9525] {
                out.push(line((x, -top), (x - 0.5, -top - 0.6)));
            }
            out
        }
        PowerPortStyle::Wave => {
            let y = top + 0.4;
            vec![Graphic::Polyline {
                points: (0..=8)
                    .map(|i| {
                        let t = i as f64 / 8.0;
                        Point::new(
                            (t - 0.5) * 2.0 * HEAD_HALF_MM,
                            y + 0.3 * (t * std::f64::consts::TAU).sin(),
                        )
                    })
                    .collect(),
                width: 0.0,
                fill: FillType::None,
            }]
        }
    }
}

/// Gap between the far edge of a power port's head and its value text, mm.
const VALUE_GAP_MM: f64 = 0.254;

/// How far the head of `style` reaches past the connection point, mm.
fn head_reach_mm(style: PowerPortStyle) -> f64 {
    match style {
        PowerPortStyle::Bar => STEM_MM,
        PowerPortStyle::Arrow | PowerPortStyle::PowerGround | PowerPortStyle::SignalGround => {
            STEM_MM + 1.0
        }
        PowerPortStyle::Circle => STEM_MM + 2.0 * HEAD_HALF_MM,
        PowerPortStyle::Wave => STEM_MM + 0.7,
        PowerPortStyle::Earth => STEM_MM + 0.6,
    }
}

/// Where the value text of a power port of `lib_id`'s style, placed at `at`,
/// sits: centred beyond the far end of the head (above a rising head, below a
/// hanging one) so the text never touches the head or the wire the port
/// hangs on.
#[must_use]
pub fn power_port_value_field(lib_id: &str, at: Point) -> TextProp {
    let style = PowerPortStyle::of(lib_id);
    let reach = head_reach_mm(style) + VALUE_GAP_MM;
    // Schematic space is Y-down: a rising head is at smaller y.
    let (dy, justify_v) = if style.hangs_down() {
        (reach, VAlign::Top)
    } else {
        (-reach, VAlign::Bottom)
    };
    TextProp {
        position: Point::new(at.x, at.y + dy),
        rotation: 0.0,
        font_size: SCHEMATIC_TEXT_MM,
        justify_h: HAlign::Center,
        justify_v,
        hidden: false,
        font_family: None,
    }
}

/// The library entry a power port of `lib_id`'s style is placed with: the
/// head as body graphics and one visible pin at the connection point whose
/// stub is the stem, so a pin-reading consumer (netlist, ERC, SVG) sees the
/// same body the canvas draws. The entry is keyed by `lib_id`.
#[must_use]
pub fn power_port_lib_symbol(lib_id: &str) -> LibSymbol {
    let style = PowerPortStyle::of(lib_id);
    LibSymbol {
        id: lib_id.to_owned(),
        reference: "#PWR".to_owned(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: format!("{} power port", style.label()),
        keywords: "power".to_owned(),
        fp_filters: String::new(),
        in_bom: false,
        on_board: false,
        in_pos_files: false,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: power_port_head(style)
            .into_iter()
            .map(|graphic| LibGraphic {
                unit: 0,
                body_style: 1,
                graphic,
            })
            .collect(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::PowerInput,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                // Library space is Y-up: 90 degrees points the stub up.
                rotation: if style.hangs_down() { 270.0 } else { 90.0 },
                length: STEM_MM,
                name: String::new(),
                number: "1".to_owned(),
                visible: true,
                name_visible: false,
                number_visible: false,
            },
        }],
        show_pin_numbers: false,
        show_pin_names: false,
        pin_name_offset: 0.0,
    }
}

impl SchematicSheet {
    /// The library entry `sym` is read from: the sheet's own entry, else the
    /// built-in [`power_port_lib_symbol`] for an `is_power` symbol, else none.
    #[must_use]
    pub fn resolve_lib_symbol(&self, sym: &Symbol) -> Option<Cow<'_, LibSymbol>> {
        match self.lib_symbols.get(&sym.lib_id) {
            Some(lib) => Some(Cow::Borrowed(lib)),
            None if sym.is_power => Some(Cow::Owned(power_port_lib_symbol(&sym.lib_id))),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_comes_from_the_lib_id() {
        assert_eq!(PowerPortStyle::of("power:GND"), PowerPortStyle::PowerGround);
        assert_eq!(
            PowerPortStyle::of("power:GNDREF"),
            PowerPortStyle::SignalGround
        );
        assert_eq!(PowerPortStyle::of("power:Earth"), PowerPortStyle::Earth);
        assert_eq!(PowerPortStyle::of("power:+5V"), PowerPortStyle::Bar);
        assert_eq!(PowerPortStyle::of("power:VCC"), PowerPortStyle::Bar);
        assert_eq!(PowerPortStyle::of("power:PWR_FLAG"), PowerPortStyle::Bar);
        assert_eq!(
            PowerPortStyle::of("power:Ground"),
            PowerPortStyle::PowerGround
        );
        assert_eq!(PowerPortStyle::of("power:arrow"), PowerPortStyle::Arrow);
        assert_eq!(PowerPortStyle::of("power:wave"), PowerPortStyle::Wave);
        assert_eq!(PowerPortStyle::of("power:circle"), PowerPortStyle::Circle);
    }

    #[test]
    fn each_style_round_trips_through_its_lib_tag() {
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            assert_eq!(PowerPortStyle::of(&lib_id), style, "{lib_id}");
        }
    }

    #[test]
    fn the_body_hangs_the_way_the_style_says() {
        // The Properties panel keeps a port's orientation across a style
        // change by trusting `hangs_down`, so the drawn body must agree.
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            let ys: Vec<f64> = power_port_graphics(&lib_id)
                .iter()
                .flat_map(|g| match g {
                    Graphic::Polyline { points, .. } => points.iter().map(|p| p.y).collect(),
                    Graphic::Circle { center, .. } => vec![center.y],
                    _ => vec![],
                })
                .collect();
            let lowest = ys.iter().copied().fold(f64::INFINITY, f64::min);
            let highest = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if style.hangs_down() {
                assert!(lowest < 0.0 && highest <= 0.0, "{lib_id} should hang down");
            } else {
                assert!(highest > 0.0 && lowest >= 0.0, "{lib_id} should rise");
            }
        }
    }

    #[test]
    fn every_style_has_a_body() {
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            assert!(!power_port_graphics(&lib_id).is_empty(), "{lib_id}");
        }
    }

    #[test]
    fn the_lib_entry_lowers_to_the_built_in_body() {
        use crate::schematic::{SymbolPrimitive, lower_placed_symbol, lower_symbol};
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            let lib = power_port_lib_symbol(&lib_id);
            let sym = crate::schematic::Symbol {
                lib_id: lib_id.clone(),
                is_power: true,
                unit: 1,
                ..Default::default()
            };
            let lowered = lower_symbol(&lib, &sym);
            let pins: Vec<_> = lowered
                .iter()
                .filter_map(|p| match p {
                    SymbolPrimitive::Pin { tip, body, .. } => Some((*tip, *body)),
                    _ => None,
                })
                .collect();
            assert_eq!(pins.len(), 1, "{lib_id}");
            assert_eq!((pins[0].0.x, pins[0].0.y), (0.0, 0.0), "{lib_id}");
            let stem = STEM_MM;
            assert!((pins[0].1.x).abs() < 1e-9, "{lib_id}");
            assert!((pins[0].1.y.abs() - stem).abs() < 1e-9, "{lib_id}");
            // Same extent as the entry-less fallback.
            let fallback = lower_placed_symbol(None, &sym);
            assert_eq!(
                format!("{:?}", crate::schematic::primitives_bounds(&lowered)),
                format!("{:?}", crate::schematic::primitives_bounds(&fallback)),
                "{lib_id}"
            );
        }
    }

    /// The value text of every style stands clear of the head and of the
    /// wire it hangs on: above a rising head, below a hanging one (#1798).
    #[test]
    fn value_text_clears_the_head_of_every_style() {
        use crate::schematic::{lower_placed_symbol, primitives_bounds, text_aabb};
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            let at = Point::new(50.0, 40.0);
            let sym = Symbol {
                lib_id: lib_id.clone(),
                is_power: true,
                position: at,
                unit: 1,
                ..Default::default()
            };
            let head = primitives_bounds(&lower_placed_symbol(None, &sym)).expect("head");
            let value = power_port_value_field(&lib_id, at);
            let text = text_aabb(
                "+3V3",
                value.font_size,
                (value.position.x, value.position.y),
                0.0,
                value.justify_h,
                value.justify_v,
            );
            let overlaps = text.min_x < head.max_x
                && text.max_x > head.min_x
                && text.min_y < head.max_y
                && text.max_y > head.min_y;
            assert!(!overlaps, "{lib_id}: value {text:?} overlaps head {head:?}");
            if style.hangs_down() {
                assert!(text.min_y >= head.max_y, "{lib_id}: below the head");
            } else {
                assert!(text.max_y <= head.min_y, "{lib_id}: above the head");
            }
        }
    }
}
