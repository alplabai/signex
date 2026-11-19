//! What a Smart Paste places (#155), as pure functions of the dialog's
//! choices: the copies and where each sits, the net names they carry, and
//! the After paste table. Power nets are never rewritten.

use signex_engine::PlacementPayload;
use signex_types::schematic::{LabelType, SchematicSheet};
use uuid::Uuid;

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
    /// As copied; annotate later.
    Keep,
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
    /// The pattern lacks `${Net}` and `${n}` both, so copies would not differ.
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

/// `fragment` with fresh ids for every object, so copies placed side by
/// side in one payload never share an id.
fn with_fresh_ids(mut fragment: SchematicSheet) -> SchematicSheet {
    let fresh = Uuid::now_v7;
    fragment.symbols.iter_mut().for_each(|o| o.uuid = fresh());
    fragment.wires.iter_mut().for_each(|o| o.uuid = fresh());
    fragment.buses.iter_mut().for_each(|o| o.uuid = fresh());
    fragment
        .bus_entries
        .iter_mut()
        .for_each(|o| o.uuid = fresh());
    fragment.junctions.iter_mut().for_each(|o| o.uuid = fresh());
    fragment
        .no_connects
        .iter_mut()
        .for_each(|o| o.uuid = fresh());
    fragment
        .no_erc_directives
        .iter_mut()
        .for_each(|o| o.uuid = fresh());
    fragment.labels.iter_mut().for_each(|o| o.uuid = fresh());
    fragment
        .text_notes
        .iter_mut()
        .for_each(|o| o.uuid = fresh());
    fragment
        .drawings
        .iter_mut()
        .for_each(|o| o.set_uuid(fresh()));
    fragment
}

/// The payload a Smart Paste puts on the cursor: every copy at its place
/// from the anchor (copy `k` at `k` times the offset), its net labels
/// rewritten, the library bodies once. Designators are given at the click.
#[must_use]
pub fn copies_payload(payload: &PlacementPayload, input: &PasteInput) -> PlacementPayload {
    let source = payload.fragment();
    // The bodies and sheet settings once; the objects come from the copies.
    let mut merged = source.clone();
    merged.symbols.clear();
    merged.wires.clear();
    merged.buses.clear();
    merged.bus_entries.clear();
    merged.junctions.clear();
    merged.no_connects.clear();
    merged.no_erc_directives.clear();
    merged.labels.clear();
    merged.text_notes.clear();
    merged.drawings.clear();
    for k in 1..=input.copies {
        let step = f64::from(k);
        let mut copy = PlacementPayload::new(source.clone())
            .translated(input.dx * step, input.dy * step)
            .fragment()
            .clone();
        if let Some((pattern, start)) = &input.rewrite {
            let n = start + i64::from(k) - 1;
            for label in copy.labels.iter_mut() {
                if !is_power_label(label.label_type) {
                    label.text = rewritten(pattern, &label.text, n);
                }
            }
        }
        let copy = if k == 1 { copy } else { with_fresh_ids(copy) };
        merged.symbols.extend(copy.symbols);
        merged.wires.extend(copy.wires);
        merged.buses.extend(copy.buses);
        merged.bus_entries.extend(copy.bus_entries);
        merged.junctions.extend(copy.junctions);
        merged.no_connects.extend(copy.no_connects);
        merged.no_erc_directives.extend(copy.no_erc_directives);
        merged.labels.extend(copy.labels);
        merged.text_notes.extend(copy.text_notes);
        merged.drawings.extend(copy.drawings);
    }
    PlacementPayload::new(merged)
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
    use signex_types::schematic::{Label, Point, Symbol};

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
