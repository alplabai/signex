//! Optional pad TSV columns — the `Pad` fields beyond the 13 base
//! columns in `PAD_TSV_COLUMNS`.
//!
//! Format decision (#960): extra columns on the existing `pads_tsv`
//! table, not a sidecar. Every extra field is a scalar, a short enum
//! token or a 4-bit flag word, so it fits one cell, stays line-diffable
//! and rides the same atomic file write as the rest of the footprint.
//!
//! A column is *emitted only when at least one pad in the footprint
//! holds a non-default value for it*, always in [`EXTENDED_COLUMNS`]
//! order after the base columns. That keeps the header a pure function
//! of the content (deterministic), and files whose pads carry only
//! default values — every stock-library footprint, every pre-#960
//! file — re-serialise byte-identically. On load a missing column
//! means "`Pad::default()` value". `Pad::default()` is hand-written
//! so `paste_enabled_*` / `thermal_relief` are `true`, matching serde
//! and the editor; a typical footprint therefore needs no extra column.

use signex_sketch::attr::{ElectricalType, PadFeature, TestpointFlags};

use super::serde_tsv::{non_finite_pad, parse_opt_f64_cell_fp};
use super::{FootprintFileError, Pad};
use crate::primitive::tsv_cell::fmt_f64;

pub(super) struct ExtendedColumn {
    pub name: &'static str,
    /// Fails on a non-finite number (#828): a pad value that cannot be
    /// written must refuse the save, never become an empty cell.
    pub encode: fn(&Pad) -> Result<String, FootprintFileError>,
    pub decode: fn(&mut Pad, &str) -> Result<(), FootprintFileError>,
}

macro_rules! string_col {
    ($f:ident) => {
        ExtendedColumn {
            name: stringify!($f),
            encode: |p| Ok(p.$f.clone()),
            decode: |p, s| {
                p.$f = s.to_string();
                Ok(())
            },
        }
    };
}

macro_rules! bool_col {
    ($f:ident) => {
        ExtendedColumn {
            name: stringify!($f),
            encode: |p| Ok(bool_cell(p.$f).to_string()),
            decode: |p, s| {
                p.$f = parse_bool(s)?;
                Ok(())
            },
        }
    };
}

macro_rules! opt_f64_col {
    ($f:ident) => {
        ExtendedColumn {
            name: stringify!($f),
            encode: |p| match p.$f {
                None => Ok(String::new()),
                Some(v) => fmt_f64(v).ok_or_else(|| non_finite_pad(&p.number, stringify!($f))),
            },
            decode: |p, s| {
                p.$f = parse_opt_f64_cell_fp(stringify!($f), s)?;
                Ok(())
            },
        }
    };
}

fn bool_cell(b: bool) -> char {
    if b { '1' } else { '0' }
}

fn parse_bool(s: &str) -> Result<bool, FootprintFileError> {
    match s {
        "1" => Ok(true),
        "0" => Ok(false),
        other => Err(FootprintFileError::UnknownEnumToken {
            kind: "bool",
            got: other.to_string(),
        }),
    }
}

fn feature_to_token(f: PadFeature) -> &'static str {
    match f {
        PadFeature::None => "None",
        PadFeature::Counterbore => "Counterbore",
        PadFeature::Countersink => "Countersink",
    }
}

fn feature_from_token(s: &str) -> Result<PadFeature, FootprintFileError> {
    Ok(match s {
        "None" => PadFeature::None,
        "Counterbore" => PadFeature::Counterbore,
        "Countersink" => PadFeature::Countersink,
        other => {
            return Err(FootprintFileError::UnknownEnumToken {
                kind: "PadFeature",
                got: other.to_string(),
            });
        }
    })
}

fn electrical_to_token(e: ElectricalType) -> &'static str {
    match e {
        ElectricalType::Load => "Load",
        ElectricalType::Source => "Source",
        ElectricalType::Terminator => "Terminator",
    }
}

fn electrical_from_token(s: &str) -> Result<ElectricalType, FootprintFileError> {
    Ok(match s {
        "Load" => ElectricalType::Load,
        "Source" => ElectricalType::Source,
        "Terminator" => ElectricalType::Terminator,
        other => {
            return Err(FootprintFileError::UnknownEnumToken {
                kind: "ElectricalType",
                got: other.to_string(),
            });
        }
    })
}

/// Testpoint flags as four bits: top_assembly, top_fab,
/// bottom_assembly, bottom_fab.
fn testpoint_to_token(t: TestpointFlags) -> String {
    [t.top_assembly, t.top_fab, t.bottom_assembly, t.bottom_fab]
        .into_iter()
        .map(bool_cell)
        .collect()
}

fn testpoint_from_token(s: &str) -> Result<TestpointFlags, FootprintFileError> {
    let bits: Vec<char> = s.chars().collect();
    if bits.len() != 4 || bits.iter().any(|c| *c != '0' && *c != '1') {
        return Err(FootprintFileError::UnknownEnumToken {
            kind: "TestpointFlags",
            got: s.to_string(),
        });
    }
    Ok(TestpointFlags {
        top_assembly: bits[0] == '1',
        top_fab: bits[1] == '1',
        bottom_assembly: bits[2] == '1',
        bottom_fab: bits[3] == '1',
    })
}

/// Every `Pad` field outside the 13 base columns. Order is the on-disk
/// column order; appending is a compatible change, reordering is not.
pub(super) static EXTENDED_COLUMNS: &[ExtendedColumn] = &[
    string_col!(template),
    string_col!(template_library),
    opt_f64_col!(paste_margin_top),
    opt_f64_col!(paste_margin_bottom),
    bool_col!(paste_enabled_top),
    bool_col!(paste_enabled_bottom),
    opt_f64_col!(mask_margin_top),
    opt_f64_col!(mask_margin_bottom),
    bool_col!(mask_tented_top),
    bool_col!(mask_tented_bottom),
    bool_col!(thermal_relief),
    opt_f64_col!(corner_radius_pct),
    ExtendedColumn {
        name: "feature_top",
        encode: |p| Ok(feature_to_token(p.feature_top).to_string()),
        decode: |p, s| {
            p.feature_top = feature_from_token(s)?;
            Ok(())
        },
    },
    ExtendedColumn {
        name: "feature_bottom",
        encode: |p| Ok(feature_to_token(p.feature_bottom).to_string()),
        decode: |p, s| {
            p.feature_bottom = feature_from_token(s)?;
            Ok(())
        },
    },
    ExtendedColumn {
        name: "testpoint",
        encode: |p| Ok(testpoint_to_token(p.testpoint)),
        decode: |p, s| {
            p.testpoint = testpoint_from_token(s)?;
            Ok(())
        },
    },
    ExtendedColumn {
        name: "electrical_type",
        encode: |p| Ok(electrical_to_token(p.electrical_type).to_string()),
        decode: |p, s| {
            p.electrical_type = electrical_from_token(s)?;
            Ok(())
        },
    },
    string_col!(net),
    bool_col!(locked),
    opt_f64_col!(hole_tolerance_plus_mm),
    opt_f64_col!(hole_tolerance_minus_mm),
    opt_f64_col!(hole_rotation_deg),
    opt_f64_col!(copper_offset_x_mm),
    opt_f64_col!(copper_offset_y_mm),
];

pub(super) fn column_by_name(name: &str) -> Option<&'static ExtendedColumn> {
    EXTENDED_COLUMNS.iter().find(|c| c.name == name)
}

/// The extended columns this pad list needs: those where at least one
/// pad differs from `Pad::default()`, in canonical order.
pub(super) fn columns_needed(
    pads: &[Pad],
) -> Result<Vec<&'static ExtendedColumn>, FootprintFileError> {
    let blank = Pad::default();
    let mut needed = Vec::new();
    for col in EXTENDED_COLUMNS {
        let blank_cell = (col.encode)(&blank)?;
        // Encode every pad, so a non-finite value errors here rather than
        // silently counting as "default" and dropping the column.
        for pad in pads {
            if (col.encode)(pad)? != blank_cell {
                needed.push(col);
                break;
            }
        }
    }
    Ok(needed)
}
