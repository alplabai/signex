//! Alternate footprints for one component row (#1234): the overloaded
//! `footprint_ref` cell codec and the mount-style pick. Format and invariants:
//! `docs/architecture/library-footprint-bindings.md`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::component::PinPadOverride;
use crate::primitive::PrimitiveRef;
use crate::primitive::footprint::MountStyle;

/// One footprint a component can be built with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintBinding {
    pub footprint: PrimitiveRef,
    /// Pinned footprint version; empty = follow latest.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Pin-to-pad overrides; empty = default 1:1 by number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pin_map_overrides: Vec<PinPadOverride>,
}

impl FootprintBinding {
    /// Binding with no pinned version and no pin-map overrides.
    pub fn new(footprint: PrimitiveRef) -> Self {
        Self {
            footprint,
            version: String::new(),
            pin_map_overrides: Vec::new(),
        }
    }
}

/// Failure encoding or decoding the `footprint_ref` cell.
#[derive(Debug, thiserror::Error)]
pub enum FootprintCellError {
    #[error("alternate footprints require a default footprint")]
    AlternatesWithoutDefault,
    #[error("footprint {} (library {}) is bound more than once", .0.uuid, .0.library_id)]
    DuplicateFootprint(PrimitiveRef),
    #[error("a one-element footprint array is not canonical; use the single-object form")]
    SingleElementArray,
    #[error("footprint cell array is empty")]
    EmptyArray,
    #[error("default footprint (array element 0) must not carry a version or pin map")]
    DefaultCarriesRowData,
    #[error("footprint cell is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("footprint cell must be empty, a JSON object, or a JSON array")]
    UnexpectedShape,
}

/// Encode the `footprint_ref` cell. `(None, [])` is the empty cell and
/// `(Some, [])` is byte-identical to the legacy single-ref object.
pub fn encode_footprint_cell(
    default: Option<&PrimitiveRef>,
    alternates: &[FootprintBinding],
) -> Result<String, FootprintCellError> {
    let Some(default) = default else {
        if alternates.is_empty() {
            return Ok(String::new());
        }
        return Err(FootprintCellError::AlternatesWithoutDefault);
    };
    if alternates.is_empty() {
        return Ok(serde_json::to_string(default)?);
    }
    let mut seen = HashSet::from([*default]);
    for alt in alternates {
        if !seen.insert(alt.footprint) {
            return Err(FootprintCellError::DuplicateFootprint(alt.footprint));
        }
    }
    let mut items = Vec::with_capacity(alternates.len() + 1);
    items.push(FootprintBinding::new(*default));
    items.extend(alternates.iter().cloned());
    Ok(serde_json::to_string(&items)?)
}

/// Refuse a row whose default and alternate footprints break the binding
/// invariants (alternates without a default, a footprint bound twice).
/// Every write path runs it, so no backend stores a row the TSV codec
/// could not write.
pub fn ensure_footprint_bindings_valid(
    row: &crate::component::ComponentRow,
) -> Result<(), crate::adapter::LibraryError> {
    encode_footprint_cell(row.footprint_ref.as_ref(), &row.alternate_footprints)
        .map(drop)
        .map_err(|e| crate::adapter::LibraryError::Backend(format!("footprint_ref: {e}")))
}

/// Decode the `footprint_ref` cell into `(default, alternates)`.
pub fn decode_footprint_cell(
    cell: &str,
) -> Result<(Option<PrimitiveRef>, Vec<FootprintBinding>), FootprintCellError> {
    if cell.is_empty() {
        return Ok((None, Vec::new()));
    }
    match cell.trim_start().as_bytes().first() {
        Some(b'{') => Ok((Some(serde_json::from_str(cell)?), Vec::new())),
        Some(b'[') => decode_array(cell),
        _ => Err(FootprintCellError::UnexpectedShape),
    }
}

fn decode_array(
    cell: &str,
) -> Result<(Option<PrimitiveRef>, Vec<FootprintBinding>), FootprintCellError> {
    let items: Vec<FootprintBinding> = serde_json::from_str(cell)?;
    let (first, rest) = items.split_first().ok_or(FootprintCellError::EmptyArray)?;
    if rest.is_empty() {
        return Err(FootprintCellError::SingleElementArray);
    }
    if !first.version.is_empty() || !first.pin_map_overrides.is_empty() {
        return Err(FootprintCellError::DefaultCarriesRowData);
    }
    let mut seen = HashSet::from([first.footprint]);
    for alt in rest {
        if !seen.insert(alt.footprint) {
            return Err(FootprintCellError::DuplicateFootprint(alt.footprint));
        }
    }
    Ok((Some(first.footprint), rest.to_vec()))
}

/// Pick the binding to use for a placement. `bindings` is the default first,
/// then alternates, each with its derived [`MountStyle`]. Each preferred style
/// in order takes the first binding with exactly that style; `Mixed` and
/// `Unknown` never match. No match or empty preference falls back to index 0.
pub fn pick_binding(bindings: &[MountStyle], preference: &[MountStyle]) -> Option<usize> {
    if bindings.is_empty() {
        return None;
    }
    let chosen = preference
        .iter()
        .filter(|p| matches!(p, MountStyle::Smd | MountStyle::ThroughHole))
        .find_map(|p| bindings.iter().position(|b| b == p));
    Some(chosen.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::MountStyle::{Mixed, Smd, ThroughHole as Tht, Unknown};
    use super::*;
    use uuid::Uuid;

    fn pr() -> PrimitiveRef {
        PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn single_ref_cell_matches_legacy_bytes() {
        let r = pr();
        // Legacy `opt_primitive_to_cell` is `serde_json::to_string(&ref)`.
        let legacy = serde_json::to_string(&r).unwrap();
        assert_eq!(encode_footprint_cell(Some(&r), &[]).unwrap(), legacy);
        assert_eq!(encode_footprint_cell(None, &[]).unwrap(), "");
        assert_eq!(decode_footprint_cell("").unwrap(), (None, vec![]));
        assert_eq!(decode_footprint_cell(&legacy).unwrap(), (Some(r), vec![]));
    }

    #[test]
    fn array_roundtrips_and_is_not_a_primitive_ref() {
        let d = pr();
        let mut alt = FootprintBinding::new(pr());
        alt.version = "3".into();
        alt.pin_map_overrides = vec![PinPadOverride::new("1", "A1")];
        let cell = encode_footprint_cell(Some(&d), std::slice::from_ref(&alt)).unwrap();
        assert!(cell.starts_with('['));
        assert!(serde_json::from_str::<PrimitiveRef>(&cell).is_err());
        assert_eq!(decode_footprint_cell(&cell).unwrap(), (Some(d), vec![alt]));
    }

    #[test]
    fn encode_rejects_alternates_without_default_and_duplicates() {
        let a = FootprintBinding::new(pr());
        assert!(matches!(
            encode_footprint_cell(None, std::slice::from_ref(&a)),
            Err(FootprintCellError::AlternatesWithoutDefault)
        ));
        let d = pr();
        let dup_default = FootprintBinding::new(d);
        assert!(matches!(
            encode_footprint_cell(Some(&d), &[dup_default]),
            Err(FootprintCellError::DuplicateFootprint(_))
        ));
        assert!(matches!(
            encode_footprint_cell(Some(&d), &[a.clone(), a]),
            Err(FootprintCellError::DuplicateFootprint(_))
        ));
    }

    #[test]
    fn decode_rejects_bad_cells() {
        assert!(matches!(
            decode_footprint_cell("[]"),
            Err(FootprintCellError::EmptyArray)
        ));
        assert!(matches!(
            decode_footprint_cell("nope"),
            Err(FootprintCellError::UnexpectedShape)
        ));
        let mut d = FootprintBinding::new(pr());
        d.version = "2".into();
        let cell = serde_json::to_string(&[d, FootprintBinding::new(pr())]).unwrap();
        assert!(matches!(
            decode_footprint_cell(&cell),
            Err(FootprintCellError::DefaultCarriesRowData)
        ));
        let a = FootprintBinding::new(pr());
        let cell = serde_json::to_string(&[a.clone(), a]).unwrap();
        assert!(matches!(
            decode_footprint_cell(&cell),
            Err(FootprintCellError::DuplicateFootprint(_))
        ));
    }

    #[test]
    fn same_uuid_in_another_library_is_distinct() {
        let d = pr();
        let other = FootprintBinding::new(PrimitiveRef::new(Uuid::new_v4(), d.uuid));
        let cell = encode_footprint_cell(Some(&d), std::slice::from_ref(&other)).unwrap();
        assert_eq!(
            decode_footprint_cell(&cell).unwrap(),
            (Some(d), vec![other])
        );
    }

    #[test]
    fn single_element_array_is_rejected() {
        let cell = serde_json::to_string(&[FootprintBinding::new(pr())]).unwrap();
        assert!(matches!(
            decode_footprint_cell(&cell),
            Err(FootprintCellError::SingleElementArray)
        ));
    }

    #[test]
    fn leading_whitespace_is_tolerated() {
        let r = pr();
        let cell = format!("  \n{}", serde_json::to_string(&r).unwrap());
        assert_eq!(decode_footprint_cell(&cell).unwrap(), (Some(r), vec![]));
        assert!(matches!(
            decode_footprint_cell("   "),
            Err(FootprintCellError::UnexpectedShape)
        ));
    }

    #[test]
    fn unknown_keys_in_array_elements_fail() {
        let (a, b) = (pr(), pr());
        let cell = format!(
            r#"[{{"footprint":{}}},{{"footprint":{},"extra":1}}]"#,
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        assert!(matches!(
            decode_footprint_cell(&cell),
            Err(FootprintCellError::Json(_))
        ));
    }

    #[test]
    fn pick_binding_table() {
        let cases: &[(&[MountStyle], &[MountStyle], Option<usize>)] = &[
            (&[], &[Smd], None),
            (&[Smd, Tht], &[], Some(0)),
            (&[Smd, Tht], &[Tht], Some(1)),
            (&[Smd, Tht], &[Tht, Smd], Some(1)),
            (&[Tht, Smd, Smd], &[Smd], Some(1)),
            (&[Smd, Mixed], &[Mixed], Some(0)),
            (&[Smd, Unknown, Tht], &[Unknown, Tht], Some(2)),
            (&[Smd, Smd], &[Tht], Some(0)),
        ];
        for (bindings, pref, want) in cases {
            assert_eq!(pick_binding(bindings, pref), *want, "{bindings:?} {pref:?}");
        }
    }
}
