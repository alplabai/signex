//! Mockup rows 208 and 209: in the active variant a part with an alternate
//! is marked `alt` by its designator and a part with a value of its own `Δ`
//! by its value; hovering a mark names the variant, what it sets and the
//! Base value. Base marks nothing.

use iced::{Point, Size};
use signex_types::project::{MPN_PARAMETER, PlacedPart, VALUE_PARAMETER, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol, TextProp};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::markers::{MarkKind, marks_layer, variant_marks};
use crate::layout_probe::{lay_out_texts, probe_points};

const C22: Uuid = Uuid::from_u128(0x0208_0022);
const R5: Uuid = Uuid::from_u128(0x0209_0005);

fn text_at(x: f64, y: f64) -> Option<TextProp> {
    Some(TextProp {
        position: signex_types::schematic::Point::new(x, y),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: Default::default(),
        justify_v: Default::default(),
        hidden: false,
    })
}

/// C22 with its MPN and R5 at 31.6k, designators at (10, 10) and (40, 10),
/// values at (10, 14) and (40, 14).
fn sheet() -> SchematicSheet {
    let part = |uuid, reference: &str, value: &str, x: f64| Symbol {
        uuid,
        lib_id: "Device:X".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        fields: [(MPN_PARAMETER.to_string(), format!("{reference}-BASE"))].into(),
        ref_text: text_at(x, 10.0),
        val_text: text_at(x, 14.0),
        ..Default::default()
    };
    SchematicSheet {
        symbols: vec![part(C22, "C22", "22µ", 10.0), part(R5, "R5", "31.6k", 40.0)],
        ..Default::default()
    }
}

/// Lite gives C22 an alternate and R5 the value 47k.
fn settings() -> VariantSettings {
    let mut data: signex_types::project::ProjectData = serde_json::from_value(
        serde_json::json!({"name": "P", "dir": "", "schematic_root": null, "pcb_file": null}),
    )
    .unwrap();
    data.add_variant("Lite").unwrap();
    let alternate = [(
        MPN_PARAMETER.to_string(),
        Some("CL21A226MPQNNNE".to_string()),
    )];
    data.set_variant_part_parameters("Lite", PlacedPart::from("C22"), &alternate)
        .unwrap();
    data.set_variant_part_parameter("Lite", "R5", VALUE_PARAMETER, Some("47k"))
        .unwrap();
    data.variant_settings
}

#[test]
fn an_alternate_and_a_value_of_its_own_are_marked_and_base_marks_nothing() {
    let marks = variant_marks(&sheet(), &InstancePath::root(), "Lite", &settings());
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0].symbol, C22);
    assert_eq!(marks[0].at, (10.0, 10.0), "by the designator");
    assert_eq!(
        marks[0].kind,
        MarkKind::Alternate {
            mpn: "CL21A226MPQNNNE".into(),
            base_mpn: "C22-BASE".into(),
        }
    );
    assert_eq!(marks[1].symbol, R5);
    assert_eq!(marks[1].at, (40.0, 14.0), "by the value");
    assert_eq!(
        marks[1].kind,
        MarkKind::Value {
            value: "47k".into(),
            base: "31.6k".into(),
        }
    );
    assert!(variant_marks(&sheet(), &InstancePath::root(), "Base", &settings()).is_empty());
    assert!(variant_marks(&sheet(), &InstancePath::root(), "Pro", &settings()).is_empty());
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

/// The marks lay out where the camera puts them, and hovering each opens
/// its card, in both themes.
#[test]
fn the_marks_show_and_hover_with_what_the_variant_sets() {
    let marks = variant_marks(&sheet(), &InstancePath::root(), "Lite", &settings());
    let window = Size::new(900.0, 600.0);
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(theme);
        let i18n = signex_i18n::I18n::en_us_ref();
        let layer = || marks_layer(&marks, (20.0, 40.0, 8.0), "Lite", &tokens, i18n);
        let texts = lay_out_texts(layer(), window);
        let alt = texts
            .iter()
            .find(|t| t.text == "alt")
            .expect("the alt mark");
        let delta = texts
            .iter()
            .find(|t| t.text == "Δ")
            .expect("the value mark");
        assert!(alt.bounds.x >= 100.0 && alt.bounds.x < 110.0, "{alt:?}");
        assert!(
            delta.bounds.x >= 340.0 && delta.bounds.x < 350.0,
            "{delta:?}"
        );
        assert!(alt.bounds.y < 120.0, "above the designator: {alt:?}");

        let at = |bounds: iced::Rectangle| Point::new(bounds.center_x(), bounds.center_y());
        let probes = probe_points(layer(), window, &[at(alt.bounds), at(delta.bounds)]);
        let tips: Vec<Vec<String>> = probes
            .iter()
            .map(|probe| probe.tips.iter().map(|tip| plain(tip)).collect())
            .collect();
        assert!(
            tips[0].contains(&"Alternate part in Lite".to_string())
                && tips[0].contains(&"CL21A226MPQNNNE".to_string())
                && tips[0].contains(&"Base: C22-BASE".to_string()),
            "{theme:?}: {:?}",
            tips[0]
        );
        assert!(
            tips[1].contains(&"Value in Lite: 47k".to_string())
                && tips[1].contains(&"Base: 31.6k".to_string()),
            "{theme:?}: {:?}",
            tips[1]
        );
        assert!(probes.iter().all(|probe| probe.clicked.is_empty()));
    }
}
