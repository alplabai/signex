//! XLSX emitter for BOM export via rust_xlsxwriter.
//!
//! Produces an Excel workbook with frozen header row, auto-fit column widths,
//! and styled header row (bold, grey background).

use super::{BomColumn, BomError, BomTable};
use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook};

/// Column width presets (in character widths, approximate).
const COL_WIDTH_NAME: f64 = 20.0;
const COL_WIDTH_REF: f64 = 8.0;
const COL_WIDTH_LIBREF: f64 = 18.0;
const COL_WIDTH_QTY: f64 = 4.0;
const COL_WIDTH_VALUE: f64 = 15.0;
const COL_WIDTH_FOOTPRINT: f64 = 30.0;
const COL_WIDTH_DESC: f64 = 40.0;
const COL_WIDTH_CUSTOM: f64 = 20.0;
const COL_WIDTH_NUMBER: f64 = 10.0;

/// Emit a BOM table as XLSX with styled header row and auto-fit widths.
pub fn emit(table: &BomTable, columns: &[BomColumn]) -> Result<Vec<u8>, BomError> {
    let mut workbook = Workbook::new();

    let worksheet = workbook.add_worksheet();

    // Create header format: bold, light grey background
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xF0F0F0))
        .set_border(FormatBorder::Thin);

    // Write header row
    for (col, column) in columns.iter().enumerate() {
        let col = col as u16;
        let header_text = column.header();
        let width = match column {
            BomColumn::Name => COL_WIDTH_NAME,
            BomColumn::Designator | BomColumn::Reference => COL_WIDTH_REF,
            BomColumn::LibRef => COL_WIDTH_LIBREF,
            BomColumn::Qty => COL_WIDTH_QTY,
            BomColumn::Value => COL_WIDTH_VALUE,
            BomColumn::Footprint => COL_WIDTH_FOOTPRINT,
            BomColumn::Description => COL_WIDTH_DESC,
            BomColumn::Custom(_) => COL_WIDTH_CUSTOM,
            BomColumn::UnitPrice
            | BomColumn::BuyQty
            | BomColumn::ExtendedPrice
            | BomColumn::Stock => COL_WIDTH_NUMBER,
            BomColumn::Currency => COL_WIDTH_QTY + 4.0,
            BomColumn::Distributor | BomColumn::PriceAge => COL_WIDTH_VALUE,
        };

        worksheet.write_string_with_format(0, col, header_text, &header_format)?;
        worksheet.set_column_width(col, width)?;
    }

    // Freeze the header row
    worksheet.set_freeze_panes(1, 0)?;

    // Write data rows
    for (row_idx, bom_row) in table.rows.iter().enumerate() {
        let row = (row_idx + 1) as u32;

        for (col_idx, column) in columns.iter().enumerate() {
            let col = col_idx as u16;
            // Quantities, prices and stock stay numbers, so a spreadsheet can
            // sum and sort them.
            match super::cell_number(bom_row, column) {
                Some(number) => worksheet.write_number(row, col, number)?,
                None => worksheet.write_string(row, col, super::cell_text(bom_row, column))?,
            };
        }
    }

    // One total line per currency under a priced BOM (#1023).
    let total_format = Format::new().set_bold();
    for (line_idx, line) in super::total_lines(table, columns).into_iter().enumerate() {
        // After one empty row, so the totals are not read as a part.
        let row = (table.rows.len() + 2 + line_idx) as u32;
        for (col_idx, (text, number)) in line.into_iter().enumerate() {
            let col = col_idx as u16;
            match number {
                Some(number) => {
                    worksheet.write_number_with_format(row, col, number, &total_format)?
                }
                None => worksheet.write_string_with_format(row, col, text, &total_format)?,
            };
        }
    }

    // Save to in-memory buffer
    let bytes = workbook
        .save_to_buffer()
        .map_err(|e| BomError::Xlsx(format!("Failed to save workbook: {}", e)))?;

    Ok(bytes)
}
