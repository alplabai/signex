//! What a Smart Paste places (mockup row 155), as pure functions of the dialog's
//! choices: the copies and where each sits, the net names they carry, and
//! the After paste table. Power nets are never rewritten.

use std::collections::HashSet;

use signex_engine::PlacementPayload;
use signex_types::object_id::UuidV7Ids;
use signex_types::schematic::{LabelType, SchematicSheet};

/// Grid the preview drag snaps the first copy to, in mm.
pub const DRAG_GRID_MM: f64 = 1.27;
/// The most copies an array places.
pub const MAX_COPIES: u32 = 64;

/// How the copies are laid out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PlaceMode {
    /// One copy, `offset` from the anchor.
    #[default]
    Offset,
    /// `copies` copies, copy `k` `k` steps from the anchor.
    Array,
}

/// What the pasted parts are called.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DesignatorChoice {
    /// The next free number of each prefix in the project.
    #[default]
    NextFree,
    /// As copied unless the project already uses the designator; a taken one
    /// takes the next free number of its prefix (DECISIONS 2026-10-31, #1768).
    KeepIfFree,
}

/// What the pasted net labels are called.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NetChoice {
    /// As copied: the pasted nets join the originals.
    Keep,
    /// Rewritten with the pattern, `${n}` counting from Start per copy.
    #[default]
    Rewrite,
}

/// The dialog's choices, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct PasteInput {
    pub copies: u32,
    /// Offset of copy 1 (and step between copies) in mm.
    pub dx: f64,
    pub dy: f64,
    pub designators: DesignatorChoice,
    /// `Some((pattern, start))` when nets are rewritten.
    pub rewrite: Option<(String, i64)>,
}

/// Why the choices cannot be pasted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputProblem {
    /// An offset or step is not a number.
    Distance,
    /// Copies is not a whole number from 1 to [`MAX_COPIES`].
    Copies,
    /// Start n at is not a whole number.
    Start,
    /// The pattern would give two nets one name: it needs `${n}` for several
    /// copies, `${Net}` for several signal nets, and one of them always.
    Pattern,
}

/// A distance typed in mm.
#[must_use]
pub fn parse_mm(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.abs() < 10_000.0)
}

/// `value` on the drag grid.
#[must_use]
pub fn snapped(value: f64) -> f64 {
    (value / DRAG_GRID_MM).round() * DRAG_GRID_MM
}

/// `net` written with `pattern` for copy number `n`: `${Net}` is the copied
/// name, `${n}` the number.
#[must_use]
pub fn rewritten(pattern: &str, net: &str, n: i64) -> String {
    pattern
        .replace("${Net}", net)
        .replace("${n}", &n.to_string())
}

/// Whether `pattern` keeps `copies` copies of `signal_nets` nets apart.
#[must_use]
pub fn pattern_separates(pattern: &str, copies: u32, signal_nets: usize) -> bool {
    let (has_net, has_n) = (pattern.contains("${Net}"), pattern.contains("${n}"));
    (has_net || has_n)
        && (has_n || copies <= 1 || signal_nets == 0)
        && (has_net || signal_nets <= 1)
}

/// Whether a copied net is a power net: a power label, never rewritten.
#[must_use]
pub fn is_power_label(label_type: LabelType) -> bool {
    matches!(label_type, LabelType::Power)
}

/// The nets the clipboard names, each once, in order: label texts, then the
/// power ports' nets. `true` marks a power net.
#[must_use]
pub fn copied_nets(fragment: &SchematicSheet) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut push = |name: &str, power: bool| {
        if !name.is_empty() && !out.iter().any(|(n, _)| n == name) {
            out.push((name.to_string(), power));
        }
    };
    for label in &fragment.labels {
        push(&label.text, is_power_label(label.label_type));
    }
    for symbol in fragment.symbols.iter().filter(|s| s.is_power) {
        push(&symbol.value, true);
    }
    out
}

/// What one copied net is called after the paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetAfter {
    /// Unchanged: the pasted net joins the original.
    Joins,
    /// A power net, never rewritten.
    Power,
    /// The new name of each copy.
    Renamed(Vec<String>),
}

/// The After paste table.
#[must_use]
pub fn after_paste(nets: &[(String, bool)], input: &PasteInput) -> Vec<(String, NetAfter)> {
    nets.iter()
        .map(|(net, power)| {
            let after = match (&input.rewrite, power) {
                (_, true) => NetAfter::Power,
                (None, false) => NetAfter::Joins,
                (Some((pattern, start)), false) => NetAfter::Renamed(
                    (0..input.copies)
                        .map(|k| rewritten(pattern, net, start + i64::from(k)))
                        .collect(),
                ),
            };
            (net.clone(), after)
        })
        .collect()
}

/// The rewritten names that would join a net (#2744), each once, in table
/// order: a name already in `in_use`, a copied power net's name, or a name
/// two rewritten nets share. Names compare trimmed, as [`signex_net`] lists
/// them. Empty when nets are kept: that join is the user's choice.
#[must_use]
pub fn taken_names(
    nets: &[(String, bool)],
    input: &PasteInput,
    in_use: &HashSet<String>,
) -> Vec<String> {
    let mut seen: HashSet<String> = nets
        .iter()
        .filter(|(_, power)| *power)
        .map(|(name, _)| name.trim().to_owned())
        .collect();
    let mut taken: Vec<String> = Vec::new();
    for (_, after) in after_paste(nets, input) {
        let NetAfter::Renamed(names) = after else {
            continue;
        };
        for name in names {
            let name = name.trim().to_owned();
            let clashes = in_use.contains(&name) || !seen.insert(name.clone());
            if clashes && !taken.contains(&name) {
                taken.push(name);
            }
        }
    }
    taken
}

/// The payload a Smart Paste puts on the cursor: every copy at its place
/// from the anchor (copy `k` at `k` times the offset), its net labels
/// rewritten, the library bodies once. Designators are given at the click.
#[must_use]
pub fn copies_payload(payload: &PlacementPayload, input: &PasteInput) -> PlacementPayload {
    payload.repeated(
        input.copies,
        input.dx,
        input.dy,
        &mut UuidV7Ids,
        |k, label| {
            let (pattern, start) = input.rewrite.as_ref()?;
            (!is_power_label(label.label_type))
                .then(|| rewritten(pattern, &label.text, start + i64::from(k) - 1))
        },
    )
}

/// "R4 → R6, R5 → R7" for one copy, or "R4 → R6, R8; R5 → R7, R9" for
/// several: each copied part's designator and what each copy calls it.
/// `numbered` is [`copies_payload`] after the numbering, `per_copy` the parts
/// of one copy.
#[must_use]
pub fn designator_mapping(
    source: &SchematicSheet,
    numbered: &SchematicSheet,
    copies: u32,
) -> Vec<(String, Vec<String>)> {
    let per_copy = source.symbols.len();
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (i, original) in source.symbols.iter().enumerate() {
        if original.is_power || original.reference.starts_with('#') {
            continue;
        }
        if out.iter().any(|(r, _)| *r == original.reference) {
            continue;
        }
        let new: Vec<String> = (0..copies as usize)
            .filter_map(|k| numbered.symbols.get(k * per_copy + i))
            .map(|s| s.reference.clone())
            .collect();
        out.push((original.reference.clone(), new));
    }
    out
}

/// The mapping as one line: `R4 → R6, R5 → R7` (one copy) or
/// `R4 → R6, R8; R5 → R7, R9` (several).
#[must_use]
pub fn mapping_text(mapping: &[(String, Vec<String>)], copies: u32) -> String {
    let separator = if copies > 1 { "; " } else { ", " };
    mapping
        .iter()
        .map(|(old, new)| format!("{old} \u{2192} {}", new.join(", ")))
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        HarnessConnector, HarnessEntry, HarnessWire, Label, Point, Symbol,
    };
    use uuid::Uuid;

    fn fragment() -> SchematicSheet {
        let mut f = SchematicSheet::default();
        for (n, reference) in ["R4", "R5", "C5"].iter().enumerate() {
            f.symbols.push(Symbol {
                uuid: Uuid::from_u128(n as u128 + 1),
                lib_id: format!("lib:{}", &reference[..1]),
                reference: (*reference).into(),
                position: Point::new(n as f64 * 5.08, 0.0),
                ..Default::default()
            });
        }
        for (n, (text, kind)) in [
            ("FB", LabelType::Net),
            ("VOUT_3V3", LabelType::Net),
            ("GND", LabelType::Power),
        ]
        .into_iter()
        .enumerate()
        {
            f.labels.push(Label {
                uuid: Uuid::from_u128(10 + n as u128),
                ..Label::new(text, Point::new(0.0, n as f64 * 2.54), kind)
            });
        }
        f
    }

    fn input(copies: u32, rewrite: bool) -> PasteInput {
        PasteInput {
            copies,
            dx: 20.32,
            dy: 0.0,
            designators: DesignatorChoice::NextFree,
            rewrite: rewrite.then(|| ("${Net}_${n}".to_string(), 2)),
        }
    }

    #[test]
    fn the_pattern_rewrites_signal_nets_and_never_power_nets() {
        let rows = after_paste(&copied_nets(&fragment()), &input(3, true));
        assert_eq!(
            rows,
            [
                (
                    "FB".to_string(),
                    NetAfter::Renamed(vec!["FB_2".into(), "FB_3".into(), "FB_4".into()])
                ),
                (
                    "VOUT_3V3".to_string(),
                    NetAfter::Renamed(vec![
                        "VOUT_3V3_2".into(),
                        "VOUT_3V3_3".into(),
                        "VOUT_3V3_4".into()
                    ])
                ),
                ("GND".to_string(), NetAfter::Power),
            ]
        );
        let kept = after_paste(&copied_nets(&fragment()), &input(1, false));
        assert_eq!(kept[0].1, NetAfter::Joins);
        assert_eq!(kept[2].1, NetAfter::Power);
    }

    #[test]
    fn an_array_lays_each_copy_one_step_further_with_its_own_ids() {
        let payload = PlacementPayload::new(fragment());
        let placed = copies_payload(&payload, &input(3, true));
        let f = placed.fragment();
        assert_eq!(f.symbols.len(), 9);
        assert_eq!(f.symbols[0].position.x, 20.32);
        assert!((f.symbols[3].position.x - 40.64).abs() < 1e-9);
        assert!((f.symbols[6].position.x - 60.96).abs() < 1e-9);
        let ids: std::collections::HashSet<Uuid> = f.symbols.iter().map(|s| s.uuid).collect();
        assert_eq!(ids.len(), 9, "no two copies share an id");
        let texts: Vec<&str> = f.labels.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "FB_2",
                "VOUT_3V3_2",
                "GND",
                "FB_3",
                "VOUT_3V3_3",
                "GND",
                "FB_4",
                "VOUT_3V3_4",
                "GND"
            ]
        );
    }

    /// #2730: a copied harness travels with its parts, once per copy.
    #[test]
    fn an_array_gives_every_copy_its_own_shifted_harness() {
        let mut source = fragment();
        let connector = HarnessConnector {
            uuid: Uuid::from_u128(0xA1),
            harness_type: "SPI_BUS".into(),
            position: Point::new(0.0, 0.0),
            width: 10.16,
            entry_pitch: 2.54,
            rotation: 0.0,
            entries: vec![HarnessEntry {
                uuid: Uuid::from_u128(0xB1),
                name: "SCK".into(),
            }],
            locked: false,
        };
        let tip = connector.tip_point();
        source.harness.wires.push(HarnessWire {
            uuid: Uuid::from_u128(0xC1),
            start: tip,
            end: Point::new(tip.x + 5.0, tip.y),
            locked: false,
        });
        source.harness.connectors.push(connector);
        let step = PasteInput {
            dx: 10.0,
            ..input(3, false)
        };
        let placed = copies_payload(&PlacementPayload::new(source), &step);
        let harness = &placed.fragment().harness;
        let xs: Vec<f64> = harness.connectors.iter().map(|c| c.position.x).collect();
        assert_eq!(xs, [10.0, 20.0, 30.0]);
        assert_eq!(harness.wires.len(), 3);
        let ids: std::collections::HashSet<Uuid> = harness
            .connectors
            .iter()
            .flat_map(|c| std::iter::once(c.uuid).chain(c.entries.iter().map(|e| e.uuid)))
            .chain(harness.wires.iter().map(|w| w.uuid))
            .collect();
        assert_eq!(ids.len(), 9, "no two harness objects share an id");
    }

    fn in_use(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn a_rewritten_name_already_in_use_is_named_once() {
        let nets = copied_nets(&fragment());
        let taken = taken_names(&nets, &input(3, true), &in_use(&["FB_3", "OTHER"]));
        assert_eq!(taken, ["FB_3"]);
        assert!(taken_names(&nets, &input(3, true), &in_use(&["OTHER"])).is_empty());
    }

    #[test]
    fn keeping_the_nets_takes_no_name() {
        let nets = copied_nets(&fragment());
        assert!(taken_names(&nets, &input(1, false), &in_use(&["FB"])).is_empty());
    }

    #[test]
    fn two_rewritten_nets_landing_on_one_name_are_named() {
        let nets = vec![("X".to_string(), false), ("X1".to_string(), false)];
        let clash = PasteInput {
            rewrite: Some(("${Net}${n}".to_string(), 1)),
            ..input(11, false)
        };
        // X at n = 11 and X1 at n = 1 both read X11.
        assert_eq!(taken_names(&nets, &clash, &in_use(&[])), ["X11"]);
    }

    #[test]
    fn a_rewritten_name_equal_to_a_copied_power_net_is_named() {
        let nets = vec![("FB".to_string(), false), ("VCC".to_string(), true)];
        let onto_power = PasteInput {
            rewrite: Some(("VCC".to_string(), 1)),
            ..input(1, false)
        };
        assert_eq!(taken_names(&nets, &onto_power, &in_use(&[])), ["VCC"]);
    }

    #[test]
    fn the_pattern_must_keep_copies_and_nets_apart() {
        assert!(pattern_separates("${Net}_${n}", 3, 2));
        assert!(!pattern_separates("${n}", 1, 2), "two nets, one name");
        assert!(pattern_separates("${n}", 3, 1));
        assert!(!pattern_separates("${Net}_A", 3, 1), "copies share a name");
        assert!(pattern_separates("${Net}_A", 1, 2));
        assert!(pattern_separates("${Net}_A", 3, 0), "nothing is rewritten");
        assert!(!pattern_separates("plain", 1, 1));
    }

    #[test]
    fn the_mapping_lists_every_copy() {
        let source = fragment();
        let mut numbered = copies_payload(&PlacementPayload::new(source.clone()), &input(2, false))
            .fragment()
            .clone();
        for (s, new) in numbered
            .symbols
            .iter_mut()
            .zip(["R6", "R7", "C7", "R8", "R9", "C9"])
        {
            s.reference = new.into();
        }
        let mapping = designator_mapping(&source, &numbered, 2);
        assert_eq!(
            mapping_text(&mapping, 2),
            "R4 \u{2192} R6, R8; R5 \u{2192} R7, R9; C5 \u{2192} C7, C9"
        );
        let one = designator_mapping(&source, &numbered, 1);
        assert_eq!(
            mapping_text(&one, 1),
            "R4 \u{2192} R6, R5 \u{2192} R7, C5 \u{2192} C7"
        );
    }

    #[test]
    fn the_drag_snaps_to_the_grid() {
        assert!((snapped(20.0) - 20.32).abs() < 1e-9);
        assert_eq!(parse_mm(" 5.08 "), Some(5.08));
        assert_eq!(parse_mm("abc"), None);
    }
}
