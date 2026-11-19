//! The Track illustration (m95, `R.track`): a track run with its Width
//! callout, its net and the "layer · round ends" note. While placing, a
//! dashed run to the pointer and a cursor cross.

use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::Label;
use signex_widgets::property_illustration::{Anchor, Illustration, TextRole};

use super::{at, band, callout, canvas, cross, dashed, keys, label, layer_label, layer_note, mm3};
use crate::panels::pcb_primitive_properties::TrackFacts;

/// The width a mixed or unknown selection is drawn with, mm.
const STAND_IN_WIDTH_MM: f64 = 0.2;
const LOWER_Y: f32 = 96.0;
const UPPER_Y: f32 = 52.0;

pub(in crate::panels::pcb_primitive_properties) fn illustration(
    i18n: &I18n,
    f: &TrackFacts,
) -> Illustration {
    let placing = f.common.target.is_placement();
    let width = f.width.unwrap_or(STAND_IN_WIDTH_MM).max(0.01);
    let scale = (40.0_f64).min(20.0 / width);
    let ws = (width * scale) as f32;
    let end_x = if placing { 196.0 } else { 240.0 };
    let run = vec![
        at(22.0, LOWER_Y),
        at(96.0, LOWER_Y),
        at(140.0, UPPER_Y),
        at(end_x, UPPER_Y),
    ];
    let layer = f
        .layer
        .as_ref()
        .map_or(PcbLayer::Copper(CopperLayer::Top), |l| {
            PcbLayer::Copper(l.layer)
        });
    let mut drawing = canvas().part(band(
        run,
        ws,
        super::layer_paint(layer),
        &[keys::WIDTH, keys::NET, keys::LAYER, keys::ENDS],
    ));
    if placing {
        drawing = drawing
            .part(dashed(at(196.0, UPPER_Y), at(240.0, UPPER_Y), &[]))
            .parts(cross(at(240.0, UPPER_Y), 6.0, &[]))
            .part(label(
                at(240.0, UPPER_Y - 10.0),
                t!(i18n, ids::PCB_PRIM_DRAWING_CURSOR),
                Anchor::Middle,
                TextRole::Note,
                &[],
            ));
    }
    let width_label = t!(i18n, ids::PCB_PRIM_DRAWING_WIDTH_VALUE, "value" => mm3(width));
    drawing = drawing.part(callout(
        at(60.0, LOWER_Y - ws / 2.0),
        at(60.0, LOWER_Y + ws / 2.0),
        Some((
            width_label.into(),
            at(60.0, LOWER_Y + ws / 2.0 + 14.0),
            Anchor::Middle,
        )),
        &[keys::WIDTH],
    ));
    if let Some(name) = f.common.net_info.name.as_ref() {
        let net: Label<'static> = if placing {
            t!(i18n, ids::PCB_PRIM_DRAWING_NEXT, "net" => name.clone()).into()
        } else {
            UserText::new(name.clone()).into()
        };
        drawing = drawing.part(label(
            at(
                if placing { 168.0 } else { 190.0 },
                UPPER_Y - ws / 2.0 - 9.0,
            ),
            net,
            Anchor::Middle,
            TextRole::Note,
            &[keys::NET],
        ));
    }
    let name = f.layer.as_ref().map_or("", |l| l.label.as_str());
    let place = f
        .layer
        .as_ref()
        .and_then(|l| f.layers.iter().position(|c| c.layer == l.layer));
    drawing.part(layer_note(i18n, &layer_label(i18n, layer, place, name)))
}

#[cfg(test)]
mod tests {
    use pcb_geom::ItemRef;
    use signex_widgets::property_illustration::{Primitive, TextRole};
    use uuid::Uuid;

    use super::super::fixture::{common, lights};
    use super::*;

    fn facts(items: Vec<ItemRef>) -> TrackFacts {
        TrackFacts {
            common: common(items),
            location: None,
            layer: None,
            layers: Vec::new(),
            width: Some(0.25),
            start: (None, None),
            end: (None, None),
            length_mm: None,
            locked: Some(false),
            paste_mask: None,
            solder_mask: None,
        }
    }

    #[test]
    fn the_width_field_lights_its_callout_and_the_layer_row_its_note() {
        let i18n = I18n::en_us();
        let drawing = illustration(&i18n, &facts(vec![ItemRef::Track(Uuid::nil())]));
        assert!(lights(&drawing, &keys::WIDTH));
        assert!(lights(&drawing, &keys::LAYER));
        let callout = drawing
            .visible_parts()
            .find(|p| matches!(p.primitive, Primitive::Callout(_)))
            .expect("a width callout");
        assert_eq!(callout.primary_field(), Some(&keys::WIDTH));
    }

    #[test]
    fn placing_draws_the_run_to_the_cursor() {
        let i18n = I18n::en_us();
        let drawing = illustration(&i18n, &facts(Vec::new()));
        let notes = drawing
            .visible_parts()
            .filter(|p| matches!(&p.primitive, Primitive::Label(l) if l.role == TextRole::Note))
            .count();
        assert_eq!(notes, 2, "the cursor label and the layer note");
    }
}
