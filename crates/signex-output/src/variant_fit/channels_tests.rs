//! #2662: per-channel variant state. A sheet repeated as three channels
//! lists each channel's copy of a part as a row of its own; the copies are
//! found together, their fit per variant reads as fitted, not fitted or
//! mixed, and the per-variant BOM drops exactly the copies a variant does
//! not fit.

use signex_types::net::SheetPlacement;
use signex_types::pcb::SymbolLink;
use signex_types::project::{PartRef, ProjectData, VariantPart, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::*;
use crate::bom::{BomOptions, rollup};
use crate::{ExportContext, ProjectMetadata};

const SHEET_SYMBOL: u128 = 0x2662_0000;

/// The placement of channel `n` (from 1) of the repeated sheet symbol.
fn channel_path(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![
        Uuid::from_u128(SHEET_SYMBOL),
        Uuid::from_u128(SHEET_SYMBOL + n),
    ])
}

/// The repeated placement every channel shares.
fn logical() -> InstancePath {
    InstancePath::from_chain(vec![Uuid::from_u128(SHEET_SYMBOL)])
}

fn resistor(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    }
}

/// R5 and C6 on one sheet placed as CH1 to CH3, each copy named
/// `CH<n>_<designator>` as the app stamps it.
fn channel_sheet() -> (SchematicSheet, MatrixChannels) {
    let mut symbols = vec![resistor("R5", "47k"), resistor("C6", "100n")];
    let mut channels = MatrixChannels::default();
    for n in 1..=3 {
        let path = channel_path(n);
        for symbol in &mut symbols {
            let name = format!("CH{n}_{}", symbol.reference);
            symbol.set_instance_designator(&path, name);
        }
        channels.insert(path, logical(), format!("CH{n}"));
    }
    let sheet = SchematicSheet {
        symbols,
        ..Default::default()
    };
    (sheet, channels)
}

fn context(sheet: SchematicSheet, settings: VariantSettings) -> ExportContext {
    ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata {
            variant_settings: settings,
            ..ProjectMetadata::default()
        },
        netlist: None,
        netlist_sheets: vec![sheet],
        netlist_placements: (1..=3)
            .map(|n| SheetPlacement {
                sheet_index: 0,
                instance_path: channel_path(n),
            })
            .collect(),
        current_sheet: None,
    }
}

/// The copy of `symbol` in channel `n`, as the settings name it.
fn copy_of(symbol: &Symbol, n: u128) -> PartRef {
    PartRef::new(
        format!("CH{n}_{}", symbol.reference),
        Some(SymbolLink::new(channel_path(n), [symbol.uuid])),
    )
}

/// Lite leaves R5 out in CH2 only and C6 out in every channel.
fn lite(sheet: &SchematicSheet) -> VariantSettings {
    let (r5, c6) = (&sheet.symbols[0], &sheet.symbols[1]);
    let mut data: ProjectData =
        serde_json::from_str(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#)
            .unwrap();
    data.add_variant("Lite").unwrap();
    data.set_variant_parts_fit("Lite", &[copy_of(r5, 2)], Some(false))
        .unwrap();
    let every_c6: Vec<PartRef> = (1..=3).map(|n| copy_of(c6, n)).collect();
    data.set_variant_parts_fit("Lite", &every_c6, Some(false))
        .unwrap();
    data.variant_settings
}

#[test]
fn each_channel_copy_is_a_row_marked_with_its_channel() {
    let (sheet, channels) = channel_sheet();
    let r5 = sheet.symbols[0].uuid;
    let matrix = variant_matrix_in_channels(
        &context(sheet, VariantSettings::default()),
        &["Lite".to_string()],
        &channels,
    );
    let r5_part = SymbolLink::new(logical(), [r5]);
    let copies: Vec<(&str, &str, &str)> = matrix
        .channel_copies(&r5_part)
        .iter()
        .map(|row| {
            let channel = row.channel.as_ref().unwrap();
            (
                row.designator.as_str(),
                channel.name.as_str(),
                channel.designator.as_str(),
            )
        })
        .collect();
    assert_eq!(
        copies,
        [
            ("CH1_R5", "CH1", "R5"),
            ("CH2_R5", "CH2", "R5"),
            ("CH3_R5", "CH3", "R5"),
        ]
    );
    assert_eq!(matrix.rows.len(), 6);

    // Without the channel placements no row is a channel copy.
    let (sheet, _) = channel_sheet();
    let plain = variant_matrix(&context(sheet, VariantSettings::default()), &[]);
    assert!(plain.rows.iter().all(|row| row.channel.is_none()));
}

#[test]
fn the_copies_fit_read_as_mixed_when_the_channels_disagree() {
    let (sheet, channels) = channel_sheet();
    let settings = lite(&sheet);
    let (r5, c6) = (sheet.symbols[0].uuid, sheet.symbols[1].uuid);
    let matrix = variant_matrix_in_channels(
        &context(sheet, settings.clone()),
        &["Lite".to_string()],
        &channels,
    )
    .with_settings(&settings);
    let r5_part = SymbolLink::new(logical(), [r5]);
    let c6_part = SymbolLink::new(logical(), [c6]);
    assert_eq!(matrix.channel_fit(&r5_part, 0), ChannelFit::Fitted);
    assert_eq!(
        matrix.channel_fit(&r5_part, 1),
        ChannelFit::Mixed {
            not_fitted: 1,
            of: 3
        }
    );
    assert_eq!(matrix.channel_fit(&c6_part, 1), ChannelFit::NotFitted);
}

/// The roadmap check: a multi-channel design with two variants exports a
/// correct per-variant BOM.
#[test]
fn the_per_variant_bom_drops_the_copies_the_variant_does_not_fit() {
    let (sheet, _) = channel_sheet();
    let settings = lite(&sheet);
    let ctx = context(sheet, settings);
    let fitted = |variant: Option<&str>| {
        let table = rollup(
            &ctx,
            &BomOptions {
                active_variant: variant.map(str::to_string),
                ..BomOptions::default()
            },
        );
        let mut references: Vec<String> = table
            .rows
            .iter()
            .filter(|row| row.fitted_qty > 0)
            .flat_map(|row| row.references.clone())
            .collect();
        references.sort();
        references
    };
    assert_eq!(
        fitted(None),
        ["CH1_C6", "CH1_R5", "CH2_C6", "CH2_R5", "CH3_C6", "CH3_R5"]
    );
    assert_eq!(fitted(Some("Lite")), ["CH1_R5", "CH3_R5"]);
}

/// An entry keyed by one channel's copy never lands on another channel's,
/// even when it carries that channel's designator (a naming change).
#[test]
fn a_channel_entry_never_names_another_channels_copy() {
    let (sheet, _) = channel_sheet();
    let r5 = &sheet.symbols[0];
    let settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "CH1_R5".into(),
            part: copy_of(r5, 2).part,
            fitted: Some(false),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    assert_eq!(
        project_fit(&settings, Some("Lite"), copy_of(r5, 1).placed()),
        None
    );
    assert_eq!(
        project_fit(&settings, Some("Lite"), copy_of(r5, 2).placed()),
        Some(false)
    );
}
