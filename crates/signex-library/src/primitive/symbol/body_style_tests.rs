//! De Morgan body style on `.snxsym` items (#1233): `0` common, `1` normal
//! (default), `2` alternate. A file with no off-normal item keeps its exact
//! legacy bytes; one with any is written as v2 with an appended TSV column.

use super::serde_tsv::*;
use super::*;

fn pin(number: &str, body_style: u8) -> SymbolPin {
    SymbolPin {
        body_style,
        ..SymbolPin::new(number, number)
    }
}

fn graphic(body_style: u8) -> SymbolGraphic {
    SymbolGraphic {
        kind: SymbolGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        stroke_width: 0.15,
        fill: None,
        part_number: 0,
        body_style,
    }
}

fn file(pins: Vec<SymbolPin>, graphics: Vec<SymbolGraphic>) -> SymbolFile {
    let mut s = Symbol::empty("G");
    s.pins = pins;
    s.graphics = graphics;
    SymbolFile::from_symbol(s)
}

#[test]
fn body_styles_zero_one_and_two_round_trip_on_pins_and_graphics() {
    let original = file(
        vec![pin("1", 0), pin("2", 1), pin("3", 2)],
        vec![graphic(0), graphic(1), graphic(2)],
    );
    let text = original.to_toml_string().unwrap();
    let back = SymbolFile::from_toml_str(&text).unwrap();
    let s = &back.symbols[0];
    assert_eq!(
        s.pins.iter().map(|p| p.body_style).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        s.graphics.iter().map(|g| g.body_style).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(back.to_toml_string().unwrap(), text);
}

#[test]
fn a_file_with_an_off_normal_item_is_v2_and_appends_the_column() {
    let text = file(vec![pin("1", 2)], vec![]).to_toml_string().unwrap();
    assert!(text.contains("snxsym/v2"), "{text}");
    assert!(
        text.contains("part_number\tbody_style\n"),
        "the column is appended after part_number:\n{text}"
    );
    let graphic_only = file(vec![pin("1", 1)], vec![graphic(2)])
        .to_toml_string()
        .unwrap();
    assert!(graphic_only.contains("snxsym/v2"), "{graphic_only}");
}

#[test]
fn a_legacy_file_loads_as_normal_and_re_saves_byte_identical() {
    // All pins and graphics on the normal body: the writer emits the legacy
    // 20-column layout, v1 token and no `body_style` key, i.e. exactly what a
    // build without this field wrote.
    let legacy = file(vec![pin("1", 1), pin("2", 1)], vec![graphic(1)])
        .to_toml_string()
        .unwrap();
    assert!(legacy.contains("snxsym/v1"), "{legacy}");
    assert!(!legacy.contains("body_style"), "{legacy}");

    let back = SymbolFile::from_toml_str(&legacy).unwrap();
    assert!(back.symbols[0].pins.iter().all(|p| p.body_style == 1));
    assert!(back.symbols[0].graphics.iter().all(|g| g.body_style == 1));
    assert_eq!(back.to_toml_string().unwrap(), legacy);
}

#[test]
fn an_out_of_range_body_cell_is_refused_not_guessed() {
    let tsv = pins_to_tsv(&[pin("1", 2)])
        .unwrap()
        .replace("\t2\n", "\t3\n");
    assert!(matches!(
        pins_from_tsv(&tsv),
        Err(SymbolFileError::InvalidNumericCell {
            column: "body_style",
            ..
        })
    ));
}

#[test]
fn to_lib_symbol_carries_the_body_instead_of_hard_coding_one() {
    let lib = file(vec![pin("1", 1), pin("2", 2)], vec![graphic(0), graphic(2)]).symbols[0]
        .to_lib_symbol("G");
    assert_eq!(
        lib.pins.iter().map(|p| p.body_style).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        lib.graphics
            .iter()
            .map(|g| g.body_style)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert!(lib.has_alternate_body(0));
}
