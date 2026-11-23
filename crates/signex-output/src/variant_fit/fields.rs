//! The fit-state fields of one placed symbol, read for Base or for one
//! variant. The only reader of these fields: the BOM and the Variants
//! section both come through [`super::part_is_fitted`] or the resolvers here.

use std::collections::HashMap;

use signex_types::schematic::Symbol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariantFieldKind {
    Fitted,
    Dnp,
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
pub(crate) fn is_fit_state_field(key: &str) -> bool {
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
pub(crate) fn field_value_ci<'a>(
    fields: &'a HashMap<String, String>,
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
    symbol: &Symbol,
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

/// Fit state from `symbol`'s own base fields (`VariantFitted`, `Fitted`,
/// `DNP`, then the same keys as custom properties), if any says one.
pub(crate) fn resolve_base_variant_fitted(symbol: &Symbol) -> Option<bool> {
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
fn resolve_variant_fitted_from_fields(symbol: &Symbol, active_variant: &str) -> Option<bool> {
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
pub(crate) fn resolve_active_variant_fitted(
    symbol: &Symbol,
    active_variant: Option<&str>,
) -> Option<bool> {
    let active_variant = active_variant?.trim();
    if active_variant.is_empty() {
        return None;
    }
    resolve_variant_fitted_from_property_overrides(symbol, active_variant)
        .or_else(|| resolve_variant_fitted_from_fields(symbol, active_variant))
}

/// A fit state for the whole part from its units': not fitted if any unit says
/// so (a DNP unit makes the part DNP), else fitted if any unit says so.
pub(crate) fn merge_fit_states(states: impl IntoIterator<Item = Option<bool>>) -> Option<bool> {
    let mut merged = None;
    for state in states {
        match state {
            Some(false) => return Some(false),
            Some(true) => merged = Some(true),
            None => {}
        }
    }
    merged
}
