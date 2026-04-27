use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

/// Input context for BOM generation.
#[derive(Debug, Clone)]
pub struct BomContext {
    pub components: Vec<BomComponent>,
    pub metadata: BomMetadata,
}

/// Normalized component candidate for BOM processing.
#[derive(Debug, Clone)]
pub struct BomComponent {
    pub reference: String,
    pub name: String,
    pub value: String,
    pub footprint: String,
    pub lib_ref: String,
    pub description: String,
    pub dnp: bool,
    pub in_bom: bool,
    pub on_board: bool,
    /// Variant override for fitted state.
    /// `None` means fallback to base schematic flags.
    pub variant_fitted: Option<bool>,
    pub custom: BTreeMap<String, String>,
}

impl BomComponent {
    pub fn is_fitted(&self) -> bool {
        self.variant_fitted
            .unwrap_or(!self.dnp && self.in_bom && self.on_board)
    }
}

/// Metadata propagated to BOM outputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BomMetadata {
    pub title: String,
    pub revision: String,
    pub date: String,
}

/// How to group components in the BOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BomGrouping {
    /// Group by value + footprint and collapse references.
    Grouped,
    /// One row per component in natural component order.
    Ungrouped,
    /// One row per component sorted by reference.
    Flat,
}

/// Engine options for BOM table generation.
#[derive(Debug, Clone)]
pub struct BomEngineOptions {
    pub grouping: BomGrouping,
    pub include_dnp: bool,
    pub include_not_fitted: bool,
    pub include_excluded_from_bom: bool,
    pub include_not_on_board: bool,
}

impl Default for BomEngineOptions {
    fn default() -> Self {
        Self {
            grouping: BomGrouping::Grouped,
            include_dnp: false,
            include_not_fitted: false,
            include_excluded_from_bom: false,
            include_not_on_board: false,
        }
    }
}

/// Supported BOM validation rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BomRule {
    MissingFootprint,
    MissingMpn,
    DuplicateDesignator,
    EmptyOrZeroQty,
}

/// Rule-level on/off switches for BOM validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomRuleOptions {
    pub missing_footprint: bool,
    pub missing_mpn: bool,
    pub duplicate_designator: bool,
    pub empty_or_zero_qty: bool,
}

impl Default for BomRuleOptions {
    fn default() -> Self {
        Self {
            missing_footprint: true,
            missing_mpn: true,
            duplicate_designator: true,
            empty_or_zero_qty: true,
        }
    }
}

impl BomRuleOptions {
    pub fn is_enabled(&self, rule: BomRule) -> bool {
        match rule {
            BomRule::MissingFootprint => self.missing_footprint,
            BomRule::MissingMpn => self.missing_mpn,
            BomRule::DuplicateDesignator => self.duplicate_designator,
            BomRule::EmptyOrZeroQty => self.empty_or_zero_qty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BomIssueSeverity {
    Error,
    Warning,
    Info,
}

/// Single validation finding produced by BOM rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BomValidationIssue {
    pub rule: BomRule,
    pub severity: BomIssueSeverity,
    pub message: String,
    pub references: Vec<String>,
}

/// Validation report emitted after BOM table generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BomValidationReport {
    pub issues: Vec<BomValidationIssue>,
}

impl BomValidationReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == BomIssueSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == BomIssueSeverity::Warning)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

/// A single row in the generated BOM table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BomRow {
    pub references: Vec<String>,
    pub name: String,
    pub qty: u32,
    pub fitted_qty: u32,
    pub not_fitted_qty: u32,
    pub value: String,
    pub footprint: String,
    pub lib_ref: String,
    pub description: String,
    pub custom: BTreeMap<String, String>,
}

/// Generated BOM table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BomTable {
    pub rows: Vec<BomRow>,
    pub metadata: BomMetadata,
}

/// Build a BOM table from a normalized BOM context.
pub fn build_table(ctx: &BomContext, opts: &BomEngineOptions) -> BomTable {
    let mut components = ctx
        .components
        .iter()
        .filter(|component| opts.include_dnp || !component.dnp)
        .filter(|component| opts.include_not_fitted || component.is_fitted())
        .filter(|component| opts.include_excluded_from_bom || component.in_bom)
        .filter(|component| opts.include_not_on_board || component.on_board)
        .cloned()
        .collect::<Vec<_>>();

    components.sort_by(|a, b| a.reference.cmp(&b.reference));

    let rows = match opts.grouping {
        BomGrouping::Grouped => {
            let mut grouped: HashMap<(String, String, String, String), Vec<&BomComponent>> =
                HashMap::new();

            for component in &components {
                grouped
                    .entry((
                        component.name.clone(),
                        component.value.clone(),
                        component.footprint.clone(),
                        component.lib_ref.clone(),
                    ))
                    .or_default()
                    .push(component);
            }

            let mut rows = grouped
                .into_iter()
                .map(|((name, value, footprint, lib_ref), grouped_components)| {
                    let references = grouped_components
                        .iter()
                        .map(|component| component.reference.clone())
                        .collect::<Vec<_>>();

                    let fitted_qty = grouped_components
                        .iter()
                        .filter(|component| component.is_fitted())
                        .count() as u32;
                    let qty = grouped_components.len() as u32;

                    let description = grouped_components
                        .first()
                        .map(|component| component.description.clone())
                        .unwrap_or_default();

                    let custom = grouped_components
                        .first()
                        .map(|component| component.custom.clone())
                        .unwrap_or_default();

                    BomRow {
                        references,
                        name,
                        qty,
                        fitted_qty,
                        not_fitted_qty: qty.saturating_sub(fitted_qty),
                        value,
                        footprint,
                        lib_ref,
                        description,
                        custom,
                    }
                })
                .collect::<Vec<_>>();

            rows.sort_by(|a, b| a.references.first().cmp(&b.references.first()));
            rows
        }
        BomGrouping::Ungrouped | BomGrouping::Flat => {
            let mut rows = components
                .iter()
                .map(|component| BomRow {
                    references: vec![component.reference.clone()],
                    name: component.name.clone(),
                    qty: 1,
                    fitted_qty: if component.is_fitted() { 1 } else { 0 },
                    not_fitted_qty: if component.is_fitted() { 0 } else { 1 },
                    value: component.value.clone(),
                    footprint: component.footprint.clone(),
                    lib_ref: component.lib_ref.clone(),
                    description: component.description.clone(),
                    custom: component.custom.clone(),
                })
                .collect::<Vec<_>>();

            if opts.grouping == BomGrouping::Flat {
                rows.sort_by(|a, b| a.references.first().cmp(&b.references.first()));
            }

            rows
        }
    };

    BomTable {
        rows,
        metadata: ctx.metadata.clone(),
    }
}

fn has_populated_field(component: &BomComponent, candidates: &[&str]) -> bool {
    component.custom.iter().any(|(key, value)| {
        candidates.iter().any(|candidate| key.eq_ignore_ascii_case(candidate))
            && !value.trim().is_empty()
    })
}

/// Run quality rules against BOM input/output and return a validation report.
pub fn validate_table(
    ctx: &BomContext,
    table: &BomTable,
    options: &BomRuleOptions,
) -> BomValidationReport {
    let mut issues = Vec::new();

    if options.is_enabled(BomRule::MissingFootprint) {
        for component in &ctx.components {
            if component.footprint.trim().is_empty() {
                issues.push(BomValidationIssue {
                    rule: BomRule::MissingFootprint,
                    severity: BomIssueSeverity::Warning,
                    message: format!(
                        "Component {} is missing footprint assignment",
                        component.reference
                    ),
                    references: vec![component.reference.clone()],
                });
            }
        }
    }

    if options.is_enabled(BomRule::MissingMpn) {
        const MPN_KEYS: &[&str] = &["MPN", "Manufacturer Part Number", "Part Number"];
        for component in &ctx.components {
            if !has_populated_field(component, MPN_KEYS) {
                issues.push(BomValidationIssue {
                    rule: BomRule::MissingMpn,
                    severity: BomIssueSeverity::Warning,
                    message: format!("Component {} is missing MPN", component.reference),
                    references: vec![component.reference.clone()],
                });
            }
        }
    }

    if options.is_enabled(BomRule::DuplicateDesignator) {
        let mut refs: HashMap<String, Vec<String>> = HashMap::new();
        for component in &ctx.components {
            let normalized = component.reference.trim().to_ascii_uppercase();
            refs.entry(normalized)
                .or_default()
                .push(component.reference.clone());
        }
        for duplicates in refs.into_values().filter(|entries| entries.len() > 1) {
            issues.push(BomValidationIssue {
                rule: BomRule::DuplicateDesignator,
                severity: BomIssueSeverity::Error,
                message: format!("Duplicate designator detected: {}", duplicates[0]),
                references: duplicates,
            });
        }
    }

    if options.is_enabled(BomRule::EmptyOrZeroQty) {
        for row in &table.rows {
            if row.qty == 0 {
                issues.push(BomValidationIssue {
                    rule: BomRule::EmptyOrZeroQty,
                    severity: BomIssueSeverity::Error,
                    message: "BOM row has empty or zero quantity".to_string(),
                    references: row.references.clone(),
                });
            }
        }
    }

    BomValidationReport { issues }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(reference: &str, value: &str, footprint: &str) -> BomComponent {
        BomComponent {
            reference: reference.to_string(),
            name: value.to_string(),
            value: value.to_string(),
            footprint: footprint.to_string(),
            lib_ref: "Device".to_string(),
            description: "component".to_string(),
            dnp: false,
            in_bom: true,
            on_board: true,
            variant_fitted: None,
            custom: BTreeMap::new(),
        }
    }

    #[test]
    fn groups_by_value_and_footprint() {
        let ctx = BomContext {
            components: vec![
                component("R2", "10k", "R_0603"),
                component("R1", "10k", "R_0603"),
                component("C1", "100n", "C_0603"),
            ],
            metadata: BomMetadata::default(),
        };

        let table = build_table(&ctx, &BomEngineOptions::default());

        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].references, vec!["C1".to_string()]);
        assert_eq!(table.rows[1].references, vec!["R1".to_string(), "R2".to_string()]);
        assert_eq!(table.rows[1].qty, 2);
        assert_eq!(table.rows[1].fitted_qty, 2);
        assert_eq!(table.rows[1].not_fitted_qty, 0);
    }

    #[test]
    fn filters_dnp_by_default() {
        let mut dnp = component("R2", "10k", "R_0603");
        dnp.dnp = true;

        let ctx = BomContext {
            components: vec![component("R1", "10k", "R_0603"), dnp],
            metadata: BomMetadata::default(),
        };

        let table = build_table(&ctx, &BomEngineOptions::default());
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].references, vec!["R1".to_string()]);
    }

    #[test]
    fn can_include_excluded_from_bom_and_not_on_board() {
        let mut excluded = component("U1", "MCU", "QFN32");
        excluded.in_bom = false;
        excluded.on_board = false;

        let ctx = BomContext {
            components: vec![excluded],
            metadata: BomMetadata::default(),
        };

        let table_default = build_table(&ctx, &BomEngineOptions::default());
        assert!(table_default.rows.is_empty());

        let table_all = build_table(
            &ctx,
            &BomEngineOptions {
                include_not_fitted: true,
                include_excluded_from_bom: true,
                include_not_on_board: true,
                ..BomEngineOptions::default()
            },
        );

        assert_eq!(table_all.rows.len(), 1);
        assert_eq!(table_all.rows[0].references, vec!["U1".to_string()]);
    }

    #[test]
    fn validation_reports_duplicate_designators() {
        let ctx = BomContext {
            components: vec![
                component("R1", "10k", "R_0603"),
                component("R1", "10k", "R_0603"),
            ],
            metadata: BomMetadata::default(),
        };

        let table = build_table(&ctx, &BomEngineOptions::default());
        let report = validate_table(&ctx, &table, &BomRuleOptions::default());

        assert!(report
            .issues
            .iter()
            .any(|issue| issue.rule == BomRule::DuplicateDesignator));
        assert!(report.has_errors());
    }

    #[test]
    fn validation_can_disable_missing_footprint_rule() {
        let ctx = BomContext {
            components: vec![component("R1", "10k", "")],
            metadata: BomMetadata::default(),
        };

        let table = build_table(&ctx, &BomEngineOptions::default());
        let report = validate_table(
            &ctx,
            &table,
            &BomRuleOptions {
                missing_footprint: false,
                ..BomRuleOptions::default()
            },
        );

        assert!(!report
            .issues
            .iter()
            .any(|issue| issue.rule == BomRule::MissingFootprint));
    }
}
