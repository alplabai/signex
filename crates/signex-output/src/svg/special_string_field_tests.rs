//! #1596: a symbol's value field prints its special strings and variables
//! resolved; a power port's value is a net name and prints as stored.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use signex_types::schematic::{Point, SchematicSheet, Symbol, TextProp};

use super::*;
use crate::pdf::PdfOptions;
use crate::{ProjectMetadata, SheetSnapshot};

fn value_text() -> TextProp {
    TextProp {
        position: Point::new(40.0, 40.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: Default::default(),
        justify_v: Default::default(),
        hidden: false,
    }
}

fn symbol(reference: &str, value: &str, is_power: bool) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        reference: reference.into(),
        value: value.into(),
        is_power,
        val_text: Some(value_text()),
        ..Default::default()
    }
}

fn printed_texts(schematic: SchematicSheet) -> Vec<String> {
    let sheet = SheetSnapshot {
        path: PathBuf::from("/p/power.snxsch"),
        schematic,
        sheet_name: "Power".into(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let params = BTreeMap::from([
        ("title".to_string(), "Supply".to_string()),
        ("rev".to_string(), "C".to_string()),
        ("VCC".to_string(), "5V".to_string()),
        ("VREF".to_string(), "= VCC / 2".to_string()),
    ]);
    let mut project = ProjectMetadata::from_values(&params);
    project.project_parameters = Some(params);
    let metadata = project.for_schematic(&sheet.schematic);
    let opts = PdfOptions::default();
    let special = crate::pdf::frame::sheet_substitution(&sheet, &opts, &metadata);
    let (refdes, nets) = (HashMap::new(), HashMap::new());
    let inputs = SvgEvaluatorInputs {
        special_strings: Some(&special),
        global_refdes: &refdes,
        net_name_by_symbol_pin: &nets,
        cell: "",
    };
    let ctx = SvgRenderContext::from_sheet(&sheet, &opts, 297.0, 210.0, 1.0, Some(&inputs));
    ctx.elements
        .into_iter()
        .filter_map(|e| match e {
            SvgElement::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_value_prints_its_special_strings_and_variables_resolved() {
    let mut schematic = SchematicSheet::default();
    schematic.variables.insert("VCC".into(), "3.3V".into());
    schematic
        .symbols
        .push(symbol("U1", "${TITLE} rev ${REVISION} at ${VREF}", false));
    let texts = printed_texts(schematic.clone());
    assert!(
        texts.iter().any(|t| t == "Supply rev C at 1.65V"),
        "{texts:?}"
    );
    // The stored value keeps its tokens.
    assert_eq!(
        schematic.symbols[0].value,
        "${TITLE} rev ${REVISION} at ${VREF}"
    );
}

#[test]
fn a_power_port_value_prints_as_stored() {
    let mut schematic = SchematicSheet::default();
    schematic.symbols.push(symbol("#PWR1", "${VCC}", true));
    let texts = printed_texts(schematic);
    assert!(texts.iter().any(|t| t == "${VCC}"), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "5V"), "{texts:?}");
}
