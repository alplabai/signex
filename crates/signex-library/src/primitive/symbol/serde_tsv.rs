//! Pin TSV codec + token conversions for the symbol wire format.

use super::*;

// ---- Pin TSV codec --------------------------------------------------

pub(super) fn pin_direction_token(d: PinDirection) -> &'static str {
    match d {
        PinDirection::Input => "Input",
        PinDirection::Output => "Output",
        PinDirection::Bidirectional => "Bidirectional",
        PinDirection::Power => "Power",
        PinDirection::Passive => "Passive",
        PinDirection::OpenCollector => "OpenCollector",
        PinDirection::OpenEmitter => "OpenEmitter",
        PinDirection::NotConnected => "NotConnected",
        PinDirection::Tristate => "Tristate",
        PinDirection::Unspecified => "Unspecified",
    }
}

pub(super) fn pin_direction_from_token(s: &str) -> Result<PinDirection, SymbolFileError> {
    Ok(match s {
        "Input" => PinDirection::Input,
        "Output" => PinDirection::Output,
        "Bidirectional" => PinDirection::Bidirectional,
        "Power" => PinDirection::Power,
        "Passive" => PinDirection::Passive,
        "OpenCollector" => PinDirection::OpenCollector,
        "OpenEmitter" => PinDirection::OpenEmitter,
        "NotConnected" => PinDirection::NotConnected,
        "Tristate" => PinDirection::Tristate,
        "Unspecified" => PinDirection::Unspecified,
        other => {
            return Err(SymbolFileError::UnknownEnumToken {
                kind: "PinDirection",
                got: other.to_string(),
            });
        }
    })
}

pub(super) fn pin_orientation_token(o: PinOrientation) -> &'static str {
    match o {
        PinOrientation::Up => "Up",
        PinOrientation::Down => "Down",
        PinOrientation::Left => "Left",
        PinOrientation::Right => "Right",
    }
}

pub(super) fn pin_orientation_from_token(s: &str) -> Result<PinOrientation, SymbolFileError> {
    Ok(match s {
        "Up" => PinOrientation::Up,
        "Down" => PinOrientation::Down,
        "Left" => PinOrientation::Left,
        "Right" => PinOrientation::Right,
        other => {
            return Err(SymbolFileError::UnknownEnumToken {
                kind: "PinOrientation",
                got: other.to_string(),
            });
        }
    })
}

pub(super) fn pin_symbol_kind_token(k: PinSymbolKind) -> &'static str {
    match k {
        PinSymbolKind::None => "None",
        PinSymbolKind::Dot => "Dot",
        PinSymbolKind::ClockEdge => "ClockEdge",
        PinSymbolKind::ActiveLowInput => "ActiveLowInput",
        PinSymbolKind::ActiveLowOutput => "ActiveLowOutput",
        PinSymbolKind::SchmittTrigger => "SchmittTrigger",
        PinSymbolKind::Analog => "Analog",
        PinSymbolKind::Digital => "Digital",
        PinSymbolKind::ShiftRight => "ShiftRight",
        PinSymbolKind::ShiftLeft => "ShiftLeft",
        PinSymbolKind::Pi => "Pi",
        PinSymbolKind::Sigma => "Sigma",
        PinSymbolKind::OpenCollector => "OpenCollector",
        PinSymbolKind::OpenEmitter => "OpenEmitter",
        PinSymbolKind::HiZ => "HiZ",
    }
}

pub(super) fn pin_symbol_kind_from_token(s: &str) -> Result<PinSymbolKind, SymbolFileError> {
    Ok(match s {
        "None" => PinSymbolKind::None,
        "Dot" => PinSymbolKind::Dot,
        "ClockEdge" => PinSymbolKind::ClockEdge,
        "ActiveLowInput" => PinSymbolKind::ActiveLowInput,
        "ActiveLowOutput" => PinSymbolKind::ActiveLowOutput,
        "SchmittTrigger" => PinSymbolKind::SchmittTrigger,
        "Analog" => PinSymbolKind::Analog,
        "Digital" => PinSymbolKind::Digital,
        "ShiftRight" => PinSymbolKind::ShiftRight,
        "ShiftLeft" => PinSymbolKind::ShiftLeft,
        "Pi" => PinSymbolKind::Pi,
        "Sigma" => PinSymbolKind::Sigma,
        "OpenCollector" => PinSymbolKind::OpenCollector,
        "OpenEmitter" => PinSymbolKind::OpenEmitter,
        "HiZ" => PinSymbolKind::HiZ,
        other => {
            return Err(SymbolFileError::UnknownEnumToken {
                kind: "PinSymbolKind",
                got: other.to_string(),
            });
        }
    })
}

/// Format an `f64` for a TSV cell. `0.0` emits literally as `"0"` so
/// the most common default is short; non-zero values use the
/// shortest precision-preserving form via `Display`. Cells must be
/// re-parseable by `f64::from_str`.
///
/// `None` for NaN / ±Inf: `Display` would write `NaN` / `inf`, which
/// the reader refuses (#678), and an empty cell would silently reset the
/// value on reopen. The caller turns it into a save error naming the
/// pin and column (#828).
fn fmt_f64(v: f64) -> Option<String> {
    if v == 0.0 {
        Some("0".to_string())
    } else if !v.is_finite() {
        None
    } else {
        Some(format!("{v}"))
    }
}

/// An optional cell: `None` is an empty cell, a non-finite value is the
/// error case reported by the caller.
fn fmt_opt_f64(v: Option<f64>) -> Option<String> {
    match v {
        None => Some(String::new()),
        Some(v) => fmt_f64(v),
    }
}

fn parse_f64_cell(col: &'static str, s: &str) -> Result<f64, SymbolFileError> {
    // #678: reject `NaN` / `inf` (and `1e999`, which overflows to inf).
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| SymbolFileError::InvalidNumericCell {
            column: col,
            value: s.to_string(),
        })
}

fn parse_opt_f64_cell(col: &'static str, s: &str) -> Result<Option<f64>, SymbolFileError> {
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_f64_cell(col, s)?))
    }
}

fn parse_bool_cell(col: &'static str, s: &str) -> Result<bool, SymbolFileError> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(SymbolFileError::InvalidBoolCell {
            column: col,
            value: s.to_string(),
        }),
    }
}

fn pin_to_tsv_row(pin: &SymbolPin, with_body: bool) -> Result<String, SymbolFileError> {
    let function_str = pin.function.join("|");
    let non_finite = |column: &str| SymbolFileError::NonFiniteField {
        field: format!("pin {:?} {column}", pin.number),
    };
    let num = |v: f64, column: &str| fmt_f64(v).ok_or_else(|| non_finite(column));
    let opt = |v: Option<f64>, column: &str| fmt_opt_f64(v).ok_or_else(|| non_finite(column));
    let mut cells: Vec<String> = vec![
        pin.number.clone(),
        pin.name.clone(),
        pin_direction_token(pin.electrical).to_string(),
        num(pin.position[0], "pos_x")?,
        num(pin.position[1], "pos_y")?,
        pin_orientation_token(pin.orientation).to_string(),
        num(pin.length, "length")?,
        pin.description.clone(),
        function_str,
        opt(pin.pin_package_length, "pin_package_length")?,
        opt(pin.propagation_delay_ns, "propagation_delay_ns")?,
        pin.designator_visible.to_string(),
        pin.name_visible.to_string(),
        pin_symbol_kind_token(pin.inside_symbol).to_string(),
        pin_symbol_kind_token(pin.inside_edge_symbol).to_string(),
        pin_symbol_kind_token(pin.outside_edge_symbol).to_string(),
        pin_symbol_kind_token(pin.outside_symbol).to_string(),
        pin.hidden.to_string(),
        pin.locked.to_string(),
        pin.part_number.to_string(),
    ];
    if with_body {
        cells.push(pin.body_style.to_string());
    }
    let columns = PIN_TSV_COLUMNS
        .iter()
        .chain(with_body.then_some(&PIN_TSV_BODY_COLUMN));
    for (col, cell) in columns.zip(cells.iter()) {
        if cell.contains('\t') || cell.contains('\n') || cell.contains("'''") {
            return Err(SymbolFileError::InvalidTsvCell {
                column: col,
                value: cell.clone(),
            });
        }
    }
    Ok(cells.join("\t"))
}

/// Encode a slice of pins as TSV — header row first, then one row
/// per pin. Empty slice still emits the header row so the round-trip
/// produces a parseable block.
///
/// The legacy 20-column layout is written unless some pin is off the
/// normal body, in which case the `body_style` column is appended.
pub(crate) fn pins_to_tsv(pins: &[SymbolPin]) -> Result<String, SymbolFileError> {
    let with_body = pins.iter().any(|p| p.body_style != BODY_NORMAL);
    let mut out = String::new();
    out.push_str(&PIN_TSV_COLUMNS.join("\t"));
    if with_body {
        out.push('\t');
        out.push_str(PIN_TSV_BODY_COLUMN);
    }
    out.push('\n');
    for pin in pins {
        out.push_str(&pin_to_tsv_row(pin, with_body)?);
        out.push('\n');
    }
    Ok(out)
}

/// Parse a `pins_tsv` payload back into `Vec<SymbolPin>`. The first
/// non-empty line is the header and must equal [`PIN_TSV_COLUMNS`];
/// each subsequent line is a pin row.
pub(crate) fn pins_from_tsv(tsv: &str) -> Result<Vec<SymbolPin>, SymbolFileError> {
    let trimmed = tsv.trim_matches('\n');
    if trimmed.is_empty() {
        return Err(SymbolFileError::EmptyPinsTsv);
    }
    let mut lines = trimmed.split('\n');
    let header = lines.next().ok_or(SymbolFileError::EmptyPinsTsv)?;
    let header_cols: Vec<&str> = header.split('\t').collect();
    // Legacy files have the 20 base columns; a De Morgan file appends one.
    let with_body = header_cols.len() == PIN_TSV_COLUMNS.len() + 1;
    let expected_len = PIN_TSV_COLUMNS.len() + usize::from(with_body);
    let expected = PIN_TSV_COLUMNS
        .iter()
        .chain(with_body.then_some(&PIN_TSV_BODY_COLUMN));
    if header_cols.len() != expected_len || header_cols.iter().zip(expected).any(|(g, e)| g != e) {
        return Err(SymbolFileError::PinsTsvSchemaMismatch {
            got: header_cols.iter().map(|s| (*s).to_string()).collect(),
        });
    }
    let mut pins = Vec::new();
    for (row_idx, line) in lines.enumerate() {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != expected_len {
            return Err(SymbolFileError::PinsTsvCellCountMismatch {
                row_index: row_idx,
                got: cells.len(),
                expected: expected_len,
            });
        }
        pins.push(pin_from_tsv_row(&cells)?);
    }
    Ok(pins)
}

fn pin_from_tsv_row(cells: &[&str]) -> Result<SymbolPin, SymbolFileError> {
    let part_number_raw = cells[19];
    let part_number: u8 =
        part_number_raw
            .parse()
            .map_err(|_| SymbolFileError::InvalidNumericCell {
                column: "part_number",
                value: part_number_raw.to_string(),
            })?;
    // Absent column = a legacy file: every pin is on the normal body.
    let body_style = match cells.get(20) {
        None => BODY_NORMAL,
        Some(raw) => match raw.parse::<u8>() {
            Ok(tag @ (BODY_COMMON | BODY_NORMAL | BODY_ALTERNATE)) => tag,
            _ => {
                return Err(SymbolFileError::InvalidNumericCell {
                    column: "body_style",
                    value: (*raw).to_string(),
                });
            }
        },
    };
    Ok(SymbolPin {
        number: cells[0].to_string(),
        name: cells[1].to_string(),
        electrical: pin_direction_from_token(cells[2])?,
        position: [
            parse_f64_cell("pos_x", cells[3])?,
            parse_f64_cell("pos_y", cells[4])?,
        ],
        orientation: pin_orientation_from_token(cells[5])?,
        length: parse_f64_cell("length", cells[6])?,
        description: cells[7].to_string(),
        function: if cells[8].is_empty() {
            Vec::new()
        } else {
            cells[8].split('|').map(str::to_string).collect()
        },
        pin_package_length: parse_opt_f64_cell("pin_package_length", cells[9])?,
        propagation_delay_ns: parse_opt_f64_cell("propagation_delay_ns", cells[10])?,
        designator_visible: parse_bool_cell("designator_visible", cells[11])?,
        name_visible: parse_bool_cell("name_visible", cells[12])?,
        inside_symbol: pin_symbol_kind_from_token(cells[13])?,
        inside_edge_symbol: pin_symbol_kind_from_token(cells[14])?,
        outside_edge_symbol: pin_symbol_kind_from_token(cells[15])?,
        outside_symbol: pin_symbol_kind_from_token(cells[16])?,
        hidden: parse_bool_cell("hidden", cells[17])?,
        locked: parse_bool_cell("locked", cells[18])?,
        part_number,
        body_style,
    })
}

#[cfg(test)]
mod non_finite_cell_tests {
    use super::*;

    /// #678: `f64::from_str` accepts `NaN` / `inf` and overflows `1e999`;
    /// a numeric cell must reject all of them.
    #[test]
    fn numeric_cell_rejects_non_finite() {
        for bad in ["NaN", "nan", "inf", "-inf", "infinity", "1e999"] {
            assert!(
                matches!(
                    parse_f64_cell("col", bad),
                    Err(SymbolFileError::InvalidNumericCell { .. })
                ),
                "{bad:?} must be rejected"
            );
        }
        assert_eq!(parse_f64_cell("col", "1.5").unwrap(), 1.5);
    }
}
