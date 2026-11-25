//! A `[[free.dimensions]]` row of the `[free]` section (#2373), with the
//! keys of its look (#2868): arrows, text, extension lines and number
//! format.
//!
//! Every look key is optional and written only where it differs from the
//! look a dimension had before these settings, which follows its text
//! height and line width (`DimensionStyle::for_text_height`). So a
//! dimension of that look writes no new key, an older file reads with that
//! look, and an older reader skips the keys it does not know. Lengths are
//! integer nm.
//!
//! The kind keys (#2507) follow the same rule: `kind` is written only for a
//! kind other than aligned, the kind every dimension had before, with
//! `measures`, the arc or circle a radial dimension measures.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb::Point;
use crate::pcb_dimension::LinearDimension;
use crate::pcb_dimension_kind::DimensionKind;
use crate::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionStyle, DimensionTextPosition, DimensionUnits,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(in crate::format) struct DimensionRaw {
    uuid: Uuid,
    start_x: i64,
    start_y: i64,
    end_x: i64,
    end_y: i64,
    offset: i64,
    layer: String,
    line_width: i64,
    text_height: i64,
    precision: u8,
    /// Absent for an aligned dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<DimensionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    measures: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arrow_style: Option<ArrowStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arrow_length: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arrow_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<BoardFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_position: Option<DimensionTextPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_gap: Option<i64>,
    /// `LinearDimension::text_at`, written only for a Manual value and
    /// absent at the middle of the line. Any other value reads where its
    /// text position puts it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_along: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_across: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extension_offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extension_overshoot: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extension_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit: Option<DimensionUnits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    show_unit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trailing_zeros: Option<bool>,
}

fn point(x: i64, y: i64) -> Point {
    Point::new(nm_to_mm(x), nm_to_mm(y))
}

/// The dimension a row holds; a look key it lacks reads the look of its
/// text height and line width.
pub(in crate::format) fn read(d: DimensionRaw) -> LinearDimension {
    let legacy = DimensionStyle::for_text_height(nm_to_mm(d.text_height), nm_to_mm(d.line_width));
    let mm = |v: Option<i64>, or: f64| v.map_or(or, nm_to_mm);
    let style = DimensionStyle {
        arrow_style: d.arrow_style.unwrap_or(legacy.arrow_style),
        arrow_length: mm(d.arrow_length, legacy.arrow_length),
        arrow_width: mm(d.arrow_width, legacy.arrow_width),
        font: d.font.unwrap_or(legacy.font),
        bold: d.bold.unwrap_or(legacy.bold),
        italic: d.italic.unwrap_or(legacy.italic),
        text_width: mm(d.text_width, legacy.text_width),
        text_position: d.text_position.unwrap_or(legacy.text_position),
        text_gap: mm(d.text_gap, legacy.text_gap),
        extension_offset: mm(d.extension_offset, legacy.extension_offset),
        extension_overshoot: mm(d.extension_overshoot, legacy.extension_overshoot),
        extension_width: mm(d.extension_width, legacy.extension_width),
        unit: d.unit.unwrap_or(legacy.unit),
        show_unit: d.show_unit.unwrap_or(legacy.show_unit),
        prefix: d.prefix.unwrap_or(legacy.prefix),
        suffix: d.suffix.unwrap_or(legacy.suffix),
        trailing_zeros: d.trailing_zeros.unwrap_or(legacy.trailing_zeros),
    };
    let manual = Point::new(mm(d.text_along, 0.0), mm(d.text_across, 0.0));
    let mut dimension = LinearDimension {
        uuid: d.uuid,
        kind: d.kind.unwrap_or_default(),
        start: point(d.start_x, d.start_y),
        end: point(d.end_x, d.end_y),
        measures: d.measures,
        offset: nm_to_mm(d.offset),
        layer: d.layer,
        line_width: nm_to_mm(d.line_width),
        text_height: nm_to_mm(d.text_height),
        precision: d.precision,
        style,
        text_at: manual,
    };
    if dimension.style.text_position != DimensionTextPosition::Manual {
        dimension.text_at = dimension.above_text_at();
    }
    dimension
}

/// Whether saving `d` writes any key a reader from before the look
/// settings would drop: its look differs, to the nm, from the look of its
/// own text height and line width, it is Manual (its point is written only
/// then), or it is not aligned (#2507: `kind`, `measures`). A dimension the
/// file cannot hold (a number that is not finite) is refused on save and
/// answers `false`.
pub fn dimension_uses_style(d: &LinearDimension) -> bool {
    let Ok(row) = write(d) else {
        return false;
    };
    let plain = LinearDimension {
        kind: DimensionKind::Aligned,
        measures: None,
        style: DimensionStyle::for_text_height(nm_to_mm(row.text_height), nm_to_mm(row.line_width)),
        text_at: Point::new(0.0, 0.0),
        ..d.clone()
    };
    write(&plain).ok().as_ref() != Some(&row)
}

/// Whether saving `board` writes a look or kind key on any dimension
/// (#2868, #2507): the board uses a dimension look or kind a reader from
/// before the settings would drop. A board where this is `false` saves
/// exactly as it did before.
pub fn board_uses_dimension_style(board: &crate::pcb::PcbBoard) -> bool {
    board.free.dimensions.iter().any(dimension_uses_style)
}

/// `v` in nm, or `None` when it is `was` to the nm.
fn length(v: f64, was: f64, field: &'static str) -> Result<Option<i64>, NonFiniteField> {
    let nm = mm_to_nm(v, field)?;
    Ok((nm != mm_to_nm(was, field)?).then_some(nm))
}

/// One coordinate of the Manual point of `d` in nm, or `None` when `d` is
/// not Manual (where its value sits follows its look, as the app places it)
/// or the coordinate is 0, the middle of the line.
fn manual_point(
    d: &LinearDimension,
    coordinate: fn(Point) -> f64,
    field: &'static str,
) -> Result<Option<i64>, NonFiniteField> {
    if d.style.text_position != DimensionTextPosition::Manual {
        return Ok(None);
    }
    length(coordinate(d.text_at), 0.0, field)
}

/// `v`, or `None` when it is `was`.
fn changed<T: PartialEq + Clone>(v: &T, was: &T) -> Option<T> {
    (v != was).then(|| v.clone())
}

/// The row of `d`; its look keys only where they differ from the look of
/// its stored text height and line width.
pub(in crate::format) fn write(d: &LinearDimension) -> Result<DimensionRaw, NonFiniteField> {
    let text_height = mm_to_nm(d.text_height, "text_height")?;
    let line_width = mm_to_nm(d.line_width, "line_width")?;
    let s = &d.style;
    let legacy = DimensionStyle::for_text_height(nm_to_mm(text_height), nm_to_mm(line_width));
    Ok(DimensionRaw {
        uuid: d.uuid,
        start_x: mm_to_nm(d.start.x, "start_x")?,
        start_y: mm_to_nm(d.start.y, "start_y")?,
        end_x: mm_to_nm(d.end.x, "end_x")?,
        end_y: mm_to_nm(d.end.y, "end_y")?,
        offset: mm_to_nm(d.offset, "offset")?,
        layer: d.layer.clone(),
        line_width,
        text_height,
        precision: d.precision,
        kind: changed(&d.kind, &DimensionKind::Aligned),
        measures: d.measures,
        arrow_style: changed(&s.arrow_style, &legacy.arrow_style),
        arrow_length: length(s.arrow_length, legacy.arrow_length, "arrow_length")?,
        arrow_width: length(s.arrow_width, legacy.arrow_width, "arrow_width")?,
        font: changed(&s.font, &legacy.font),
        bold: changed(&s.bold, &legacy.bold),
        italic: changed(&s.italic, &legacy.italic),
        text_width: length(s.text_width, legacy.text_width, "text_width")?,
        text_position: changed(&s.text_position, &legacy.text_position),
        text_gap: length(s.text_gap, legacy.text_gap, "text_gap")?,
        text_along: manual_point(d, |p| p.x, "text_along")?,
        text_across: manual_point(d, |p| p.y, "text_across")?,
        extension_offset: length(
            s.extension_offset,
            legacy.extension_offset,
            "extension_offset",
        )?,
        extension_overshoot: length(
            s.extension_overshoot,
            legacy.extension_overshoot,
            "extension_overshoot",
        )?,
        extension_width: length(s.extension_width, legacy.extension_width, "extension_width")?,
        unit: changed(&s.unit, &legacy.unit),
        show_unit: changed(&s.show_unit, &legacy.show_unit),
        prefix: changed(&s.prefix, &legacy.prefix),
        suffix: changed(&s.suffix, &legacy.suffix),
        trailing_zeros: changed(&s.trailing_zeros, &legacy.trailing_zeros),
    })
}
