use std::collections::BTreeMap;

use uuid::Uuid;

use super::writer::quote;
use super::*;
use crate::format_version::FormatVersionError;
use crate::object_id::SequentialIds;

const DIAGRAM_ID: u128 = 0x100;

fn empty() -> BlockDiagram {
    BlockDiagram::with_id(DocumentId::from_uuid(Uuid::from_u128(DIAGRAM_ID)).unwrap())
}

/// Two blocks, a port on the first, a connection from that port to the
/// second block's edge.
fn small() -> BlockDiagram {
    small_from(&mut SequentialIds::default())
}

fn small_from(ids: &mut SequentialIds) -> BlockDiagram {
    let mut d = empty();
    let mcu = BlockFields::named("MCU", Point::new(10.0, 20.5));
    let a = d.add_block(ids, mcu).unwrap();
    let sensor = BlockFields::named("Sensor", Point::new(80.0, 20.5));
    let b = d.add_block(ids, sensor).unwrap();
    let both_ways = PortDirection::Bidirectional;
    let sda = d
        .add_port(ids, a, "SDA", PortSide::Right, 5.0, both_ways)
        .unwrap();
    let from = Endpoint {
        block: a,
        port: Some(sda),
    };
    let to = Endpoint {
        block: b,
        port: None,
    };
    d.connect(ids, from, to, Interface::I2c, "SDA, SCL")
        .unwrap();
    d
}

/// The writer's exact layout for [`small`]: the determinism pin.
const SMALL_TEXT: &str = r#"format = "snxblk/1"
diagram_id = "00000000-0000-0000-0000-000000000100"

[[blocks]]
id = "00000000-0000-0000-0000-000000000001"
name = "MCU"
shape = "rectangle"
x_nm = 10000000
y_nm = 20500000
width_nm = 40000000
height_nm = 25000000

[[blocks.ports]]
id = "00000000-0000-0000-0000-000000000003"
name = "SDA"
side = "right"
offset_nm = 5000000
direction = "bidirectional"

[[blocks]]
id = "00000000-0000-0000-0000-000000000002"
name = "Sensor"
shape = "rectangle"
x_nm = 80000000
y_nm = 20500000
width_nm = 40000000
height_nm = 25000000

[[connections]]
id = "00000000-0000-0000-0000-000000000004"
from_block = "00000000-0000-0000-0000-000000000001"
from_port = "00000000-0000-0000-0000-000000000003"
to_block = "00000000-0000-0000-0000-000000000002"
interface = "i2c"
label = "SDA, SCL"
"#;

/// A diagram that uses every optional key.
fn full() -> BlockDiagram {
    let mut ids = SequentialIds::default();
    let mut d = small_from(&mut ids);
    let g = d
        .add_group(&mut ids, "Power", Some(Color::new(0xd0, 0x45, 0x3a, 0xff)))
        .unwrap();
    let mut fields = BlockFields::named("Buck", Point::new(-3.25, 0.000001));
    fields.function = "3 A buck".into();
    fields.note = "line one\nline two".into();
    fields.shape = BlockShape::RoundedRectangle;
    fields.color = Some(Color::new(1, 2, 3, 128));
    fields.group = Some(g);
    fields.rails = vec!["VIN".into(), "VOUT".into()];
    fields.specs = BTreeMap::from([
        ("Vout".to_string(), "3.30 V".to_string()),
        ("Fsw".to_string(), "570 kHz".to_string()),
    ]);
    let buck = d.add_block(&mut ids, fields).unwrap();
    let link = SheetLink {
        sheet: DocumentId::from_uuid(Uuid::from_u128(0xa1)).unwrap(),
        path: "power/dcdc.snxsch".into(),
        state: LinkState::Unlinked,
    };
    d.link_sheet(buck, link).unwrap();
    let mcu = d.blocks[0].id;
    let ends = (
        Endpoint {
            block: buck,
            port: None,
        },
        Endpoint {
            block: mcu,
            port: None,
        },
    );
    let can = Interface::Other("CAN".into());
    let c = d.connect(&mut ids, ends.0, ends.1, can, "").unwrap();
    d.connections
        .iter_mut()
        .find(|l| l.id == c)
        .unwrap()
        .derived_from = Some("CAN_H".into());
    d
}

#[test]
fn the_writer_writes_a_fixed_layout() {
    assert_eq!(encode_block_diagram(&small()).unwrap(), SMALL_TEXT);
}

#[test]
fn a_diagram_round_trips_through_its_text() {
    for diagram in [empty(), small(), full()] {
        let text = encode_block_diagram(&diagram).unwrap();
        let read = parse_block_diagram(&text).unwrap();
        assert_eq!(read.diagram, diagram);
        assert_eq!(read.token, SNXBLK_FORMAT_V1);
        assert!(!read.minted_id && !read.is_read_only());
        assert_eq!(encode_block_diagram(&read.diagram).unwrap(), text);
    }
}

#[test]
fn hostile_text_round_trips() {
    let hostile = "q\"uote \\ back\ttab\nnl\r\u{1}\u{7f}\u{8}\u{c} # [x] = { } ünï 日本";
    assert_eq!(quote("a\"b"), "\"a\\\"b\"");
    assert_eq!(quote("\u{1b}"), "\"\\u001B\"");
    let mut d = small();
    d.blocks[0].name = hostile.into();
    d.blocks[0].note = hostile.into();
    d.blocks[0].rails = vec![hostile.into()];
    d.blocks[0].specs = BTreeMap::from([(hostile.to_string(), hostile.to_string())]);
    d.connections[0].label = hostile.into();
    d.connections[0].interface = Interface::Other(hostile.trim().into());
    let text = encode_block_diagram(&d).unwrap();
    assert_eq!(parse_block_diagram(&text).unwrap().diagram, d);
}

#[test]
fn the_same_diagram_always_writes_the_same_bytes() {
    let one = encode_block_diagram(&full()).unwrap();
    let two = encode_block_diagram(&full()).unwrap();
    assert_eq!(one, two);
    let fsw = one.find("\"Fsw\"").unwrap();
    assert!(fsw < one.find("\"Vout\"").unwrap(), "specs sorted by name");
}

#[test]
fn a_newer_minor_opens_read_only_and_refuses_its_save() {
    let text = SMALL_TEXT.replacen("snxblk/1\"", "snxblk/1.1\"", 1);
    let read = parse_block_diagram(&text).unwrap();
    assert_eq!(read.token, "snxblk/1.1");
    assert!(read.is_read_only());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.snxblk");
    let refused = write_block_diagram(&path, &read.diagram, &read.token);
    assert!(matches!(
        refused,
        Err(BlockDiagramFileError::Version(
            FormatVersionError::NewerReadOnly { .. }
        ))
    ));
    assert!(!path.exists(), "nothing is written");
}

#[test]
fn a_newer_major_a_missing_token_or_a_foreign_one_is_refused() {
    let version_error = |text: &str| {
        matches!(
            parse_block_diagram(text),
            Err(BlockDiagramFileError::Version(_))
        )
    };
    let token = "snxblk/1\"";
    for other in ["snxblk/2\"", "snxsch/1\"", "snxblk/01\""] {
        let text = SMALL_TEXT.replacen(token, other, 1);
        assert!(version_error(&text), "{other}");
    }
    let untokened = SMALL_TEXT.replacen("format = \"snxblk/1\"\n", "", 1);
    assert!(version_error(&untokened));
}

#[test]
fn a_file_that_is_not_consistent_is_refused() {
    let dangling = SMALL_TEXT.replacen(
        "to_block = \"00000000-0000-0000-0000-000000000002\"",
        "to_block = \"00000000-0000-0000-0000-000000000099\"",
        1,
    );
    assert!(matches!(
        parse_block_diagram(&dangling),
        Err(BlockDiagramFileError::Fault(
            DiagramFault::MissingBlock { .. }
        ))
    ));
    let twice = SMALL_TEXT.replacen(
        "id = \"00000000-0000-0000-0000-000000000002\"",
        "id = \"00000000-0000-0000-0000-000000000001\"",
        1,
    );
    assert!(matches!(
        parse_block_diagram(&twice),
        Err(BlockDiagramFileError::Fault(DiagramFault::DuplicateId(_)))
    ));
    let flat = SMALL_TEXT.replacen("height_nm = 25000000", "height_nm = 0", 1);
    assert!(matches!(
        parse_block_diagram(&flat),
        Err(BlockDiagramFileError::Fault(DiagramFault::BadBlockSize(_)))
    ));
}

#[test]
fn a_bad_value_is_refused_naming_it() {
    for (from, to) in [
        ("shape = \"rectangle\"", "shape = \"blob\""),
        ("side = \"right\"", "side = \"middle\""),
        ("direction = \"bidirectional\"", "direction = \"sideways\""),
        ("x_nm = 10000000", "x_nm = 1.5"),
        ("name = \"MCU\"\n", ""),
    ] {
        let text = SMALL_TEXT.replacen(from, to, 1);
        assert!(
            matches!(
                parse_block_diagram(&text),
                Err(BlockDiagramFileError::Parse(_))
            ),
            "{to}"
        );
    }
    let colored = SMALL_TEXT.replacen(
        "shape = \"rectangle\"\n",
        "shape = \"rectangle\"\ncolor = \"red\"\n",
        1,
    );
    let Err(BlockDiagramFileError::Parse(message)) = parse_block_diagram(&colored) else {
        panic!("a bad colour is refused");
    };
    assert!(message.contains("red"), "{message}");
}

#[test]
fn a_missing_or_nil_id_is_minted_on_read() {
    let nil = SMALL_TEXT.replacen(
        "diagram_id = \"00000000-0000-0000-0000-000000000100\"",
        "diagram_id = \"00000000-0000-0000-0000-000000000000\"",
        1,
    );
    let missing = SMALL_TEXT.replacen(
        "diagram_id = \"00000000-0000-0000-0000-000000000100\"\n",
        "",
        1,
    );
    for text in [nil, missing] {
        let read = parse_block_diagram(&text).unwrap();
        assert!(read.minted_id);
        assert!(!read.diagram.id.as_uuid().is_nil());
    }
}

#[test]
fn an_unknown_interface_name_is_kept_as_written() {
    let text = SMALL_TEXT.replacen("interface = \"i2c\"", "interface = \"MIPI CSI\"", 1);
    let read = parse_block_diagram(&text).unwrap();
    assert_eq!(
        read.diagram.connections[0].interface,
        Interface::Other("MIPI CSI".into())
    );
    assert_eq!(encode_block_diagram(&read.diagram).unwrap(), text);
}

#[test]
fn a_length_the_file_cannot_hold_refuses_the_save() {
    let mut d = small();
    d.blocks[0].width_mm = 1.0e300;
    assert!(matches!(
        encode_block_diagram(&d),
        Err(BlockDiagramFileError::Length(field)) if field.starts_with("width_nm")
    ));
    let mut d = small();
    d.blocks[0].position.x = f64::INFINITY;
    assert!(matches!(
        encode_block_diagram(&d),
        Err(BlockDiagramFileError::Fault(
            DiagramFault::BadBlockPosition(_)
        ))
    ));
}

#[test]
fn the_file_is_written_atomically_and_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("system.{BLOCK_DIAGRAM_EXTENSION}"));
    let diagram = full();
    write_block_diagram(&path, &diagram, SNXBLK_FORMAT_V1).unwrap();
    let read = read_block_diagram(&path).unwrap();
    assert_eq!(read.diagram, diagram);
    assert!(matches!(
        read_block_diagram(&dir.path().join("missing.snxblk")),
        Err(BlockDiagramFileError::Io(_))
    ));
}
