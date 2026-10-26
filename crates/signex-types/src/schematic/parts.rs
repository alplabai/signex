//! Grouping placed symbol instances into physical parts (#691).
//!
//! A multi-unit part (a dual op-amp, a quad gate) is placed as several
//! [`Symbol`](super::Symbol) instances that share one designator and differ
//! in `unit`. Annotation, ERC and the BOM all have to agree on which
//! instances are one part, so the rule lives here and nowhere else.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::Symbol;

/// The identity fields of a placed symbol that decide part membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartKey<'a> {
    pub reference: &'a str,
    pub lib_id: &'a str,
    pub unit: u32,
}

/// A library id as part grouping compares it: surrounding whitespace removed,
/// case kept. Library lookups (`lib_symbols`) match the id exactly, so two ids
/// that differ in case name two different library symbols; only stray
/// whitespace from hand-edited files is forgiven.
///
/// Two units of one designator that resolve to different library symbols are
/// not one part. ERC says so when it reports the designator: "units of U1 come
/// from different library symbols".
pub fn normalized_lib_id(lib_id: &str) -> &str {
    lib_id.trim()
}

impl Symbol {
    /// The fields [`group_into_parts`] groups by.
    pub fn part_key(&self) -> PartKey<'_> {
        PartKey {
            reference: self.reference.trim(),
            lib_id: normalized_lib_id(&self.lib_id),
            unit: self.unit,
        }
    }
}

/// Group instances into physical parts. Returns one `Vec` of input positions
/// per part, parts ordered by their first member, members in input order.
///
/// Two instances are units of one part when they share a designator and a
/// library symbol and carry different unit numbers. The same unit placed
/// twice under one designator is two parts (a duplicate designator, not a
/// second unit), and so are instances of different library symbols. Callers
/// choose the input order; it decides which instances pair up when several
/// unannotated (`U?`) parts of one library symbol are mixed together.
pub fn group_into_parts<'a>(items: impl IntoIterator<Item = PartKey<'a>>) -> Vec<Vec<usize>> {
    let mut parts: Vec<Vec<usize>> = Vec::new();
    // Unit numbers each part already holds, parallel to `parts`.
    let mut taken_units: Vec<Vec<u32>> = Vec::new();
    // Parts per (designator, library symbol), in creation order.
    let mut open: HashMap<(&str, &str), Vec<usize>> = HashMap::new();

    for (position, key) in items.into_iter().enumerate() {
        let candidates = open.entry((key.reference, key.lib_id)).or_default();
        match candidates
            .iter()
            .copied()
            .find(|&part| !taken_units[part].contains(&key.unit))
        {
            Some(part) => {
                taken_units[part].push(key.unit);
                parts[part].push(position);
            }
            None => {
                candidates.push(parts.len());
                taken_units.push(vec![key.unit]);
                parts.push(vec![position]);
            }
        }
    }
    parts
}

/// Order in which Annotate visits symbols. Schematic coordinates are y-down
/// (see `SymbolTransform`): a larger `y` sits lower on screen, so "Up" means
/// decreasing `y` and "Down" means increasing `y`; "Across" is always
/// increasing `x` (left to right).
///
/// Positions are compared exactly, with no row/column banding: two symbols
/// share a row (or column) only when their `y` (or `x`) is equal. Symbols with
/// identical positions fall back to uuid so the result is deterministic.
/// A multi-unit part is numbered when its first unit in this order is reached,
/// so the order also picks which unit is the part's anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnnotateOrder {
    /// Columns left to right (primary: `x` ascending); inside a column
    /// bottom to top (secondary: `y` descending).
    UpThenAcross,
    /// Columns left to right (primary: `x` ascending); inside a column
    /// top to bottom (secondary: `y` ascending).
    DownThenAcross,
    /// Rows top to bottom (primary: `y` ascending); inside a row left to
    /// right (secondary: `x` ascending). This is the order Annotate used
    /// before the setting was honoured, and the dialog default.
    #[default]
    AcrossThenDown,
    /// Rows bottom to top (primary: `y` descending); inside a row left to
    /// right (secondary: `x` ascending).
    AcrossThenUp,
}

/// Symbol indices in annotation order for `order`. Annotation and its preview
/// both walk this order, so the number the dialog promises a symbol is the
/// number the pass assigns it.
pub fn annotation_order(symbols: &[Symbol], order: AnnotateOrder) -> Vec<usize> {
    use std::cmp::Ordering;
    let cmp = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(Ordering::Equal);
    let mut indices: Vec<usize> = (0..symbols.len()).collect();
    indices.sort_by(|&a, &b| {
        let (pa, pb) = (&symbols[a].position, &symbols[b].position);
        let x = cmp(pa.x, pb.x);
        let y = cmp(pa.y, pb.y);
        let by_position = match order {
            AnnotateOrder::AcrossThenDown => y.then(x),
            AnnotateOrder::AcrossThenUp => y.reverse().then(x),
            AnnotateOrder::DownThenAcross => x.then(y),
            AnnotateOrder::UpThenAcross => x.then(y.reverse()),
        };
        by_position.then(symbols[a].uuid.cmp(&symbols[b].uuid))
    });
    indices
}

/// Parts of a whole project, grouped across every sheet before any pass
/// renumbers anything, so a part whose units sit on different sheets stays one
/// part. Sheets are addressed by their ordinal in the slice given to
/// [`ProjectParts::new`]; the caller fixes that order (it decides which `U?`
/// units pair up) and keeps it for every later lookup. The parts also own the
/// [`AnnotateOrder`] symbols are visited in, so the grouping and the pass that
/// numbers by it cannot disagree.
///
/// Only symbols a designator applies to take part (not power ports, not `#`
/// markers).
///
/// # Locks
///
/// A part with a locked unit keeps its designator as a whole. Its unlocked
/// units join the locked unit's designator; when that designator is still
/// unannotated (`U?`) the part stays unannotated, so a lock never leaves one
/// part holding two different designators.
#[derive(Debug, Default)]
pub struct ProjectParts {
    part_of: HashMap<(usize, usize), usize>,
    locked_designator: HashMap<usize, String>,
    order: AnnotateOrder,
}

impl ProjectParts {
    pub fn new(sheets: &[&[Symbol]], locked: &HashSet<Uuid>, order: AnnotateOrder) -> Self {
        let mut eligible: Vec<(usize, usize)> = Vec::new();
        for (sheet, symbols) in sheets.iter().enumerate() {
            for idx in annotation_order(symbols, order) {
                let symbol = &symbols[idx];
                if !symbol.is_power && !symbol.reference.starts_with('#') {
                    eligible.push((sheet, idx));
                }
            }
        }
        let parts = group_into_parts(
            eligible
                .iter()
                .map(|&(sheet, idx)| sheets[sheet][idx].part_key()),
        );
        let mut part_of = HashMap::new();
        let mut locked_designator = HashMap::new();
        for (part, members) in parts.iter().enumerate() {
            for &member in members {
                let (sheet, idx) = eligible[member];
                part_of.insert((sheet, idx), part);
                let symbol = &sheets[sheet][idx];
                if locked.contains(&symbol.uuid) {
                    locked_designator
                        .entry(part)
                        .or_insert_with(|| symbol.reference.clone());
                }
            }
        }
        Self {
            part_of,
            locked_designator,
            order,
        }
    }

    /// The order the parts were grouped in; the annotate pass visits each
    /// sheet's symbols in this same order.
    pub fn order(&self) -> AnnotateOrder {
        self.order
    }

    /// The part a symbol belongs to, or `None` for a symbol designators skip.
    pub fn part(&self, sheet: usize, symbol: usize) -> Option<usize> {
        self.part_of.get(&(sheet, symbol)).copied()
    }

    /// The designator a part keeps because one of its units is locked.
    pub fn locked_designator(&self, part: usize) -> Option<&str> {
        self.locked_designator.get(&part).map(String::as_str)
    }
}

/// References held by more than one distinct part, ignoring unannotated
/// (`?`) ones. Units of one part sharing a designator are not duplicates.
/// `symbols` may span every sheet of a project.
pub fn duplicate_designators<'a>(symbols: impl IntoIterator<Item = &'a Symbol>) -> HashSet<String> {
    let placed: Vec<&Symbol> = symbols
        .into_iter()
        .filter(|s| {
            !s.is_power && !s.reference.starts_with('#') && !s.reference.trim().ends_with('?')
        })
        .collect();
    let parts = group_into_parts(placed.iter().map(|s| s.part_key()));
    let mut parts_per_reference: HashMap<&str, usize> = HashMap::new();
    for members in &parts {
        *parts_per_reference
            .entry(placed[members[0]].reference.trim())
            .or_default() += 1;
    }
    parts_per_reference
        .into_iter()
        .filter(|&(_, count)| count > 1)
        .map(|(reference, _)| reference.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key<'a>(reference: &'a str, lib_id: &'a str, unit: u32) -> PartKey<'a> {
        PartKey {
            reference,
            lib_id,
            unit,
        }
    }

    #[test]
    fn units_with_one_designator_form_one_part() {
        let parts = group_into_parts([key("U1", "A", 1), key("U1", "A", 2)]);
        assert_eq!(parts, vec![vec![0, 1]]);
    }

    #[test]
    fn same_unit_twice_is_two_parts() {
        let parts = group_into_parts([key("U1", "A", 1), key("U1", "A", 1)]);
        assert_eq!(parts, vec![vec![0], vec![1]]);
    }

    #[test]
    fn different_library_symbols_are_different_parts() {
        let parts = group_into_parts([key("U1", "A", 1), key("U1", "B", 2)]);
        assert_eq!(parts, vec![vec![0], vec![1]]);
    }

    #[test]
    fn unannotated_units_pair_in_input_order() {
        let parts = group_into_parts([
            key("U?", "A", 1),
            key("U?", "A", 2),
            key("U?", "A", 1),
            key("U?", "A", 2),
        ]);
        assert_eq!(parts, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn empty_input_has_no_parts() {
        assert!(group_into_parts(std::iter::empty()).is_empty());
    }

    fn placed(reference: &str, lib_id: &str, unit: u32) -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            reference: reference.to_string(),
            lib_id: lib_id.to_string(),
            unit,
            ..Default::default()
        }
    }

    #[test]
    fn library_ids_compare_trimmed_and_case_sensitive() {
        assert_eq!(normalized_lib_id("  Lib:Part "), "Lib:Part");
        assert_ne!(normalized_lib_id("Lib:Part"), normalized_lib_id("lib:part"));
        let parts = group_into_parts([
            placed("U1", "A", 1).part_key(),
            placed("U1", " A ", 2).part_key(),
        ]);
        assert_eq!(parts, vec![vec![0, 1]]);
    }

    #[test]
    fn project_parts_join_units_across_sheets() {
        let (a, b) = ([placed("U1", "A", 1)], [placed("U1", "A", 2)]);
        let parts = ProjectParts::new(&[&a, &b], &HashSet::new(), AnnotateOrder::default());
        assert_eq!(parts.part(0, 0), parts.part(1, 0));
        assert!(parts.part(0, 0).is_some());
    }

    /// Units A(0,0) u1, B(10,0) u2, C(0,10) u2, D(10,10) u1 of one library
    /// symbol: which unit-1 pairs with which unit-2 depends on the visit
    /// order, and the parts now carry that order themselves.
    fn quad() -> [Symbol; 4] {
        let at = |reference: &str, unit: u32, x: f64, y: f64| {
            let mut s = placed(reference, "A", unit);
            s.position = crate::schematic::Point::new(x, y);
            s
        };
        [
            at("U?", 1, 0.0, 0.0),
            at("U?", 2, 10.0, 0.0),
            at("U?", 2, 0.0, 10.0),
            at("U?", 1, 10.0, 10.0),
        ]
    }

    #[test]
    fn the_pairing_follows_the_order_the_parts_own() {
        let sheet = quad();
        let pair = |order| {
            let parts = ProjectParts::new(&[&sheet], &HashSet::new(), order);
            assert_eq!(parts.order(), order);
            let p = |i| parts.part(0, i).unwrap();
            (p(0) == p(1), p(2) == p(3), p(0) == p(2), p(1) == p(3))
        };
        // (A,B)+(C,D) across then down; (A,C)+(B,D) down then across.
        assert_eq!(
            pair(AnnotateOrder::AcrossThenDown),
            (true, true, false, false)
        );
        assert_eq!(
            pair(AnnotateOrder::DownThenAcross),
            (false, false, true, true)
        );
    }

    #[test]
    fn project_parts_skip_power_ports_and_hash_markers() {
        let mut power = placed("#PWR01", "pwr", 1);
        power.is_power = true;
        let marker = placed("#FLG01", "flag", 1);
        let sheet = [power, marker];
        let parts = ProjectParts::new(&[&sheet], &HashSet::new(), AnnotateOrder::default());
        assert_eq!(parts.part(0, 0), None);
        assert_eq!(parts.part(0, 1), None);
    }

    #[test]
    fn a_lock_pins_the_whole_part_to_the_locked_designator() {
        let locked_unit = placed("U?", "A", 1);
        let locked = HashSet::from([locked_unit.uuid]);
        let sheet = [locked_unit, placed("U?", "A", 2)];
        let parts = ProjectParts::new(&[&sheet], &locked, AnnotateOrder::default());
        let part = parts.part(0, 1).unwrap();
        assert_eq!(parts.locked_designator(part), Some("U?"));
    }

    #[test]
    fn units_of_one_part_are_not_duplicate_designators() {
        let sheet = [placed("U1", "A", 1), placed("U1", "A", 2)];
        assert!(duplicate_designators(&sheet).is_empty());
    }

    #[test]
    fn a_repeated_unit_is_a_duplicate_designator() {
        let sheet = [placed("U1", "A", 1), placed("U1", "A", 1)];
        assert_eq!(
            duplicate_designators(&sheet),
            HashSet::from(["U1".to_string()])
        );
    }

    #[test]
    fn unannotated_references_are_never_duplicates() {
        let sheet = [placed("U?", "A", 1), placed("U?", "A", 1)];
        assert!(duplicate_designators(&sheet).is_empty());
    }
}
