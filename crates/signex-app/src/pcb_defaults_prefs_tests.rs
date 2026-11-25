//! #2868: the PCB Defaults of a new dimension in `prefs.json`.

use signex_types::pcb_dimension_style::{ArrowStyle, DimensionStyle, DimensionTextPosition};

use super::*;

fn round_trip(defaults: &DimensionDefaults) -> DimensionDefaults {
    let mut prefs = serde_json::Map::new();
    write_into(defaults, &mut prefs);
    from_prefs(Some(&serde_json::Value::Object(prefs)))
}

#[test]
fn the_defaults_round_trip_through_prefs() {
    let custom = DimensionDefaults {
        line_width: 0.2,
        text_height: 1.5,
        precision: 3,
        style: DimensionStyle {
            arrow_style: ArrowStyle::Filled,
            text_position: DimensionTextPosition::Inline,
            prefix: "W=".into(),
            trailing_zeros: false,
            ..DimensionStyle::default()
        },
    };
    assert_eq!(round_trip(&custom), custom);
    assert_eq!(
        round_trip(&DimensionDefaults::default()),
        DimensionDefaults::default()
    );
}

#[test]
fn missing_or_unsound_defaults_read_as_the_old_look() {
    assert_eq!(from_prefs(None), DimensionDefaults::default());
    let broken = serde_json::json!({ KEY: "not an object" });
    assert_eq!(from_prefs(Some(&broken)), DimensionDefaults::default());
    let negative = serde_json::json!({ KEY: { "line_width": -1.0 } });
    assert_eq!(from_prefs(Some(&negative)), DimensionDefaults::default());
}

#[test]
fn a_draft_differs_only_by_a_value_or_a_refused_field() {
    let saved = DimensionDefaults::default();
    let mut draft = PcbDefaultsDraft::of(&saved);
    assert!(!draft.differs_from(&saved));
    draft.texts.insert(PrimitiveField::ArrowLength, "1".into());
    assert!(!draft.differs_from(&saved), "the same value typed");
    draft
        .refused
        .insert(PrimitiveField::ArrowLength, "why".into());
    assert!(draft.differs_from(&saved));
}
