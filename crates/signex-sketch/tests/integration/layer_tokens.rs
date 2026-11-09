//! The `layer` attribute stores board-layer tokens only; anything else is a
//! read error naming the attribute and the bad value.

use signex_sketch::attr::{KeepoutAttr, SilkAttr};
use signex_types::pcb_layer::PcbLayer;

fn silk(layer: &str) -> Result<SilkAttr, toml::de::Error> {
    toml::from_str(&format!("layer = {layer}"))
}

#[test]
fn every_board_layer_round_trips_as_its_token() {
    for layer in PcbLayer::all() {
        let a = SilkAttr { layer };
        let back: SilkAttr = toml::from_str(&toml::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a, "{layer:?}");
    }
    let k: KeepoutAttr =
        toml::from_str("layer = \"TopCopper\"\n[kinds]\nno_routing = true\n").unwrap();
    assert_eq!(
        toml::from_str::<KeepoutAttr>(&toml::to_string(&k).unwrap()).unwrap(),
        k
    );
}

#[test]
fn a_retired_spelling_is_a_read_error_naming_attribute_and_value() {
    for old in ["\"top_silk\"", "\"keep_out\"", "{ mechanical = 4 }"] {
        let e = silk(old).expect_err("must not load").to_string();
        assert!(e.contains("layer attribute"), "{e}");
        assert!(e.contains("is not a board layer"), "{e}");
    }
    let e = silk("\"top_silk\"").unwrap_err().to_string();
    assert!(e.contains("top_silk"), "{e}");
}

#[test]
fn an_out_of_range_index_is_a_read_error_naming_the_value() {
    for bad in [
        "Inner0Copper",
        "Inner31Copper",
        "Mechanical0",
        "Mechanical33",
    ] {
        let e = silk(&format!("\"{bad}\"")).unwrap_err().to_string();
        assert!(e.contains("layer attribute") && e.contains(bad), "{e}");
    }
}
