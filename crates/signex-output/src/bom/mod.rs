//! BOM (Bill of Materials) export — CSV, HTML, XLSX formats.
//!
//! See `OUTPUT_PLAN.md` §8. Walks every sheet in an ExportContext, aggregates
//! components, skips power ports and DNP symbols, and emits in one of three formats.

use std::collections::BTreeMap;
use std::path::Path;

use signex_bom::{BomComponent, BomContext, BomEngineOptions, build_table, validate_table};
use thiserror::Error;

use crate::{ExportContext, Exporter};

pub(crate) mod csv;
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

/// Roll up and validate in one pass — the same table + report
/// [`BomExporter::export`] produces, without emitting any bytes. The BOM
/// preview calls this so the Rules tab can never disagree with the export.
pub fn rollup_with_validation(
    ctx: &ExportContext,
    opts: &BomOptions,
) -> (BomTable, BomValidationReport) {
    let bom_ctx = build_bom_context(ctx, opts);
    let table = build_table(&bom_ctx, &engine_options_from_opts(opts));
    let report = validate_table(
        &build_validation_context(&bom_ctx, opts),
        &table,
        &opts.rule_options,
    );
    (table, report)
}

/// Re-validate an already rolled-up `table` (e.g. after a rule toggle,
/// which does not change the table). Uses the same exported-rows filter as
/// [`BomExporter::export`].
pub fn validate(ctx: &ExportContext, opts: &BomOptions, table: &BomTable) -> BomValidationReport {
    let bom_ctx = build_bom_context(ctx, opts);
    validate_table(
        &build_validation_context(&bom_ctx, opts),
        table,
        &opts.rule_options,
    )
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

            let variant_fitted = resolve_active_variant_fitted(sym, opts.active_variant.as_deref());
            let base_fitted = resolve_base_variant_fitted(sym);

            let mut custom = sym
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>();
            custom.remove("Name");
            custom.remove("Description");
            custom.remove("LibRef");
            // Fit-state fields feed `variant_fitted`/`base_fitted`, not the
            // part's identity. Drop whatever the resolvers recognise so a
            // variant override cannot split otherwise identical parts.
            custom.retain(|key, _| !is_fit_state_field(key));

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
                base_fitted,
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
    component.passes_dnp_filter(opts.include_dnp)
        && (opts.include_not_fitted || component.is_fitted())
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

/// True for any field key the fit-state resolvers read, so the BOM identity
/// strips exactly what they consume and the two cannot drift.
fn is_fit_state_field(key: &str) -> bool {
    parse_variant_field_key(key).is_some() || property_variant_kind(key).is_some()
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

/// Rank a parsed fit-state field so Fitted always outranks DNP on a tie.
///
/// Shared by every path that must arbitrate more than one candidate field for
/// the same symbol, so a Fitted-vs-DNP conflict resolves the same way
/// regardless of which path resolved it — encoding the precedence twice is
/// exactly how the base `custom_properties` path and the variant `fields`
/// path drifted apart.
fn rank_fitted_over_dnp(kind: VariantFieldKind, parsed: bool) -> (u8, bool) {
    match kind {
        VariantFieldKind::Fitted => (0, parsed),
        VariantFieldKind::Dnp => (1, !parsed),
    }
}

/// Case-insensitive field lookup, deterministic on a case collision.
///
/// `fields` is a `HashMap`, so naively taking the first case-insensitive
/// match via `.iter().find()` depends on hash-iteration order — a symbol
/// carrying both `Fitted` and `fitted` with contradictory values could
/// resolve either way, run to run. Prefer the exact-case match when one
/// exists (the unambiguous, intended lookup); otherwise fall back to the
/// case-insensitive match with the lexicographically smallest key, which is
/// a pure function of the map's contents rather than of the allocator's
/// `RandomState` seed.
fn field_value_ci<'a>(
    fields: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Option<&'a str> {
    if let Some(exact) = fields.get(key) {
        return Some(exact.as_str());
    }
    fields
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .min_by_key(|(candidate, _)| candidate.as_str())
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

    // `custom_properties` is a `Vec`, so its order is already deterministic
    // per file — but a first-match return still lets a symbol that lists DNP
    // before Fitted resolve as DNP here and as Fitted via `field_value_ci`
    // (which checks "Fitted" ahead of "DNP" explicitly). Rank every
    // candidate with the same Fitted-over-DNP precedence the variant path
    // uses so the two paths never disagree on the same symbol.
    symbol
        .custom_properties
        .iter()
        .filter_map(|property| {
            let kind = property_variant_kind(&property.key)?;
            let parsed = parse_bool_field(&property.value)?;
            Some(rank_fitted_over_dnp(kind, parsed))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, fitted)| fitted)
}

/// Fit state for `active_variant` taken from `symbol.fields`.
///
/// `fields` is a `HashMap`, so a symbol carrying contradictory entries
/// (`Fitted@LITE=yes` alongside `DNP@LITE=yes`) would resolve by hash order and
/// export a different fit state from one run to the next. Resolve with the same
/// precedence `resolve_base_variant_fitted` applies — Fitted ahead of DNP — and
/// break any remaining tie on the field key, so the answer is a pure function
/// of the file rather than of the allocator.
fn resolve_variant_fitted_from_fields(
    symbol: &signex_types::schematic::Symbol,
    active_variant: &str,
) -> Option<bool> {
    symbol
        .fields
        .iter()
        .filter_map(|(key, value)| {
            let (variant_name, kind) = parse_variant_field_key(key)?;
            if !variant_name.eq_ignore_ascii_case(active_variant) {
                return None;
            }
            let parsed = parse_bool_field(value)?;
            let (rank, fitted) = rank_fitted_over_dnp(kind, parsed);
            Some(((rank, key.as_str()), fitted))
        })
        .min_by_key(|(precedence, _)| *precedence)
        .map(|(_, fitted)| fitted)
}

/// Fit state from the active variant's own override for `symbol`, if any.
/// Base fields are resolved separately by `resolve_base_variant_fitted`.
fn resolve_active_variant_fitted(
    symbol: &signex_types::schematic::Symbol,
    active_variant: Option<&str>,
) -> Option<bool> {
    let active_variant = active_variant?.trim();
    if active_variant.is_empty() {
        return None;
    }
    resolve_variant_fitted_from_property_overrides(symbol, active_variant)
        .or_else(|| resolve_variant_fitted_from_fields(symbol, active_variant))
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
        let (table, validation_report) = rollup_with_validation(ctx, opts);

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
    use crate::{ProjectMetadata, SheetSnapshot};
    use signex_types::property::SchematicProperty;
    use signex_types::schematic::{SchematicSheet, Symbol};
    use uuid::Uuid;

    /// The fit override `BomComponent::is_fitted` applies: the active
    /// variant's, else the base fields'.
    fn resolve_variant_fitted(symbol: &Symbol, active_variant: Option<&str>) -> Option<bool> {
        resolve_active_variant_fitted(symbol, active_variant)
            .or_else(|| resolve_base_variant_fitted(symbol))
    }

    fn test_symbol() -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            footprint: "R_0603".to_string(),
            ..Default::default()
        }
    }

    fn base_dnp_variant_context(property_override: bool) -> ExportContext {
        let mut symbol = test_symbol();
        symbol.reference = "R5".to_string();
        symbol.dnp = true;
        symbol.footprint.clear();
        if property_override {
            symbol.custom_properties.push(SchematicProperty {
                key: "VariantFitted".to_string(),
                value: "0".to_string(),
                variant_overrides: BTreeMap::from([("ProA".to_string(), "1".to_string())]),
                ..Default::default()
            });
        } else {
            symbol
                .fields
                .insert("Fitted@ProA".to_string(), "yes".to_string());
        }
        ExportContext {
            sheets: vec![SheetSnapshot {
                path: "variant.snxsch".into(),
                schematic: SchematicSheet {
                    paper_size: "A4".to_string(),
                    symbols: vec![symbol],
                    ..Default::default()
                },
                sheet_name: "Main".to_string(),
                sheet_number: 1,
                sheet_count: 1,
            }],
            metadata: ProjectMetadata::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
        }
    }

    #[test]
    fn variant_fitted_base_dnp_is_included_in_rollup_and_csv() -> Result<(), BomError> {
        for property_override in [false, true] {
            let ctx = base_dnp_variant_context(property_override);
            let opts = BomOptions {
                active_variant: Some("ProA".to_string()),
                columns: vec![BomColumn::Designator, BomColumn::Qty],
                ..BomOptions::default()
            };
            let table = rollup(&ctx, &opts);
            assert_eq!(table.rows.len(), 1);
            assert_eq!(table.rows[0].references, vec!["R5"]);
            assert_eq!(table.rows[0].qty, 1);
            assert_eq!(table.rows[0].fitted_qty, 1);
            assert_eq!(table.rows[0].not_fitted_qty, 0);
            let output = BomExporter.export(&ctx, &opts)?;
            assert_eq!(output.bytes, b"\xef\xbb\xbfDesignator,Qty\r\nR5,1\r\n");

            for active_variant in [None, Some("Other".to_string())] {
                let opts = BomOptions {
                    active_variant,
                    ..opts.clone()
                };
                assert!(rollup(&ctx, &opts).rows.is_empty());
                let output = BomExporter.export(&ctx, &opts)?;
                assert_eq!(output.bytes, b"\xef\xbb\xbfDesignator,Qty\r\n");
                assert!(output.validation_report.issues.is_empty());
            }
        }
        Ok(())
    }

    #[test]
    fn variant_fitted_base_dnp_receives_export_validation() -> Result<(), BomError> {
        for property_override in [false, true] {
            let ctx = base_dnp_variant_context(property_override);
            let opts = BomOptions {
                active_variant: Some("ProA".to_string()),
                ..BomOptions::default()
            };
            let output = BomExporter.export(&ctx, &opts)?;
            let issues = output.validation_report.issues;
            assert_eq!(issues.len(), 2);
            for rule in [BomRule::MissingFootprint, BomRule::MissingMpn] {
                assert!(
                    issues
                        .iter()
                        .any(|issue| { issue.rule == rule && issue.references == vec!["R5"] })
                );
            }
        }
        Ok(())
    }

    /// Fit state read from the symbol's own base fields must not override its
    /// DNP flag; only the active variant's override may (#674 review).
    #[test]
    fn base_fit_fields_do_not_bypass_dnp() -> Result<(), BomError> {
        let base_fit_fields = [
            ("Fitted", "yes", false),
            ("VariantFitted", "1", false),
            ("DNP", "No", false),
            ("VariantFitted", "1", true),
        ];
        for (key, value, as_property) in base_fit_fields {
            let mut ctx = base_dnp_variant_context(false);
            let symbol = &mut ctx.sheets[0].schematic.symbols[0];
            symbol.fields.remove("Fitted@ProA");
            if as_property {
                symbol.custom_properties.push(SchematicProperty {
                    key: key.to_string(),
                    value: value.to_string(),
                    ..Default::default()
                });
            } else {
                symbol.fields.insert(key.to_string(), value.to_string());
            }

            for active_variant in [None, Some("ProA".to_string())] {
                let opts = BomOptions {
                    active_variant,
                    ..BomOptions::default()
                };
                let case = format!("{key}={value} (property: {as_property}), {opts:?}");
                assert!(rollup(&ctx, &opts).rows.is_empty(), "{case}");
                let output = BomExporter.export(&ctx, &opts)?;
                assert!(output.validation_report.issues.is_empty(), "{case}");

                // Including DNP parts still counts the base fit state, as before.
                let opts = BomOptions {
                    include_dnp: true,
                    ..opts
                };
                let table = rollup(&ctx, &opts);
                assert_eq!(table.rows.len(), 1, "{case}");
                assert_eq!(table.rows[0].fitted_qty, 1, "{case}");
            }
        }
        Ok(())
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
    fn variant_fitted_falls_back_to_base_when_variant_value_unparseable() {
        let mut symbol = test_symbol();
        symbol
            .fields
            .insert("Fitted@LITE".to_string(), "maybe".to_string());
        symbol
            .fields
            .insert("Fitted".to_string(), "yes".to_string());

        assert_eq!(resolve_variant_fitted(&symbol, Some("LITE")), Some(true));

        // A parseable sibling variant field must still be reached after the
        // unparseable one is skipped.
        symbol
            .fields
            .insert("DNP@LITE".to_string(), "no".to_string());
        assert_eq!(resolve_variant_fitted(&symbol, Some("LITE")), Some(true));
    }

    /// `symbol.fields` is a `HashMap`, so a symbol carrying contradictory
    /// variant entries used to resolve by whichever key the allocator handed
    /// over first — the same file could export a different fit state run to
    /// run. Fitted wins over DNP, deterministically. Rebuilt per iteration
    /// because `RandomState` re-seeds per map, not per process.
    #[test]
    fn contradictory_variant_fields_resolve_deterministically() {
        for _ in 0..64 {
            let mut symbol = test_symbol();
            symbol
                .fields
                .insert("Fitted@LITE".to_string(), "no".to_string());
            symbol
                .fields
                .insert("DNP@LITE".to_string(), "no".to_string());
            // Fitted@LITE=no → not fitted; DNP@LITE=no → fitted. Fitted wins.
            assert_eq!(resolve_variant_fitted(&symbol, Some("LITE")), Some(false));
        }
    }

    /// A symbol carrying both `Fitted` and `fitted` used to resolve by
    /// whichever key the `HashMap` handed back first. The exact-case match
    /// must win, deterministically. Rebuilt per iteration because
    /// `RandomState` re-seeds per map, not per process.
    #[test]
    fn field_value_ci_prefers_exact_case_on_collision() {
        for _ in 0..64 {
            let mut fields = std::collections::HashMap::new();
            fields.insert("Fitted".to_string(), "yes".to_string());
            fields.insert("fitted".to_string(), "no".to_string());
            assert_eq!(field_value_ci(&fields, "Fitted"), Some("yes"));
        }
    }

    /// With no exact-case match, the fallback must still be a pure function
    /// of the map's contents (lexicographically smallest key), not of hash
    /// order.
    #[test]
    fn field_value_ci_falls_back_to_smallest_key_without_exact_match() {
        for _ in 0..64 {
            let mut fields = std::collections::HashMap::new();
            fields.insert("FITTED".to_string(), "no".to_string());
            fields.insert("fitted".to_string(), "yes".to_string());
            assert_eq!(field_value_ci(&fields, "Fitted"), Some("no"));
        }
    }

    /// `resolve_base_variant_fitted`'s `custom_properties` loop used to return
    /// on the first parseable match with no ranking, so a symbol listing DNP
    /// before Fitted resolved as DNP — disagreeing with the variant path,
    /// which always ranks Fitted ahead of DNP. Both orderings must now agree.
    #[test]
    fn base_custom_properties_prefers_fitted_over_dnp_regardless_of_order() {
        let mut dnp_first = test_symbol();
        dnp_first.custom_properties.push(SchematicProperty {
            key: "DNP".to_string(),
            value: "no".to_string(), // DNP=no -> fitted
            ..Default::default()
        });
        dnp_first.custom_properties.push(SchematicProperty {
            key: "Fitted".to_string(),
            value: "no".to_string(), // Fitted=no -> not fitted
            ..Default::default()
        });
        assert_eq!(resolve_variant_fitted(&dnp_first, None), Some(false));

        let mut fitted_first = test_symbol();
        fitted_first.custom_properties.push(SchematicProperty {
            key: "Fitted".to_string(),
            value: "no".to_string(),
            ..Default::default()
        });
        fitted_first.custom_properties.push(SchematicProperty {
            key: "DNP".to_string(),
            value: "no".to_string(),
            ..Default::default()
        });
        assert_eq!(resolve_variant_fitted(&fitted_first, None), Some(false));
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

    // ---- validation surface (#355) ----

    fn export_ctx(symbols: Vec<Symbol>) -> ExportContext {
        use crate::{ProjectMetadata, SheetSnapshot};
        use signex_types::schematic::SchematicSheet;
        let schematic = SchematicSheet {
            uuid: Uuid::new_v4(),
            paper_size: "A4".to_string(),
            symbols,
            ..Default::default()
        };
        ExportContext {
            sheets: vec![SheetSnapshot {
                path: "a.snxsch".into(),
                schematic,
                sheet_name: "a".to_string(),
                sheet_number: 1,
                sheet_count: 1,
            }],
            metadata: ProjectMetadata::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
        }
    }

    /// R1 without a footprint + a second R1 (duplicate designator). MPN
    /// rule off so exactly the two rules under test can fire.
    fn faulty_fixture() -> (ExportContext, BomOptions) {
        let mut no_fp = test_symbol();
        no_fp.footprint = String::new();
        let dup = test_symbol();
        let mut opts = BomOptions::default();
        opts.rule_options.missing_mpn = false;
        (export_ctx(vec![no_fp, dup]), opts)
    }

    #[test]
    fn missing_footprint_plus_duplicate_designator_is_exactly_two_issues() {
        let (ctx, opts) = faulty_fixture();
        let (_table, report) = rollup_with_validation(&ctx, &opts);
        assert_eq!(report.issues.len(), 2, "{:?}", report.issues);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.rule == BomRule::MissingFootprint)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.rule == BomRule::DuplicateDesignator)
        );
    }

    #[test]
    fn disabling_duplicate_designator_drops_it_to_one_issue() {
        let (ctx, mut opts) = faulty_fixture();
        let (table, _) = rollup_with_validation(&ctx, &opts);
        opts.rule_options
            .set_enabled(BomRule::DuplicateDesignator, false);
        let report = validate(&ctx, &opts, &table);
        assert_eq!(report.issues.len(), 1, "{:?}", report.issues);
        assert_eq!(report.issues[0].rule, BomRule::MissingFootprint);
    }

    #[test]
    fn exporter_report_matches_the_preview_report() {
        let (ctx, opts) = faulty_fixture();
        let out = BomExporter.export(&ctx, &opts).expect("export");
        let (_t, preview) = rollup_with_validation(&ctx, &opts);
        assert_eq!(out.validation_report.issues, preview.issues);
    }

    #[test]
    fn issues_respect_include_dnp() {
        let mut dnp = test_symbol();
        dnp.reference = "R9".to_string();
        dnp.footprint = String::new();
        dnp.dnp = true;
        let ctx = export_ctx(vec![dnp]);
        let mut opts = BomOptions::default();
        opts.rule_options.missing_mpn = false;
        let (_t, report) = rollup_with_validation(&ctx, &opts);
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        opts.include_dnp = true;
        opts.include_not_fitted = true;
        let (_t, report) = rollup_with_validation(&ctx, &opts);
        assert_eq!(report.issues.len(), 1, "{:?}", report.issues);
    }

    // ---- grouping identity (#673) ----

    fn part_with_fields(reference: &str, fields: &[(&str, &str)]) -> Symbol {
        let mut symbol = test_symbol();
        symbol.reference = reference.to_string();
        for (key, value) in fields {
            symbol
                .fields
                .insert((*key).to_string(), (*value).to_string());
        }
        symbol
    }

    #[test]
    fn identical_parts_with_mpn_and_manufacturer_merge() {
        let part = [("MPN", "RC0603FR-0710KL"), ("Manufacturer", "Yageo")];
        let ctx = export_ctx(vec![
            part_with_fields("R1", &part),
            part_with_fields("R2", &part),
        ]);
        let table = rollup(&ctx, &BomOptions::default());
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].references, vec!["R1", "R2"]);
    }

    #[test]
    fn variant_fit_fields_do_not_split_identical_parts() {
        let variant_keys = [
            "DNP",
            "fitted",
            "Fitted@Lite",
            "dnp@Lite",
            "Lite.DNP",
            "lite.fitted",
            "Variant:Lite:Fitted",
            "VARIANT:Lite:dnp",
        ];
        for key in variant_keys {
            let ctx = export_ctx(vec![
                part_with_fields("R1", &[("MPN", "RC0603FR-0710KL")]),
                part_with_fields("R2", &[("MPN", "RC0603FR-0710KL"), (key, "no")]),
            ]);
            let opts = BomOptions {
                include_not_fitted: true,
                include_dnp: true,
                ..BomOptions::default()
            };
            let table = rollup(&ctx, &opts);
            assert_eq!(table.rows.len(), 1, "{key} split identical parts");
            assert_eq!(table.rows[0].references, vec!["R1", "R2"], "{key}");
            assert!(
                table.rows[0].custom.keys().all(|k| k == "MPN"),
                "{key} leaked into custom: {:?}",
                table.rows[0].custom
            );
        }
    }

    #[test]
    fn part_level_fields_still_split_parts() {
        let ctx = export_ctx(vec![
            part_with_fields("R1", &[("Tolerance", "1%")]),
            part_with_fields("R2", &[("Tolerance", "5%")]),
        ]);
        assert_eq!(rollup(&ctx, &BomOptions::default()).rows.len(), 2);
    }
}
