//! Visual + parametric diff over two Revisions. See LIBRARY_PLAN §9.
//!
//! The diff is pure-function over already-typed `Revision` pairs. It is the
//! data backbone for:
//!
//! * the visual diff renderer (drawn by signex-app — out of scope for the
//!   library crate),
//! * WS-A's auto-bump heuristic — call [`auto_bump_kind`] to decide whether
//!   a save should be a `.minor` or `.major` version bump.
//!
//! Symbol/footprint diffs use [`standard_parser::sexpr`] to walk the embedded
//! S-expression text and extract pins/pads. Pin identity is the pin number
//! (Standard pin "name" is the printed label; "number" is the silk-stable
//! identifier — we want number for diffing). Pad identity is the pad number.
//!
//! Position equality uses an epsilon (1 µm) so float round-trip noise from
//! Standard's `(at x y rot)` doesn't manufacture spurious "moved" entries.

use std::collections::{BTreeMap, BTreeSet};

use standard_parser::sexpr::{self, SExpr};

use crate::component::Revision;
use crate::embed::{ParamMap, ParamValue, SupplierLink};
use crate::lifecycle::LifecycleState;

/// Position-equality epsilon (mm). Standard coordinates round-trip in 1 µm.
const POS_EPS_MM: f64 = 1e-3;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RevisionDiff {
    pub symbol: SymbolDiff,
    pub footprint: FootprintDiff,
    pub parameters: ParameterDiff,
    pub suppliers: SupplierDiff,
    pub lifecycle: LifecycleDiff,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolDiff {
    pub added_pins: Vec<String>,
    pub removed_pins: Vec<String>,
    pub moved_pins: Vec<(String, [f64; 2], [f64; 2])>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FootprintDiff {
    pub added_pads: Vec<String>,
    pub removed_pads: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<(String, String, String)>, // key, old, new
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SupplierDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LifecycleDiff {
    pub from: Option<LifecycleState>,
    pub to: Option<LifecycleState>,
}

/// Auto-bump heuristic — see WS-A acceptance criteria.
///
/// A revision counts as **major** if any pin or pad set changed
/// (added/removed/moved); otherwise it is a **minor** bump.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BumpKind {
    Minor,
    Major,
}

/// Decide whether the change between the two revisions feeding `diff`
/// is a `Minor` or `Major` version bump.
///
/// LIBRARY_PLAN §9 / WS-D contract: empty symbol+footprint diff → Minor;
/// any pin/pad add/remove/move → Major.
pub fn auto_bump_kind(diff: &RevisionDiff) -> BumpKind {
    let symbol_changed = !diff.symbol.added_pins.is_empty()
        || !diff.symbol.removed_pins.is_empty()
        || !diff.symbol.moved_pins.is_empty();
    let footprint_changed =
        !diff.footprint.added_pads.is_empty() || !diff.footprint.removed_pads.is_empty();
    if symbol_changed || footprint_changed {
        BumpKind::Major
    } else {
        BumpKind::Minor
    }
}

/// Compute the diff from `a` to `b`. Order matters: `added_*` are present in
/// `b` but not `a`; `removed_*` are present in `a` but not `b`.
pub fn diff_revisions(a: &Revision, b: &Revision) -> RevisionDiff {
    RevisionDiff {
        symbol: diff_symbol(&a.schematic.symbol.sexpr, &b.schematic.symbol.sexpr),
        footprint: diff_footprint(&a.pcb.footprint.sexpr, &b.pcb.footprint.sexpr),
        parameters: diff_parameters(&a.shared.parameters, &b.shared.parameters),
        suppliers: diff_suppliers(&a.shared.suppliers, &b.shared.suppliers),
        lifecycle: diff_lifecycle(a.state, b.state),
    }
}

// ---------------------------------------------------------------------------
// Symbol / footprint pin & pad extraction
// ---------------------------------------------------------------------------

/// (pin_number, [x_mm, y_mm]) — number is the diff identity. Empty body or
/// unparseable text returns an empty list (treat as "no pins").
fn extract_pins(sexpr_text: &str) -> Vec<(String, [f64; 2])> {
    extract_at_keyed(sexpr_text, "pin", pin_number)
}

/// (pad_number, [x_mm, y_mm]) — first arg of `(pad N ...)` is the pad number.
fn extract_pads(sexpr_text: &str) -> Vec<(String, [f64; 2])> {
    extract_at_keyed(sexpr_text, "pad", |node| {
        node.first_arg().map(|s| s.to_string())
    })
}

/// Walk the parsed S-expr tree, collect every `(<keyword> ...)` node, and key
/// it via `key_of`. Position is read from the standard `(at x y [rot])` child.
fn extract_at_keyed(
    sexpr_text: &str,
    keyword: &str,
    key_of: impl Fn(&SExpr) -> Option<String>,
) -> Vec<(String, [f64; 2])> {
    let trimmed = sexpr_text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let parsed = match sexpr::parse(trimmed) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut stack: Vec<&SExpr> = vec![&parsed];
    while let Some(node) = stack.pop() {
        if node.keyword() == Some(keyword)
            && let Some(k) = key_of(node)
        {
            let pos = at_position(node).unwrap_or([0.0, 0.0]);
            out.push((k, pos));
        }
        for child in node.children() {
            if matches!(child, SExpr::List(_)) {
                stack.push(child);
            }
        }
    }
    out
}

/// Standard pin nodes carry the silk-stable identifier as `(number "1")`.
fn pin_number(node: &SExpr) -> Option<String> {
    node.find("number")
        .and_then(|n| n.first_arg())
        .map(|s| s.to_string())
}

fn at_position(node: &SExpr) -> Option<[f64; 2]> {
    let at = node.find("at")?;
    let x = at.arg_f64(0)?;
    let y = at.arg_f64(1)?;
    Some([x, y])
}

fn diff_symbol(a_sexpr: &str, b_sexpr: &str) -> SymbolDiff {
    let a_pins: BTreeMap<String, [f64; 2]> = extract_pins(a_sexpr).into_iter().collect();
    let b_pins: BTreeMap<String, [f64; 2]> = extract_pins(b_sexpr).into_iter().collect();

    let a_keys: BTreeSet<&String> = a_pins.keys().collect();
    let b_keys: BTreeSet<&String> = b_pins.keys().collect();

    let added: Vec<String> = b_keys.difference(&a_keys).map(|s| (*s).clone()).collect();
    let removed: Vec<String> = a_keys.difference(&b_keys).map(|s| (*s).clone()).collect();

    let mut moved: Vec<(String, [f64; 2], [f64; 2])> = Vec::new();
    for k in a_keys.intersection(&b_keys) {
        let a_pos = a_pins[*k];
        let b_pos = b_pins[*k];
        if !pos_eq(a_pos, b_pos) {
            moved.push(((*k).clone(), a_pos, b_pos));
        }
    }
    moved.sort_by(|x, y| x.0.cmp(&y.0));

    SymbolDiff {
        added_pins: added,
        removed_pins: removed,
        moved_pins: moved,
    }
}

fn diff_footprint(a_sexpr: &str, b_sexpr: &str) -> FootprintDiff {
    let a_pads: BTreeMap<String, [f64; 2]> = extract_pads(a_sexpr).into_iter().collect();
    let b_pads: BTreeMap<String, [f64; 2]> = extract_pads(b_sexpr).into_iter().collect();

    let a_keys: BTreeSet<&String> = a_pads.keys().collect();
    let b_keys: BTreeSet<&String> = b_pads.keys().collect();

    FootprintDiff {
        added_pads: b_keys.difference(&a_keys).map(|s| (*s).clone()).collect(),
        removed_pads: a_keys.difference(&b_keys).map(|s| (*s).clone()).collect(),
    }
}

fn pos_eq(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= POS_EPS_MM && (a[1] - b[1]).abs() <= POS_EPS_MM
}

// ---------------------------------------------------------------------------
// Parameter / supplier / lifecycle diffs
// ---------------------------------------------------------------------------

fn diff_parameters(a: &ParamMap, b: &ParamMap) -> ParameterDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    let a_keys: BTreeSet<&String> = a.keys().collect();
    let b_keys: BTreeSet<&String> = b.keys().collect();

    for k in b_keys.difference(&a_keys) {
        added.push((*k).clone());
    }
    for k in a_keys.difference(&b_keys) {
        removed.push((*k).clone());
    }
    for k in a_keys.intersection(&b_keys) {
        let av = &a[*k];
        let bv = &b[*k];
        if av != bv {
            changed.push(((*k).clone(), display_param(av), display_param(bv)));
        }
    }
    ParameterDiff {
        added,
        removed,
        changed,
    }
}

fn display_param(v: &ParamValue) -> String {
    match v {
        ParamValue::Text(s) => s.clone(),
        ParamValue::Number(n) => n.to_string(),
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Measurement { value, unit } => format!("{value} {unit}"),
    }
}

fn diff_suppliers(a: &[SupplierLink], b: &[SupplierLink]) -> SupplierDiff {
    let to_key = |s: &SupplierLink| format!("{}:{}", s.distributor, s.sku);
    let a_keys: BTreeSet<String> = a.iter().map(to_key).collect();
    let b_keys: BTreeSet<String> = b.iter().map(to_key).collect();

    SupplierDiff {
        added: b_keys.difference(&a_keys).cloned().collect(),
        removed: a_keys.difference(&b_keys).cloned().collect(),
    }
}

fn diff_lifecycle(a: LifecycleState, b: LifecycleState) -> LifecycleDiff {
    if a == b {
        LifecycleDiff::default()
    } else {
        LifecycleDiff {
            from: Some(a),
            to: Some(b),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{FootprintBody, PcbSide, SchematicSide, SharedSide, SymbolBody};
    use crate::identity::Version;

    const SYMBOL_2_PINS: &str = r#"
        (symbol "R0805"
            (pin passive line (at -2.54 0 0) (length 1.27)
                (name "1") (number "1"))
            (pin passive line (at  2.54 0 180) (length 1.27)
                (name "2") (number "2")))
    "#;

    const SYMBOL_3_PINS: &str = r#"
        (symbol "R0805"
            (pin passive line (at -2.54 0 0) (length 1.27)
                (name "1") (number "1"))
            (pin passive line (at  2.54 0 180) (length 1.27)
                (name "2") (number "2"))
            (pin passive line (at  0 2.54 270) (length 1.27)
                (name "3") (number "3")))
    "#;

    const SYMBOL_2_PINS_MOVED: &str = r#"
        (symbol "R0805"
            (pin passive line (at -3.81 0 0) (length 1.27)
                (name "1") (number "1"))
            (pin passive line (at  2.54 0 180) (length 1.27)
                (name "2") (number "2")))
    "#;

    fn rev_with(sym: &str, fp: &str) -> Revision {
        Revision {
            version: Version::new(1, 0),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test".into(),
            message: "fixture".into(),
            schematic: SchematicSide {
                symbol: SymbolBody {
                    sexpr: sym.to_string(),
                },
                ..Default::default()
            },
            pcb: PcbSide {
                footprint: FootprintBody {
                    sexpr: fp.to_string(),
                },
                ..Default::default()
            },
            shared: SharedSide::default(),
            content_hash: [0u8; 32],
        }
    }

    #[test]
    fn extract_pins_finds_pin_numbers_and_positions() {
        let pins = extract_pins(SYMBOL_2_PINS);
        assert_eq!(pins.len(), 2);
        let by_num: BTreeMap<String, [f64; 2]> = pins.into_iter().collect();
        assert!(by_num.contains_key("1"));
        assert!(by_num.contains_key("2"));
        assert!((by_num["1"][0] - -2.54).abs() < POS_EPS_MM);
    }

    #[test]
    fn extract_pins_handles_empty_body() {
        assert!(extract_pins("").is_empty());
        assert!(extract_pins("   ").is_empty());
        assert!(extract_pins("(symbol R0805)").is_empty());
    }

    #[test]
    fn empty_symbol_diff_when_bodies_equal() {
        let a = rev_with(SYMBOL_2_PINS, "");
        let b = rev_with(SYMBOL_2_PINS, "");
        let d = diff_revisions(&a, &b);
        assert!(d.symbol.added_pins.is_empty());
        assert!(d.symbol.removed_pins.is_empty());
        assert!(d.symbol.moved_pins.is_empty());
    }

    #[test]
    fn pin_added_shows_in_added_pins_one_way_and_removed_other_way() {
        let a = rev_with(SYMBOL_2_PINS, "");
        let b = rev_with(SYMBOL_3_PINS, "");
        let forward = diff_revisions(&a, &b);
        let reverse = diff_revisions(&b, &a);
        assert_eq!(forward.symbol.added_pins, vec!["3".to_string()]);
        assert!(forward.symbol.removed_pins.is_empty());
        assert!(reverse.symbol.added_pins.is_empty());
        assert_eq!(reverse.symbol.removed_pins, vec!["3".to_string()]);
    }

    #[test]
    fn moved_pin_recorded_with_old_and_new_positions() {
        let a = rev_with(SYMBOL_2_PINS, "");
        let b = rev_with(SYMBOL_2_PINS_MOVED, "");
        let d = diff_revisions(&a, &b);
        assert_eq!(d.symbol.moved_pins.len(), 1);
        let (num, from, to) = &d.symbol.moved_pins[0];
        assert_eq!(num, "1");
        assert!((from[0] - -2.54).abs() < POS_EPS_MM);
        assert!((to[0] - -3.81).abs() < POS_EPS_MM);
    }

    #[test]
    fn footprint_pad_diff_keyed_by_pad_number() {
        let fp_a = r#"(footprint "R_0805_2012Metric"
            (pad "1" smd rect (at -1.0 0))
            (pad "2" smd rect (at  1.0 0)))"#;
        let fp_b = r#"(footprint "R_0805_2012Metric"
            (pad "1" smd rect (at -1.0 0))
            (pad "2" smd rect (at  1.0 0))
            (pad "3" smd rect (at  0.0 1.0)))"#;
        let a = rev_with("", fp_a);
        let b = rev_with("", fp_b);
        let d = diff_revisions(&a, &b);
        assert_eq!(d.footprint.added_pads, vec!["3".to_string()]);
        assert!(d.footprint.removed_pads.is_empty());
    }

    #[test]
    fn parameter_diff_added_removed_changed() {
        let mut a = rev_with("", "");
        let mut b = rev_with("", "");
        a.shared
            .parameters
            .insert("value".into(), ParamValue::Text("10k".into()));
        a.shared
            .parameters
            .insert("tolerance".into(), ParamValue::Text("1%".into()));
        b.shared
            .parameters
            .insert("value".into(), ParamValue::Text("10k".into()));
        b.shared
            .parameters
            .insert("package".into(), ParamValue::Text("0805".into()));
        b.shared
            .parameters
            .insert("tolerance".into(), ParamValue::Text("0.1%".into()));

        let d = diff_revisions(&a, &b);
        assert_eq!(d.parameters.added, vec!["package".to_string()]);
        assert!(d.parameters.removed.is_empty());
        assert_eq!(d.parameters.changed.len(), 1);
        assert_eq!(d.parameters.changed[0].0, "tolerance");
        assert_eq!(d.parameters.changed[0].1, "1%");
        assert_eq!(d.parameters.changed[0].2, "0.1%");
    }

    #[test]
    fn supplier_diff_keyed_by_distributor_and_sku_tuple() {
        let mut a = rev_with("", "");
        let mut b = rev_with("", "");
        a.shared.suppliers.push(SupplierLink {
            distributor: "DigiKey".into(),
            sku: "DK-1".into(),
            url: None,
        });
        b.shared.suppliers.push(SupplierLink {
            distributor: "DigiKey".into(),
            sku: "DK-1".into(),
            url: None,
        });
        b.shared.suppliers.push(SupplierLink {
            distributor: "Mouser".into(),
            sku: "M-1".into(),
            url: None,
        });

        let d = diff_revisions(&a, &b);
        assert_eq!(d.suppliers.added, vec!["Mouser:M-1".to_string()]);
        assert!(d.suppliers.removed.is_empty());
    }

    #[test]
    fn lifecycle_diff_only_when_state_actually_changes() {
        let mut a = rev_with("", "");
        let mut b = rev_with("", "");
        a.state = LifecycleState::Released;
        b.state = LifecycleState::Released;
        let d = diff_revisions(&a, &b);
        assert!(d.lifecycle.from.is_none() && d.lifecycle.to.is_none());

        b.state = LifecycleState::Deprecated;
        let d = diff_revisions(&a, &b);
        assert_eq!(d.lifecycle.from, Some(LifecycleState::Released));
        assert_eq!(d.lifecycle.to, Some(LifecycleState::Deprecated));
    }

    #[test]
    fn auto_bump_is_minor_when_only_metadata_changes() {
        let a = rev_with(SYMBOL_2_PINS, "");
        let mut b = rev_with(SYMBOL_2_PINS, "");
        b.shared.mpn = "ANOTHER_MPN".into();
        let d = diff_revisions(&a, &b);
        assert_eq!(auto_bump_kind(&d), BumpKind::Minor);
    }

    #[test]
    fn auto_bump_is_major_when_pin_count_changes() {
        let a = rev_with(SYMBOL_2_PINS, "");
        let b = rev_with(SYMBOL_3_PINS, "");
        let d = diff_revisions(&a, &b);
        assert_eq!(auto_bump_kind(&d), BumpKind::Major);
    }

    #[test]
    fn auto_bump_is_major_when_pin_moves() {
        let a = rev_with(SYMBOL_2_PINS, "");
        let b = rev_with(SYMBOL_2_PINS_MOVED, "");
        let d = diff_revisions(&a, &b);
        assert_eq!(auto_bump_kind(&d), BumpKind::Major);
    }
}
