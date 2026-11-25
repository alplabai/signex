//! #2868: the look rows of the Dimension panel map to their engine edits,
//! and the facts show what the selection shares.

use signex_engine::pcb::primitive_edit::{DimensionEdit, DimensionStyleEdit as E};
use signex_types::pcb::Point;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_style::ArrowStyle;
use uuid::Uuid;

use super::dimension_style::{style_facts, words_edit};
use super::{PrimitiveEdit, dimension_edit, dimension_style_edit};
use crate::panels::pcb_primitive_properties::PrimitiveField as F;

fn dimension(id: u128) -> LinearDimension {
    LinearDimension::new(
        Uuid::from_u128(id),
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        2.0,
    )
}

#[test]
fn each_look_field_maps_to_its_edit() {
    for (field, expected) in [
        (F::ArrowLength, E::ArrowLength(1.5)),
        (F::ArrowWidth, E::ArrowWidth(1.5)),
        (F::TextWidth, E::TextWidth(1.5)),
        (F::TextGap, E::TextGap(1.5)),
        (F::TextAlong, E::TextAlong(1.5)),
        (F::TextAcross, E::TextAcross(1.5)),
        (F::ExtensionOffset, E::ExtensionOffset(1.5)),
        (F::ExtensionOvershoot, E::ExtensionOvershoot(1.5)),
        (F::ExtensionWidth, E::ExtensionWidth(1.5)),
    ] {
        let edit = PrimitiveEdit::Number(field, 1.5);
        assert_eq!(
            dimension_style_edit(&edit),
            Some(expected.clone()),
            "{field:?}"
        );
        assert_eq!(
            dimension_edit(&edit),
            Some(Ok(DimensionEdit::Style(expected)))
        );
    }
    assert_eq!(words_edit(F::Prefix, "L="), Some(E::Prefix("L=".into())));
    assert_eq!(
        words_edit(F::Suffix, " ref"),
        Some(E::Suffix(" ref".into()))
    );
    assert_eq!(words_edit(F::Text, "x"), None);
    assert!(F::Prefix.is_text() && F::Suffix.is_text());
    // The rows from before keep their own edits.
    let width = PrimitiveEdit::Number(F::LineWidth, 0.2);
    assert_eq!(
        dimension_edit(&width),
        Some(Ok(DimensionEdit::LineWidth(0.2)))
    );
}

#[test]
fn the_facts_show_what_the_selection_shares() {
    let a = dimension(1);
    let mut b = dimension(2);
    b.style.arrow_style = ArrowStyle::Filled;
    let facts = style_facts(&[&a, &b]);
    assert_eq!(facts.arrow_style, None, "the two differ");
    assert_eq!(facts.arrow_length, Some(a.style.arrow_length));
    assert_eq!(facts.prefix, Some(String::new()));
    let one = style_facts(&[&b]);
    assert_eq!(one.arrow_style, Some(ArrowStyle::Filled));
}
