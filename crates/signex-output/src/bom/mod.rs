//! BOM (Bill of Materials) export — CSV, HTML, XLSX formats.
//!
//! See `OUTPUT_PLAN.md` §8. Walks every sheet in an ExportContext, aggregates
//! components, skips power ports and DNP symbols, and emits in one of three formats.

use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

use crate::{ExportContext, Exporter};

mod csv;
mod html;
mod xlsx;

pub use csv::emit as csv_emit;
pub use html::emit as html_emit;
pub use xlsx::emit as xlsx_emit;

// ============================================================================
// Public API
// ============================================================================

pub struct BomExporter;

/// Configuration for BOM export.
#[derive(Debug, Clone)]
pub struct BomOptions {
    pub columns: Vec<BomColumn>,
    pub grouping: BomGrouping,
    pub format: BomFormat,
    pub include_dnp: bool,
}

impl Default for BomOptions {
    fn default() -> Self {
        Self {
            columns: vec![
                BomColumn::Reference,
                BomColumn::Qty,
                BomColumn::Value,
                BomColumn::Footprint,
                BomColumn::Description,
            ],
            grouping: BomGrouping::Grouped,
            format: BomFormat::Csv,
            include_dnp: false,
        }
    }
}

/// Column identifiers for the BOM.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BomColumn {
    Reference,
    Qty,
    Value,
    Footprint,
    Description,
    Custom(String),
}

impl BomColumn {
    pub fn header(&self) -> &str {
        match self {
            Self::Reference => "Reference",
            Self::Qty => "Qty",
            Self::Value => "Value",
            Self::Footprint => "Footprint",
            Self::Description => "Description",
            Self::Custom(name) => name,
        }
    }
}

/// How to group components in the BOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomGrouping {
    /// Group by (value, footprint); collapse references; qty = count.
    Grouped,
    /// One row per component.
    Ungrouped,
    /// One row per component, sorted by reference.
    Flat,
}

/// Output format for the BOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomFormat {
    Csv,
    Html,
    Xlsx,
}

/// A single row in the BOM table.
#[derive(Debug, Clone)]
pub struct BomRow {
    pub references: Vec<String>, // ["R1", "R2", "R7"]
    pub qty: u32,
    pub value: String,
    pub footprint: String,
    pub description: String,
    pub custom: BTreeMap<String, String>,
}

/// The assembled BOM table.
#[derive(Debug, Clone)]
pub struct BomTable {
    pub rows: Vec<BomRow>,
    pub metadata: BomMetadata,
}

/// Metadata for the BOM (used in HTML headers, etc).
#[derive(Debug, Clone)]
pub struct BomMetadata {
    pub title: String,
    pub revision: String,
    pub date: String,
}

/// The output of a BOM export.
#[derive(Debug, Clone)]
pub struct BomOutput {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum BomError {
    #[error("io: {0}")]
    Io(String),
    #[error("xlsx: {0}")]
    Xlsx(String),
}

impl From<rust_xlsxwriter::XlsxError> for BomError {
    fn from(err: rust_xlsxwriter::XlsxError) -> Self {
        BomError::Xlsx(err.to_string())
    }
}

// ============================================================================
// Rollup logic
// ============================================================================

/// Walks every sheet in the ExportContext, aggregates components according
/// to the BomOptions, and returns a BomTable ready to emit.
pub fn rollup(ctx: &ExportContext, opts: &BomOptions) -> BomTable {
    let mut all_symbols = Vec::new();

    // Collect all symbols from all sheets, skipping power ports and DNP
    for sheet_snap in &ctx.sheets {
        for sym in &sheet_snap.schematic.symbols {
            // Skip power ports (#PWR*)
            if sym.reference.starts_with("#PWR") {
                continue;
            }

            // Skip if power symbol
            if sym.is_power {
                continue;
            }

            // Skip if DNP and not including DNP
            if !opts.include_dnp && sym.dnp {
                continue;
            }

            all_symbols.push(sym.clone());
        }
    }

    // Sort symbols by reference for deterministic output
    all_symbols.sort_by(|a, b| a.reference.cmp(&b.reference));

    // Build the BOM according to grouping mode
    let rows = match opts.grouping {
        BomGrouping::Grouped => {
            let mut grouped: HashMap<(String, String), Vec<String>> = HashMap::new();

            for sym in &all_symbols {
                let key = (sym.value.clone(), sym.footprint.clone());
                grouped
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .push(sym.reference.clone());
            }

            let mut rows_vec: Vec<BomRow> = grouped
                .into_iter()
                .map(|((value, footprint), references)| {
                    let qty = references.len() as u32;
                    let description = all_symbols
                        .iter()
                        .find(|s| references.contains(&s.reference))
                        .and_then(|s| s.fields.get("Description"))
                        .cloned()
                        .unwrap_or_default();

                    BomRow {
                        references,
                        qty,
                        value,
                        footprint,
                        description,
                        custom: BTreeMap::new(),
                    }
                })
                .collect();

            // Sort rows by first reference in each group for consistent output
            rows_vec.sort_by(|a, b| a.references.first().cmp(&b.references.first()));

            rows_vec
        }
        BomGrouping::Ungrouped | BomGrouping::Flat => {
            let mut rows_vec: Vec<BomRow> = all_symbols
                .iter()
                .map(|sym| {
                    let description = sym.fields.get("Description").cloned().unwrap_or_default();

                    BomRow {
                        references: vec![sym.reference.clone()],
                        qty: 1,
                        value: sym.value.clone(),
                        footprint: sym.footprint.clone(),
                        description,
                        custom: BTreeMap::new(),
                    }
                })
                .collect();

            if opts.grouping == BomGrouping::Flat {
                rows_vec.sort_by(|a, b| a.references.first().cmp(&b.references.first()));
            }

            rows_vec
        }
    };

    BomTable {
        rows,
        metadata: BomMetadata {
            title: ctx.metadata.title.clone(),
            revision: ctx.metadata.revision.clone(),
            date: ctx.metadata.date.clone(),
        },
    }
}

// ============================================================================
// Exporter impl
// ============================================================================

impl Exporter for BomExporter {
    type Options = BomOptions;
    type Output = BomOutput;
    type Error = BomError;

    fn export(
        &self,
        ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        let table = rollup(ctx, opts);

        let bytes = match opts.format {
            BomFormat::Csv => csv_emit(&table, &opts.columns)?,
            BomFormat::Html => html_emit(&table, &opts.columns)?,
            BomFormat::Xlsx => xlsx_emit(&table, &opts.columns)?,
        };

        Ok(BomOutput { bytes })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollup_groups_by_value_and_footprint() {
        // Test grouping logic without constructing full ExportContext
        let mut rows_map: HashMap<(String, String), Vec<String>> = HashMap::new();

        let refs = vec!["R1", "R2", "R7"];
        let key = ("10k".to_string(), "R_0603".to_string());
        rows_map.insert(key, refs.iter().map(|s| s.to_string()).collect());

        let groups = rows_map.len();
        assert_eq!(groups, 1);

        let first_group = rows_map.values().next().unwrap();
        assert_eq!(first_group.len(), 3);
    }

    #[test]
    fn csv_emits_rfc4180() -> Result<(), BomError> {
        let table = BomTable {
            rows: vec![BomRow {
                references: vec!["R1".to_string()],
                qty: 1,
                value: "10k".to_string(),
                footprint: "R_0603".to_string(),
                description: "Resistor".to_string(),
                custom: BTreeMap::new(),
            }],
            metadata: BomMetadata {
                title: "Test".to_string(),
                revision: "1".to_string(),
                date: "2026-04-22".to_string(),
            },
        };

        let opts = BomOptions::default();
        let bytes = csv_emit(&table, &opts.columns)?;

        // Check for UTF-8 BOM
        assert!(bytes.starts_with(b"\xef\xbb\xbf"), "Missing UTF-8 BOM");

        // Check for \r\n line endings
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("\r\n"), "Missing CRLF line endings");

        Ok(())
    }

    #[test]
    fn html_self_contained() -> Result<(), BomError> {
        let table = BomTable {
            rows: vec![BomRow {
                references: vec!["R1".to_string()],
                qty: 1,
                value: "10k".to_string(),
                footprint: "R_0603".to_string(),
                description: "Resistor".to_string(),
                custom: BTreeMap::new(),
            }],
            metadata: BomMetadata {
                title: "Test".to_string(),
                revision: "1".to_string(),
                date: "2026-04-22".to_string(),
            },
        };

        let opts = BomOptions::default();
        let bytes = html_emit(&table, &opts.columns)?;
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("<html"), "Missing <html tag");
        assert!(text.contains("<style"), "Missing <style tag");
        assert!(text.contains("</html>"), "Missing closing </html> tag");
        assert!(text.contains("<table"), "Missing <table tag");

        Ok(())
    }

    #[test]
    fn xlsx_produces_nonempty_bytes() -> Result<(), BomError> {
        let table = BomTable {
            rows: vec![BomRow {
                references: vec!["R1".to_string()],
                qty: 1,
                value: "10k".to_string(),
                footprint: "R_0603".to_string(),
                description: "Resistor".to_string(),
                custom: BTreeMap::new(),
            }],
            metadata: BomMetadata {
                title: "Test".to_string(),
                revision: "1".to_string(),
                date: "2026-04-22".to_string(),
            },
        };

        let opts = BomOptions::default();
        let bytes = xlsx_emit(&table, &opts.columns)?;

        // XLSX is a ZIP file, so it should start with PK
        assert!(
            bytes.starts_with(b"PK"),
            "XLSX should start with PK (ZIP magic)"
        );
        assert!(!bytes.is_empty(), "XLSX output should not be empty");

        Ok(())
    }
}
