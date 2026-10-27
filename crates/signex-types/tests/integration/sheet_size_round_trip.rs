//! `paper_size` through the `.snxsch` codec: every token the sheet-size model
//! reads, and every one it does not, re-saves byte-identical and keeps its
//! text (#1082).

use signex_types::format::SnxSchematic;
use signex_types::schematic::{SheetOrientation, SheetSize};

const BASE: &str = include_str!("../corpus/schematic/empty_sheet.snxsch");

fn toml_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn with_paper(token: &str) -> String {
    BASE.replace(
        "paper_size = \"A4\"",
        &format!("paper_size = {}", toml_string(token)),
    )
}

fn assert_round_trips(token: &str) -> SnxSchematic {
    let text = with_paper(token);
    let parsed = SnxSchematic::parse(&text).expect("parses");
    assert_eq!(parsed.sheet.paper_size, token);
    assert_eq!(parsed.write_string().expect("writes"), text, "{token:?}");
    parsed
}

#[test]
fn legacy_tokens_resave_byte_identical_and_read_as_typed_sizes() {
    for token in [
        "A0", "A1", "A2", "A3", "A4", "A5", "B5", "Letter", "Legal", "Tabloid",
    ] {
        let parsed = assert_round_trips(token);
        let (size, orientation) = parsed.sheet.sheet_size();
        assert!(size.is_recognized(), "{token}");
        assert_eq!(orientation, SheetOrientation::Landscape);
    }
}

#[test]
fn ansi_portrait_and_custom_round_trip() {
    for token in ["ANSI A", "ANSI E", "A3 portrait", "Custom 300x200.5"] {
        let parsed = assert_round_trips(token);
        assert!(parsed.sheet.sheet_size().0.is_recognized(), "{token}");
    }
    let parsed = assert_round_trips("ANSI D portrait");
    assert_eq!(parsed.sheet.paper_mm(), (558.8, 863.6));
    assert_eq!(parsed.sheet.sheet_size().0, SheetSize::AnsiD);
}

#[test]
fn unknown_tokens_keep_their_text() {
    for token in ["Foolscap", "a4", "", "Custom 0x0"] {
        let parsed = assert_round_trips(token);
        assert!(!parsed.sheet.sheet_size().0.is_recognized(), "{token:?}");
    }
}
