//! BOM (Bill of Materials) export — CSV, HTML, XLSX formats.
//!
//! See `OUTPUT_PLAN.md` §8. Walks every sheet in an ExportContext, aggregates
//! components, skips power ports and DNP symbols, and emits in one of three formats.

use std::collections::BTreeMap;
use std::path::Path;

use signex_bom::{BomComponent, BomContext, BomEngineOptions, build_table, validate_table};
use thiserror::Error;

use crate::{ExportContext, Exporter};

mod csv;
mod html;
mod xlsx;

pub use csv::emit as csv_emit;
pub use html::emit as html_emit;
pub use signex_bom::{
    BomGrouping, BomIssueSeverity, BomMetadata, BomRow, BomRule, BomRuleOptions, BomTable,
    BomValidationIssue, BomValidationReport,
};
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
    pub include_not_fitted: bool,
    pub active_variant: Option<String>,
    pub rule_options: BomRuleOptions,
}

impl Default for BomOptions {
    fn default() -> Self {
        Self {
            columns: vec![
                BomColumn::Name,
                BomColumn::Description,
                BomColumn::Designator,
                BomColumn::Footprint,
                BomColumn::LibRef,
                BomColumn::Qty,
            ],
            grouping: BomGrouping::Grouped,
            format: BomFormat::Csv,
            include_dnp: false,
            include_not_fitted: false,
            active_variant: None,
            rule_options: BomRuleOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariantFieldKind {
    Fitted,
    Dnp,
}

/// Column identifiers for the BOM.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BomColumn {
    Name,
    Designator,
    LibRef,
    /// Legacy alias kept for compatibility.
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
            Self::Name => "Name",
            Self::Designator => "Designator",
            Self::LibRef => "LibRef",
            Self::Reference => "Reference",
            Self::Qty => "Qty",
            Self::Value => "Value",
            Self::Footprint => "Footprint",
            Self::Description => "Description",
            Self::Custom(name) => name,
        }
    }
}

/// Output format for the BOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomFormat {
    Csv,
    Html,
    Xlsx,
}

impl BomFormat {
    /// Resolve BOM format from output file extension.
    /// Falls back to CSV when extension is missing or unknown.
    pub fn from_output_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("xlsx") => Self::Xlsx,
            Some("html") | Some("htm") => Self::Html,
            _ => Self::Csv,
        }
    }
}

/// The output of a BOM export.
#[derive(Debug, Clone)]
pub struct BomOutput {
    pub bytes: Vec<u8>,
    pub validation_report: BomValidationReport,
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
    let bom_ctx = build_bom_context(ctx, opts);
    let engine_opts = engine_options_from_opts(opts);

    build_table(&bom_ctx, &engine_opts)
}

fn engine_options_from_opts(opts: &BomOptions) -> BomEngineOptions {
    BomEngineOptions {
        grouping: opts.grouping,
        include_dnp: opts.include_dnp,
        include_not_fitted: opts.include_not_fitted,
        // Keep v0.8 behavior stable for now: include symbols regardless of these flags.
        include_excluded_from_bom: true,
        include_not_on_board: true,
    }
}

fn build_bom_context(ctx: &ExportContext, opts: &BomOptions) -> BomContext {
    let mut components = Vec::new();

    for sheet_snap in &ctx.sheets {
        for sym in &sheet_snap.schematic.symbols {
            if sym.reference.starts_with("#PWR") || sym.is_power {
                continue;
            }

            let name = sym
                .fields
                .get("Name")
                .cloned()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| sym.value.clone());
            let description = sym.fields.get("Description").cloned().unwrap_or_default();
            let lib_ref = sym
                .fields
                .get("LibRef")
                .or_else(|| sym.fields.get("Part Number"))
                .cloned()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| sym.lib_id.clone());

            let variant_fitted = resolve_variant_fitted(sym, opts.active_variant.as_deref());

            let mut custom = sym
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>();
            custom.remove("Name");
            custom.remove("Description");
            custom.remove("LibRef");
            custom.remove("VariantFitted");
            custom.remove("Fitted");

            components.push(BomComponent {
                reference: sym.reference.clone(),
                name,
                value: sym.value.clone(),
                footprint: sym.footprint.clone(),
                lib_ref,
                description,
                dnp: sym.dnp,
                in_bom: sym.in_bom,
                on_board: sym.on_board,
                variant_fitted,
                custom,
            });
        }
    }

    BomContext {
        components,
        metadata: BomMetadata {
            title: ctx.metadata.title.clone(),
            revision: ctx.metadata.revision.clone(),
            date: ctx.metadata.date.clone(),
        },
    }
}

fn component_is_exported(component: &BomComponent, opts: &BomOptions) -> bool {
    (opts.include_dnp || !component.dnp) && (opts.include_not_fitted || component.is_fitted())
}

fn build_validation_context(ctx: &BomContext, opts: &BomOptions) -> BomContext {
    BomContext {
        components: ctx
            .components
            .iter()
            .filter(|component| component_is_exported(component, opts))
            .cloned()
            .collect(),
        metadata: ctx.metadata.clone(),
    }
}

fn parse_bool_field(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" | "fitted" => Some(true),
        "0" | "false" | "no" | "n" | "off" | "not fitted" => Some(false),
        _ => None,
    }
}

fn parse_variant_field_key(key: &str) -> Option<(String, VariantFieldKind)> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }

    if let Some((lhs, rhs)) = key.split_once('@') {
        let kind = if lhs.eq_ignore_ascii_case("fitted") {
            VariantFieldKind::Fitted
        } else if lhs.eq_ignore_ascii_case("dnp") {
            VariantFieldKind::Dnp
        } else {
            return None;
        };
        let variant = rhs.trim();
        if !variant.is_empty() {
            return Some((variant.to_string(), kind));
        }
    }

    if let Some((variant, suffix)) = key.rsplit_once('.') {
        let kind = if suffix.eq_ignore_ascii_case("fitted") {
            VariantFieldKind::Fitted
        } else if suffix.eq_ignore_ascii_case("dnp") {
            VariantFieldKind::Dnp
        } else {
            return None;
        };
        let variant = variant.trim();
        if !variant.is_empty() {
            return Some((variant.to_string(), kind));
        }
    }

    let mut parts = key.split(':');
    let head = parts.next()?;
    let variant = parts.next()?;
    let tail = parts.next()?;
    if parts.next().is_some() || !head.eq_ignore_ascii_case("variant") {
        return None;
    }
    let kind = if tail.eq_ignore_ascii_case("fitted") {
        VariantFieldKind::Fitted
    } else if tail.eq_ignore_ascii_case("dnp") {
        VariantFieldKind::Dnp
    } else {
        return None;
    };
    let variant = variant.trim();
    if variant.is_empty() {
        return None;
    }
    Some((variant.to_string(), kind))
}

fn property_variant_kind(key: &str) -> Option<VariantFieldKind> {
    if key.eq_ignore_ascii_case("variantfitted") || key.eq_ignore_ascii_case("fitted") {
        return Some(VariantFieldKind::Fitted);
    }
    if key.eq_ignore_ascii_case("dnp") {
        return Some(VariantFieldKind::Dnp);
    }
    None
}

fn field_value_ci<'a>(
    fields: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Option<&'a str> {
    fields
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
}

fn resolve_variant_fitted_from_property_overrides(
    symbol: &signex_types::schematic::Symbol,
    active_variant: &str,
) -> Option<bool> {
    for property in &symbol.custom_properties {
        let Some(kind) = property_variant_kind(&property.key) else {
            continue;
        };
        for (variant_name, variant_value) in &property.variant_overrides {
            if variant_name.eq_ignore_ascii_case(active_variant) {
                let Some(parsed) = parse_bool_field(variant_value) else {
                    continue;
                };
                return Some(match kind {
                    VariantFieldKind::Fitted => parsed,
                    VariantFieldKind::Dnp => !parsed,
                });
            }
        }
    }
    None
}

fn resolve_base_variant_fitted(symbol: &signex_types::schematic::Symbol) -> Option<bool> {
    if let Some(raw) = field_value_ci(&symbol.fields, "VariantFitted") {
        return parse_bool_field(raw);
    }
    if let Some(raw) = field_value_ci(&symbol.fields, "Fitted") {
        return parse_bool_field(raw);
    }
    if let Some(raw) = field_value_ci(&symbol.fields, "DNP") {
        return parse_bool_field(raw).map(|value| !value);
    }

    for property in &symbol.custom_properties {
        let Some(kind) = property_variant_kind(&property.key) else {
            continue;
        };
        let Some(parsed) = parse_bool_field(&property.value) else {
            continue;
        };
        return Some(match kind {
            VariantFieldKind::Fitted => parsed,
            VariantFieldKind::Dnp => !parsed,
        });
    }

    None
}

fn resolve_variant_fitted(
    symbol: &signex_types::schematic::Symbol,
    active_variant: Option<&str>,
) -> Option<bool> {
    if let Some(active_variant) = active_variant {
        let active_variant = active_variant.trim();
        if !active_variant.is_empty() {
            if let Some(property_override) =
                resolve_variant_fitted_from_property_overrides(symbol, active_variant)
            {
                return Some(property_override);
            }
            for (key, value) in &symbol.fields {
                if let Some((variant_name, kind)) = parse_variant_field_key(key)
                    && variant_name.eq_ignore_ascii_case(active_variant)
                {
                    let parsed = parse_bool_field(value)?;
                    return Some(match kind {
                        VariantFieldKind::Fitted => parsed,
                        VariantFieldKind::Dnp => !parsed,
                    });
                }
            }
        }
    }

    resolve_base_variant_fitted(symbol)
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
        let bom_ctx = build_bom_context(ctx, opts);
        let table = build_table(&bom_ctx, &engine_options_from_opts(opts));
        let validation_ctx = build_validation_context(&bom_ctx, opts);
        let validation_report = validate_table(&validation_ctx, &table, &opts.rule_options);

        let bytes = match opts.format {
            BomFormat::Csv => csv_emit(&table, &opts.columns)?,
            BomFormat::Html => html_emit(&table, &opts.columns)?,
            BomFormat::Xlsx => xlsx_emit(&table, &opts.columns)?,
        };

        Ok(BomOutput {
            bytes,
            validation_report,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::property::SchematicProperty;
    use signex_types::schematic::{Point, Symbol};
    use uuid::Uuid;

    fn test_symbol() -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            footprint: "R_0603".to_string(),
            datasheet: String::new(),
            position: Point { x: 0.0, y: 0.0 },
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: Vec::new(),
            pin_uuids: std::collections::HashMap::new(),
            instances: Vec::new(),
        }
    }

    #[test]
    fn variant_fitted_prefers_property_override() {
        let mut symbol = test_symbol();
        symbol
            .fields
            .insert("Fitted".to_string(), "yes".to_string());

        let mut fitted_property = SchematicProperty {
            key: "Fitted".to_string(),
            value: "yes".to_string(),
            ..Default::default()
        };
        fitted_property
            .variant_overrides
            .insert("LITE".to_string(), "no".to_string());
        symbol.custom_properties.push(fitted_property);

        assert_eq!(resolve_variant_fitted(&symbol, Some("LITE")), Some(false));
        assert_eq!(resolve_variant_fitted(&symbol, Some("PRO")), Some(true));
        assert_eq!(resolve_variant_fitted(&symbol, None), Some(true));
    }

    #[test]
    fn csv_emits_rfc4180() -> Result<(), BomError> {
        let table = BomTable {
            rows: vec![BomRow {
                references: vec!["R1".to_string()],
                name: "10k".to_string(),
                qty: 1,
                fitted_qty: 1,
                not_fitted_qty: 0,
                value: "10k".to_string(),
                footprint: "R_0603".to_string(),
                lib_ref: "Device:R".to_string(),
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
                name: "10k".to_string(),
                qty: 1,
                fitted_qty: 1,
                not_fitted_qty: 0,
                value: "10k".to_string(),
                footprint: "R_0603".to_string(),
                lib_ref: "Device:R".to_string(),
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
                name: "10k".to_string(),
                qty: 1,
                fitted_qty: 1,
                not_fitted_qty: 0,
                value: "10k".to_string(),
                footprint: "R_0603".to_string(),
                lib_ref: "Device:R".to_string(),
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

    #[test]
    fn bom_format_resolves_from_output_path() {
        assert_eq!(
            BomFormat::from_output_path(Path::new("bom.csv")),
            BomFormat::Csv
        );
        assert_eq!(
            BomFormat::from_output_path(Path::new("bom.xlsx")),
            BomFormat::Xlsx
        );
        assert_eq!(
            BomFormat::from_output_path(Path::new("bom.html")),
            BomFormat::Html
        );
        assert_eq!(
            BomFormat::from_output_path(Path::new("bom.htm")),
            BomFormat::Html
        );
        assert_eq!(
            BomFormat::from_output_path(Path::new("bom.unknown")),
            BomFormat::Csv
        );
        assert_eq!(
            BomFormat::from_output_path(Path::new("bom")),
            BomFormat::Csv
        );
    }
}
