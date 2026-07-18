//! Pad TSV codec + token conversions for the footprint wire format.

use super::*;

// ---- Pad TSV codec --------------------------------------------------

pub(super) fn pad_kind_token(k: PadKind) -> &'static str {
    match k {
        PadKind::Smd => "Smd",
        PadKind::Tht => "Tht",
        PadKind::NptHole => "NptHole",
        PadKind::ConnectorPad => "ConnectorPad",
        PadKind::Castellated => "Castellated",
        PadKind::Fiducial => "Fiducial",
    }
}

pub(super) fn pad_kind_from_token(s: &str) -> Result<PadKind, FootprintFileError> {
    Ok(match s {
        "Smd" => PadKind::Smd,
        "Tht" => PadKind::Tht,
        "NptHole" => PadKind::NptHole,
        "ConnectorPad" => PadKind::ConnectorPad,
        "Castellated" => PadKind::Castellated,
        "Fiducial" => PadKind::Fiducial,
        other => {
            return Err(FootprintFileError::UnknownEnumToken {
                kind: "PadKind",
                got: other.to_string(),
            });
        }
    })
}

/// HI-10: see [`crate::primitive::symbol::fmt_f64`] — same NaN/inf guard.
fn fmt_f64_fp(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else if !v.is_finite() {
        debug_assert!(v.is_finite(), "fmt_f64_fp called with non-finite {v}");
        String::new()
    } else {
        format!("{v}")
    }
}

fn fmt_opt_f64_fp(v: Option<f64>) -> String {
    v.map(fmt_f64_fp).unwrap_or_default()
}

pub(super) fn pad_shape_to_token(shape: &PadShape) -> Result<String, FootprintFileError> {
    Ok(match shape {
        PadShape::Round => "round".to_string(),
        PadShape::Rect => "rect".to_string(),
        PadShape::Oval => "oval".to_string(),
        PadShape::RoundRect { radius_ratio } => {
            format!("round_rect:{}", fmt_f64_fp(*radius_ratio))
        }
        PadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => {
            let bits = format!(
                "{}{}{}{}",
                bool_bit(corners.top_left),
                bool_bit(corners.top_right),
                bool_bit(corners.bottom_left),
                bool_bit(corners.bottom_right),
            );
            format!("chamfered:{}:{}", fmt_f64_fp(*chamfer_ratio), bits)
        }
        PadShape::Custom(poly) => {
            let mut parts: Vec<String> = Vec::with_capacity(poly.points.len());
            for p in &poly.points {
                parts.push(format!("{},{}", fmt_f64_fp(p[0]), fmt_f64_fp(p[1])));
            }
            format!("custom:{}", parts.join("|"))
        }
    })
}

fn bool_bit(b: bool) -> char {
    if b { '1' } else { '0' }
}

pub(super) fn pad_shape_from_token(s: &str) -> Result<PadShape, FootprintFileError> {
    let invalid = || FootprintFileError::InvalidPadShape(s.to_string());
    if s == "round" {
        return Ok(PadShape::Round);
    }
    if s == "rect" {
        return Ok(PadShape::Rect);
    }
    if s == "oval" {
        return Ok(PadShape::Oval);
    }
    if let Some(rest) = s.strip_prefix("round_rect:") {
        let radius_ratio: f64 = rest.parse().map_err(|_| invalid())?;
        return Ok(PadShape::RoundRect { radius_ratio });
    }
    if let Some(rest) = s.strip_prefix("chamfered:") {
        let mut parts = rest.splitn(2, ':');
        let ratio_str = parts.next().ok_or_else(invalid)?;
        let bits_str = parts.next().ok_or_else(invalid)?;
        let chamfer_ratio: f64 = ratio_str.parse().map_err(|_| invalid())?;
        let bits: Vec<char> = bits_str.chars().collect();
        if bits.len() != 4 || bits.iter().any(|c| *c != '0' && *c != '1') {
            return Err(invalid());
        }
        let corners = ChamferedCorners {
            top_left: bits[0] == '1',
            top_right: bits[1] == '1',
            bottom_left: bits[2] == '1',
            bottom_right: bits[3] == '1',
        };
        return Ok(PadShape::Chamfered {
            chamfer_ratio,
            corners,
        });
    }
    if let Some(rest) = s.strip_prefix("custom:") {
        let points: Vec<[f64; 2]> = if rest.is_empty() {
            Vec::new()
        } else {
            let mut points = Vec::new();
            for p in rest.split('|') {
                let mut xy = p.split(',');
                let x_str = xy.next().ok_or_else(invalid)?;
                let y_str = xy.next().ok_or_else(invalid)?;
                if xy.next().is_some() {
                    return Err(invalid());
                }
                let x: f64 = x_str.parse().map_err(|_| invalid())?;
                let y: f64 = y_str.parse().map_err(|_| invalid())?;
                points.push([x, y]);
            }
            points
        };
        return Ok(PadShape::Custom(Polygon::new(points)));
    }
    Err(invalid())
}

fn layers_to_token(layers: &[LayerId]) -> Result<String, FootprintFileError> {
    for layer in layers {
        if layer.as_str().contains('|') {
            return Err(FootprintFileError::InvalidTsvCell {
                column: "layers",
                value: layer.as_str().to_string(),
            });
        }
    }
    Ok(layers
        .iter()
        .map(|l| l.as_str())
        .collect::<Vec<&str>>()
        .join("|"))
}

fn layers_from_token(s: &str) -> Vec<LayerId> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split('|').map(LayerId::new).collect()
    }
}

fn parse_f64_cell_fp(col: &'static str, s: &str) -> Result<f64, FootprintFileError> {
    s.parse()
        .map_err(|_| FootprintFileError::InvalidNumericCell {
            column: col,
            value: s.to_string(),
        })
}

fn parse_opt_f64_cell_fp(col: &'static str, s: &str) -> Result<Option<f64>, FootprintFileError> {
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_f64_cell_fp(col, s)?))
    }
}

fn pad_to_tsv_row(pad: &Pad) -> Result<String, FootprintFileError> {
    let shape_cell = pad_shape_to_token(&pad.shape)?;
    let layers_cell = layers_to_token(&pad.layers)?;
    let drill_diameter_cell = pad
        .drill
        .as_ref()
        .map(|d| fmt_f64_fp(d.diameter))
        .unwrap_or_default();
    let drill_slot_cell = pad
        .drill
        .as_ref()
        .and_then(|d| d.slot_length)
        .map(fmt_f64_fp)
        .unwrap_or_default();
    let cells: [String; 13] = [
        pad.number.clone(),
        pad_kind_token(pad.kind).to_string(),
        shape_cell,
        fmt_f64_fp(pad.size[0]),
        fmt_f64_fp(pad.size[1]),
        fmt_f64_fp(pad.position[0]),
        fmt_f64_fp(pad.position[1]),
        fmt_f64_fp(pad.rotation),
        layers_cell,
        drill_diameter_cell,
        drill_slot_cell,
        fmt_opt_f64_fp(pad.solder_mask_margin),
        fmt_opt_f64_fp(pad.paste_margin),
    ];
    for (col, cell) in PAD_TSV_COLUMNS.iter().zip(cells.iter()) {
        if cell.contains('\t') || cell.contains('\n') || cell.contains("'''") {
            return Err(FootprintFileError::InvalidTsvCell {
                column: col,
                value: cell.clone(),
            });
        }
    }
    Ok(cells.join("\t"))
}

/// Encode a slice of pads as TSV — header row first, then one row
/// per pad. Empty slice still emits the header row.
pub(crate) fn pads_to_tsv(pads: &[Pad]) -> Result<String, FootprintFileError> {
    let mut out = String::new();
    out.push_str(&PAD_TSV_COLUMNS.join("\t"));
    out.push('\n');
    for pad in pads {
        out.push_str(&pad_to_tsv_row(pad)?);
        out.push('\n');
    }
    Ok(out)
}

/// Parse a `pads_tsv` payload back into `Vec<Pad>`. The first non-
/// empty line is the header and must equal [`PAD_TSV_COLUMNS`]; each
/// subsequent line is a pad row.
pub(crate) fn pads_from_tsv(tsv: &str) -> Result<Vec<Pad>, FootprintFileError> {
    let trimmed = tsv.trim_matches('\n');
    if trimmed.is_empty() {
        return Err(FootprintFileError::EmptyPadsTsv);
    }
    let mut lines = trimmed.split('\n');
    let header = lines.next().ok_or(FootprintFileError::EmptyPadsTsv)?;
    let header_cols: Vec<&str> = header.split('\t').collect();
    if header_cols.len() != PAD_TSV_COLUMNS.len()
        || header_cols
            .iter()
            .zip(PAD_TSV_COLUMNS.iter())
            .any(|(g, e)| g != e)
    {
        return Err(FootprintFileError::PadsTsvSchemaMismatch {
            got: header_cols.iter().map(|s| (*s).to_string()).collect(),
        });
    }
    let mut pads = Vec::new();
    for (row_idx, line) in lines.enumerate() {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != PAD_TSV_COLUMNS.len() {
            return Err(FootprintFileError::PadsTsvCellCountMismatch {
                row_index: row_idx,
                got: cells.len(),
                expected: PAD_TSV_COLUMNS.len(),
            });
        }
        pads.push(pad_from_tsv_row(&cells)?);
    }
    Ok(pads)
}

fn pad_from_tsv_row(cells: &[&str]) -> Result<Pad, FootprintFileError> {
    let drill = if cells[9].is_empty() {
        if !cells[10].is_empty() {
            return Err(FootprintFileError::InvalidNumericCell {
                column: "drill_slot_length",
                value: format!(
                    "drill_slot_length set ({:?}) without a drill_diameter",
                    cells[10]
                ),
            });
        }
        None
    } else {
        Some(Drill {
            diameter: parse_f64_cell_fp("drill_diameter", cells[9])?,
            slot_length: parse_opt_f64_cell_fp("drill_slot_length", cells[10])?,
        })
    };
    Ok(Pad {
        number: cells[0].to_string(),
        kind: pad_kind_from_token(cells[1])?,
        shape: pad_shape_from_token(cells[2])?,
        size: [
            parse_f64_cell_fp("size_x", cells[3])?,
            parse_f64_cell_fp("size_y", cells[4])?,
        ],
        position: [
            parse_f64_cell_fp("pos_x", cells[5])?,
            parse_f64_cell_fp("pos_y", cells[6])?,
        ],
        rotation: parse_f64_cell_fp("rotation", cells[7])?,
        layers: layers_from_token(cells[8]),
        drill,
        solder_mask_margin: parse_opt_f64_cell_fp("solder_mask_margin", cells[11])?,
        paste_margin: parse_opt_f64_cell_fp("paste_margin", cells[12])?,
        ..Pad::default()
    })
}

/// Error variants raised by [`FootprintFile`] parsers + serialisers.
#[derive(Debug, thiserror::Error)]
pub enum FootprintFileError {
    #[error("empty .snxfpt file")]
    Empty,
    #[error("invalid UTF-8 in TOML payload: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("TOML deserialise failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("TOML serialise failed: {0}")]
    TomlSerialize(toml::ser::Error),
    #[error("unsupported .snxfpt format token {got:?}; this build supports \"snxfpt/1\"")]
    UnsupportedFormat { got: String },
    #[error(
        "TSV cell in column {column:?} contains a tab, newline, or triple-quote: \
         {value:?}; cells must be free of \\t, \\n, and the literal \"'''\""
    )]
    InvalidTsvCell { column: &'static str, value: String },
    #[error("pads_tsv block is empty (no header row)")]
    EmptyPadsTsv,
    #[error("pads_tsv header does not match the expected schema; got columns {got:?}")]
    PadsTsvSchemaMismatch { got: Vec<String> },
    #[error("pads_tsv row {row_index} has {got} cells; header declares {expected}")]
    PadsTsvCellCountMismatch {
        row_index: usize,
        got: usize,
        expected: usize,
    },
    #[error("unknown {kind} token {got:?} in pads_tsv cell")]
    UnknownEnumToken { kind: &'static str, got: String },
    #[error("invalid pad shape token {0:?}")]
    InvalidPadShape(String),
    #[error("invalid numeric cell in column {column:?}: {value:?}")]
    InvalidNumericCell { column: &'static str, value: String },
}
