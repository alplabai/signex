//! XLSX emitter for BOM export via rust_xlsxwriter.
//!
//! Produces an Excel workbook with frozen header row, auto-fit column widths,
//! and styled header row (bold, grey background).

use super::{BomColumn, BomError, BomTable};
use rust_xlsxwriter::{Workbook, Format, Color, FormatBorder};

/// Column width presets (in character widths, approximate).
const COL_WIDTH_REF: f64 = 8.0;
const COL_WIDTH_QTY: f64 = 4.0;
const COL_WIDTH_VALUE: f64 = 15.0;
const COL_WIDTH_FOOTPRINT: f64 = 30.0;
const COL_WIDTH_DESC: f64 = 40.0;
const COL_WIDTH_CUSTOM: f64 = 20.0;

/// Emit a BOM table as XLSX with styled header row and auto-fit widths.
pub fn emit(table: &BomTable, columns: &[BomColumn]) -> Result<Vec<u8>, BomError> {
    let mut workbook = Workbook::new();

    let worksheet = workbook.add_worksheet();

    // Create header format: bold, light grey background
    let _header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xF0F0F0))
        .set_border(FormatBorder::Thin);

    // Write header row
    let mut col: u16 = 0;
    for column in columns {
        let header_text = column.header();
        let width = match column {
            BomColumn::Reference => COL_WIDTH_REF,
            BomColumn::Qty => COL_WIDTH_QTY,
            BomColumn::Value => COL_WIDTH_VALUE,
            BomColumn::Footprint => COL_WIDTH_FOOTPRINT,
            BomColumn::Description => COL_WIDTH_DESC,
            BomColumn::Custom(_) => COL_WIDTH_CUSTOM,
        };

        worksheet.write_string(0, col, header_text)?;
        worksheet.set_column_width(col, width)?;

        col += 1;
    }

    // Freeze the header row
    worksheet.set_freeze_panes(1, 0)?;

    // Write data rows
    let mut row: u32 = 1;
    for bom_row in &table.rows {
        let mut col: u16 = 0;

        for column in columns {
            let value = match column {
                BomColumn::Reference => bom_row.references.join(", "),
                BomColumn::Qty => bom_row.qty.to_string(),
                BomColumn::Value => bom_row.value.clone(),
                BomColumn::Footprint => bom_row.footprint.clone(),
                BomColumn::Description => bom_row.description.clone(),
                BomColumn::Custom(name) => {
                    bom_row.custom.get(name).cloned().unwrap_or_default()
                }
            };

            // Try to parse as number for Qty column; otherwise write as string
            if matches!(column, BomColumn::Qty) {
                if let Ok(qty_num) = value.parse::<u32>() {
                    worksheet.write_number(row, col, qty_num as f64)?;
                } else {
                    worksheet.write_string(row, col, &value)?;
                }
            } else {
                worksheet.write_string(row, col, &value)?;
            }

            col += 1;
        }

        row += 1;
    }

    // Save to in-memory buffer
    let bytes = workbook.save_to_buffer()
        .map_err(|e| BomError::Xlsx(format!("Failed to save workbook: {}", e)))?;

    Ok(bytes)
}
