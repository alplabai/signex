use std::io::Cursor;

use super::{ApertureShape, GerberPrimitive, PrimitivePolarity, load_gerber_reader};

const SIMPLE_LAYER: &str = r#"%FSLAX46Y46*%
%MOMM*%
%ADD10C,1.000*%
%ADD11R,2.000X1.000*%
D10*
X0000000Y0000000D02*
X0100000Y0000000D01*
X0100000Y0100000D03*
D11*
X0200000Y0100000D03*
M02*
"#;

#[test]
fn extracts_strokes_and_standard_flashes() {
    let layer = load_gerber_reader("simple.gbr", Cursor::new(SIMPLE_LAYER))
        .expect("simple Gerber must parse");

    assert!(matches!(
        layer.geometry.primitives[0],
        GerberPrimitive::Stroke {
            width,
            polarity: PrimitivePolarity::Dark,
            ..
        } if (width - 1.0).abs() < 1e-9
    ));
    assert!(layer.geometry.primitives.iter().any(|primitive| matches!(
        primitive,
        GerberPrimitive::Flash {
            aperture: ApertureShape::Rectangle { width, height },
            ..
        } if (*width - 2.0).abs() < 1e-9 && (*height - 1.0).abs() < 1e-9
    )));
}

#[test]
fn associates_x2_components_with_emitted_primitives_and_honors_deletion() {
    let source = r#"%FSLAX46Y46*%
%MOMM*%
%ADD10C,1.000*%
D10*
%TO.C,R1*%
X0000000Y0000000D03*
%TO.C,C2*%
X0100000Y0000000D03*
%TD.C*%
X0200000Y0000000D03*
M02*
"#;

    let layer = load_gerber_reader("components.gbr", Cursor::new(source))
        .expect("X2 component Gerber must parse");
    let components = layer
        .geometry
        .primitive_attributes
        .iter()
        .map(|attributes| attributes.component.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(components, [Some("R1"), Some("C2"), None]);
    assert_eq!(
        layer.geometry.primitive_attributes[0].attributes[0].name,
        ".C"
    );
    assert!(layer.geometry.primitive_attributes[2].attributes.is_empty());
    assert_eq!(
        layer.geometry.primitives.len(),
        layer.geometry.primitive_attributes.len()
    );
}

#[test]
fn associates_x2_nets_with_primitives_and_honors_deletion() {
    let source = r#"%FSLAX46Y46*%
%MOMM*%
%ADD10C,1.000*%
D10*
%TO.N,GND,SIGNAL*%
X0000000Y0000000D03*
%TO.N,N/C*%
X0100000Y0000000D03*
%TD.N*%
X0200000Y0000000D03*
M02*
"#;

    let layer =
        load_gerber_reader("nets.gbr", Cursor::new(source)).expect("X2 net Gerber must parse");
    let nets = layer
        .geometry
        .primitive_attributes
        .iter()
        .map(|attributes| attributes.nets.as_slice())
        .collect::<Vec<_>>();

    assert_eq!(nets[0], ["GND", "SIGNAL"]);
    assert_eq!(nets[1], ["N/C"]);
    assert!(nets[2].is_empty());
    assert_eq!(
        layer.geometry.primitive_attributes[0].attributes[0].name,
        ".N"
    );
    assert!(layer.geometry.primitive_attributes[2].attributes.is_empty());
}

#[test]
fn preserves_replaces_and_deletes_all_x2_object_attributes() {
    let source = r#"%FSLAX46Y46*%
%MOMM*%
%ADD10C,1.000*%
D10*
%TO.CVal,10k*%
X0000000Y0000000D03*
%TO.CVal,22k*%
%TO.MyAttribute,Alpha,Beta*%
X0100000Y0000000D03*
%TD.CVal*%
X0200000Y0000000D03*
%TD*%
X0300000Y0000000D03*
M02*
"#;

    let layer = load_gerber_reader("attributes.gbr", Cursor::new(source))
        .expect("X2 object attributes must parse");
    let attributes = &layer.geometry.primitive_attributes;

    assert_eq!(
        attributes[0].attributes,
        [super::GerberAttributeValue {
            name: ".CVal".into(),
            values: vec!["10k".into()],
        }]
    );
    assert_eq!(
        attributes[1].attributes,
        [
            super::GerberAttributeValue {
                name: ".CVal".into(),
                values: vec!["22k".into()],
            },
            super::GerberAttributeValue {
                name: ".MyAttribute".into(),
                values: vec!["Alpha".into(), "Beta".into()],
            },
        ]
    );
    assert_eq!(
        attributes[2].attributes,
        [super::GerberAttributeValue {
            name: ".MyAttribute".into(),
            values: vec!["Alpha".into(), "Beta".into()],
        }]
    );
    assert!(attributes[3].attributes.is_empty());
}
