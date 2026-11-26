//! The symbol envelope and the two footprint envelopes of the fixture.

use super::*;

pub(crate) fn pin(
    number: &str,
    name: &str,
    electrical: PinDirection,
    position: [f64; 2],
    orientation: PinOrientation,
) -> SymbolPin {
    SymbolPin {
        electrical,
        position,
        orientation,
        ..SymbolPin::new(number, name)
    }
}

pub(crate) fn graphic(kind: SymbolGraphicKind, part_number: u8, body_style: u8) -> SymbolGraphic {
    SymbolGraphic {
        kind,
        stroke_width: 0.254,
        fill: None,
        part_number,
        body_style,
    }
}

pub(crate) fn symbol(uuid: &str, name: &str) -> Symbol {
    Symbol {
        uuid: id(uuid),
        created: created(),
        updated: updated(),
        version: "1.0.0".into(),
        ..Symbol::empty(name)
    }
}

pub(crate) fn resistor_symbol() -> Symbol {
    let mut pin1 = pin(
        "1",
        "1",
        PinDirection::Passive,
        [-5.08, 0.0],
        PinOrientation::Right,
    );
    let mut pin2 = pin(
        "2",
        "2",
        PinDirection::Passive,
        [5.08, 0.0],
        PinOrientation::Left,
    );
    pin1.name_visible = false;
    pin2.name_visible = false;
    Symbol {
        pins: vec![pin1, pin2],
        graphics: vec![graphic(
            SymbolGraphicKind::Rectangle {
                from: [-2.54, 1.016],
                to: [2.54, -1.016],
            },
            0,
            1,
        )],
        designator: "R?".into(),
        designator_pos: Some([0.0, 2.54]),
        comment_pos: Some([0.0, -2.54]),
        description: "Resistor".into(),
        released: true,
        ..symbol(RESISTOR_SYMBOL, "Resistor")
    }
}

/// Two units; inputs on both bodies (0), the output once per body (1, 2).
pub(crate) fn nand_unit(unit: u8, first: u8) -> Vec<SymbolPin> {
    let n = |offset: u8| (first + offset).to_string();
    let y = unit_y(unit);
    let mut a = pin(
        &n(0),
        "A",
        PinDirection::Input,
        [-7.62, y + 2.54],
        PinOrientation::Right,
    );
    let mut b = pin(
        &n(1),
        "B",
        PinDirection::Input,
        [-7.62, y - 2.54],
        PinOrientation::Right,
    );
    let mut out = pin(
        &n(2),
        "Y",
        PinDirection::Output,
        [7.62, y],
        PinOrientation::Left,
    );
    let mut out_alt = pin(
        &n(2),
        "Y",
        PinDirection::Output,
        [10.16, y],
        PinOrientation::Left,
    );
    for p in [&mut a, &mut b, &mut out, &mut out_alt] {
        p.part_number = unit;
    }
    a.body_style = 0;
    b.body_style = 0;
    out_alt.body_style = 2;
    out_alt.outside_edge_symbol = PinSymbolKind::Dot;
    vec![a, b, out, out_alt]
}

pub(crate) fn unit_y(unit: u8) -> f64 {
    if unit == 1 { 7.62 } else { -7.62 }
}

pub(crate) fn nand_symbol() -> Symbol {
    let mut pins = nand_unit(1, 1);
    pins[0].function = vec!["A1".into(), "SCL".into()];
    pins[1].pin_package_length = Some(1.5);
    pins[1].propagation_delay_ns = Some(0.25);
    pins.extend(nand_unit(2, 4));
    for (number, name, y, orientation) in [
        ("7", "GND", -15.24, PinOrientation::Up),
        ("14", "VCC", 15.24, PinOrientation::Down),
    ] {
        let mut power = pin(
            number,
            name,
            PinDirection::PowerInput,
            [0.0, y],
            orientation,
        );
        power.part_number = 0;
        power.body_style = 0;
        power.hidden = true;
        pins.push(power);
    }
    let mut graphics = Vec::new();
    for unit in [1, 2] {
        let y = unit_y(unit);
        let mut body = graphic(
            SymbolGraphicKind::Rectangle {
                from: [-5.08, y + 5.08],
                to: [5.08, y - 5.08],
            },
            unit,
            1,
        );
        body.fill = Some([255, 255, 194, 255]);
        graphics.push(body);
        graphics.push(graphic(
            SymbolGraphicKind::Circle {
                center: [0.0, y],
                radius: 5.08,
            },
            unit,
            2,
        ));
    }
    graphics.push(graphic(
        SymbolGraphicKind::Text {
            position: [0.0, 0.0],
            content: "&".into(),
            size: 1.27,
        },
        0,
        0,
    ));
    let mut schematic_params = ParamMap::new();
    schematic_params.insert("logic_family".into(), ParamValue::Text("HC".into()));
    Symbol {
        pins,
        graphics,
        schematic_params,
        description: "Dual 2-input \"NAND\" gate, De Morgan bodies".into(),
        local_line_color: Some([0, 0, 128, 255]),
        part_count: 2,
        component_type: ComponentType::Standard,
        ..symbol(NAND_SYMBOL, "Dual NAND")
    }
}

pub(crate) fn symbol_envelope() -> SymbolFile {
    SymbolFile {
        format: String::new(),
        file_uuid: id(SYMBOL_FILE),
        display_name: "Conformance Symbols".into(),
        copied_from: None,
        symbols: vec![resistor_symbol(), nand_symbol()],
        created: created(),
        updated: updated(),
    }
}

// ── Footprints ───────────────────────────────────────────────────────────

pub(crate) fn chip_pad(number: &str, x: f64, size: [f64; 2], shape: PadShape) -> Pad {
    Pad {
        number: number.into(),
        kind: PadKind::Smd,
        shape,
        size,
        position: [x, 0.0],
        layers: LAYERS.iter().map(|l| LayerId::new(*l)).collect(),
        solder_mask_margin: Some(0.05),
        ..Pad::default()
    }
}

pub(crate) fn rectangle(half_w: f64, half_h: f64) -> Polygon {
    Polygon::new(vec![
        [-half_w, -half_h],
        [half_w, -half_h],
        [half_w, half_h],
        [-half_w, half_h],
    ])
}

pub(crate) fn chip_footprint(uuid: &str, name: &str, half_w: f64, half_h: f64) -> Footprint {
    let mut fp = Footprint::with_identity(name, id(uuid), created());
    fp.updated = updated();
    fp.version = "1.0.0".into();
    fp.courtyard = rectangle(half_w, half_h);
    fp.fab_f = vec![FpGraphic::drawn(
        FpGraphicKind::Rectangle {
            from: [-half_w + 0.7, -half_h + 0.35],
            to: [half_w - 0.7, half_h - 0.35],
        },
        0.1,
        false,
    )];
    fp.default_designator = "R?".into();
    fp
}

pub(crate) fn r0603() -> Footprint {
    let mut fp = chip_footprint(R0603_FOOTPRINT, "R_0603", 1.5, 0.75);
    fp.pads = vec![
        chip_pad("1", -0.8, [0.8, 0.95], PadShape::Rect),
        chip_pad("2", 0.8, [0.8, 0.95], PadShape::Rect),
    ];
    fp.silk_f = [0.6, -0.6]
        .into_iter()
        .map(|y| {
            FpGraphic::drawn(
                FpGraphicKind::Line {
                    from: [-0.2, y],
                    to: [0.2, y],
                },
                0.12,
                false,
            )
        })
        .collect();
    fp.body_3d.height_mm = 0.55;
    fp.height_mm = Some(0.55);
    fp.description = "Chip resistor, 0603 (1608 metric)".into();
    fp
}

/// Stands in for a STEP file's bytes: only their hash is in the fixture.
pub(crate) const STEP_PLACEHOLDER: &[u8] = b"ISO-10303-21;\nEND-ISO-10303-21;\n";

pub(crate) fn r0805() -> Footprint {
    let mut fp = chip_footprint(R0805_FOOTPRINT, "R_0805", 1.75, 1.0);
    let shape = PadShape::RoundRect { radius_ratio: 0.25 };
    fp.pads = vec![
        chip_pad("1", -0.9, [1.15, 1.45], shape.clone()),
        chip_pad("2", 0.9, [1.15, 1.45], shape),
    ];
    for pad in &mut fp.pads {
        pad.paste_margin = Some(0.0);
        pad.corner_radius_pct = Some(25.0);
    }
    fp.body_3d.height_mm = 0.6;
    fp.step_attachment = Some(StepAttachment {
        content_hash: hex(&Sha256::digest(STEP_PLACEHOLDER)),
        filename: "R_0805.step".into(),
        offset_xyz: [0.0, 0.0, 0.0],
        rotation_xyz: [0.0, 0.0, 90.0],
    });
    fp.description = "Chip resistor, 0805 (2012 metric)".into();
    fp
}

/// Asymmetric: pads 1 and 2 at +y, pad 3 alone at -y. The footprint frame
/// is the board's, +y down the screen, so pad 3 shows above pads 1 and 2.
pub(crate) fn sot23() -> Footprint {
    let mut fp = Footprint::with_identity("SOT-23", id(SOT23_FOOTPRINT), created());
    fp.updated = updated();
    fp.version = "1.0.0".into();
    let pad = |number: &str, x: f64, y: f64| Pad {
        position: [x, y],
        ..chip_pad(number, x, [0.6, 0.7], PadShape::Rect)
    };
    fp.pads = vec![
        pad("1", -0.95, 1.1),
        pad("2", 0.95, 1.1),
        pad("3", 0.0, -1.1),
    ];
    fp.courtyard = rectangle(1.7, 1.75);
    fp.silk_f = vec![FpGraphic::drawn(
        FpGraphicKind::Circle {
            center: [-1.45, 1.6],
            radius: 0.1,
        },
        0.12,
        true,
    )];
    fp.default_designator = "Q?".into();
    fp.description = "SOT-23, pin 1 at bottom left".into();
    fp
}

/// Every fixture footprint with its envelope's file uuid.
pub(crate) fn footprints() -> impl Iterator<Item = (Footprint, &'static str)> {
    [
        (r0603(), R0603_FILE),
        (r0805(), R0805_FILE),
        (sot23(), SOT23_FILE),
    ]
    .into_iter()
}

pub(crate) fn footprint_envelope(fp: Footprint, file_uuid: &str) -> FootprintFile {
    let mut file = FootprintFile::with_identity(fp, id(file_uuid), created());
    file.updated = updated();
    file
}
