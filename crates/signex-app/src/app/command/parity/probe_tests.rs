//! The probe behind the gate-derived inputs, checked against an exhaustive
//! enumeration of the facts each gate's source names, so a fact the sampled
//! probe misses fails a test whatever the probe found.
//!
//! The source scan reads `ctx.<field>` and `helper(ctx)` only. It does not
//! parse `helper(ctx, x)`, `&ctx`, a multi-variant `Self::A | Self::B` arm,
//! or a helper defined outside needs.rs and enablement.rs; a gate written
//! that way is caught by `the_source_scan_names_every_fact_the_probe_sees`
//! when the probe still sees the fact.

use std::collections::BTreeSet;

use super::super::{CommandContext, enablement, needs};
use super::inputs::{FACTS, Input, Set, decides, facts_read_by, read_by, samples};
use crate::keymap::{AppCommandId, all_command_ids};

const NEEDS_SOURCE: &str = include_str!("../needs.rs");
const ENABLEMENT_SOURCE: &str = include_str!("../enablement.rs");
const THIS_SOURCE: &str = include_str!("probe_tests.rs");

/// A gate's answer for a context.
type GateFn = Box<dyn Fn(&CommandContext) -> bool>;

/// A gate the table probes, named.
type Gate = (String, GateFn);

/// Every gate the table probes, with the facts its source names: each
/// `needs()` gate, and the locked-selection refusal, which depends on the
/// id only through its gate.
fn every_gate() -> Vec<(Gate, BTreeSet<String>)> {
    let refusal_facts = facts_named(
        fn_body(ENABLEMENT_SOURCE, "explains_refusal_in"),
        ENABLEMENT_SOURCE,
    );
    let mut gates: Vec<(Gate, BTreeSet<String>)> = Vec::new();
    for id in all_command_ids() {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        let gate = needs::needs(&command);
        let name = format!("{gate:?}");
        if gates.iter().any(|((known, _), _)| *known == name) {
            continue;
        }
        let (needs_gate, named): (GateFn, _) = match gate {
            Some(gate) => (
                Box::new(move |ctx| gate.is_met(ctx)),
                needs_facts(&format!("{gate:?}")),
            ),
            None => (Box::new(|_| true), BTreeSet::new()),
        };
        gates.push(((name, needs_gate), named));
        gates.push((
            (
                format!("explains_refusal_in with {gate:?}"),
                Box::new(move |ctx| enablement::explains_refusal_in(&command, ctx)),
            ),
            refusal_facts.clone(),
        ));
    }
    gates
}

/// The facts the `is_met` arms of `Needs::variant` name in `needs.rs`.
fn needs_facts(variant: &str) -> BTreeSet<String> {
    let start = NEEDS_SOURCE.find("fn is_met(").expect("is_met");
    let end = NEEDS_SOURCE[start..].find("fn reason(").expect("reason") + start;
    let mut arms = String::new();
    let mut in_arm = false;
    for line in NEEDS_SOURCE[start..end].lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Self::") {
            let name: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            in_arm = name == variant;
        }
        if in_arm && !line.starts_with("//") {
            arms.push_str(line);
            arms.push('\n');
        }
    }
    facts_named(&arms, NEEDS_SOURCE)
}

/// The body of `fn name` in `source`, up to the first closing brace at the
/// start of a line.
fn fn_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("fn {name} in source"));
    let end = source[start..]
        .find("\n}")
        .map_or(source.len(), |at| at + start);
    &source[start..end]
}

/// The facts `ctx.<field>` (or `core.<field>`, the core half a Ui need
/// reads) names in `body`, with those of every helper `name(ctx)` it calls
/// whose body is in `source`.
fn facts_named(body: &str, source: &str) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    for line in body.lines().filter(|line| !line.trim().starts_with("//")) {
        let reads = ["ctx.", "core."].into_iter().flat_map(|prefix| {
            line.match_indices(prefix)
                .map(move |(at, _)| at + prefix.len())
        });
        for at in reads {
            let path: String = line[at..]
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '_' || *c == '.')
                .collect();
            let mut parts = path.split('.');
            let first = parts.next().unwrap_or_default();
            let fact = match first {
                "pcb" => format!("pcb.{}", parts.next().unwrap_or_default()),
                _ => first.to_string(),
            };
            named.insert(fact);
        }
        for (at, _) in line.match_indices("(ctx)") {
            let helper: String = line[..at]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if source.contains(&format!("fn {helper}(")) && !body.contains(&format!("fn {helper}("))
            {
                named.extend(facts_named(fn_body(source, &helper), source));
            }
        }
    }
    named
}

/// The setter of the fact `name`.
fn setter(name: &str) -> Set {
    FACTS
        .iter()
        .find(|(fact, _, _)| *fact == name)
        .map(|(_, _, set)| *set)
        .unwrap_or_else(|| panic!("`{name}` is not a fact the probe can set"))
}

/// The facts among `named` that decide `gate`, by every assignment of
/// `named` (the others left at their defaults, which `gate` does not read),
/// that the probe did not find.
fn missed_by_the_probe(
    gate: &dyn Fn(&CommandContext) -> bool,
    named: &BTreeSet<String>,
) -> BTreeSet<String> {
    let named: Vec<(&str, Set)> = named
        .iter()
        .map(|name| (name.as_str(), setter(name)))
        .collect();
    let mut read = BTreeSet::new();
    for bits in 0..1u32 << named.len() {
        let mut ctx = CommandContext::default();
        for (index, (_, set)) in named.iter().enumerate() {
            set(&mut ctx, bits >> index & 1 == 1);
        }
        for (name, set) in &named {
            if decides(gate, ctx, *set) {
                read.insert(name.to_string());
            }
        }
    }
    let found: BTreeSet<String> = facts_read_by(gate)
        .into_iter()
        .map(str::to_string)
        .collect();
    read.difference(&found).cloned().collect()
}

/// Every fact a gate's source names and that decides the gate, the probe
/// finds.
#[test]
fn the_probe_finds_every_fact_a_gate_reads() {
    // The source scan is live: Turn names its board and sheet facts, and
    // the sheet's through `editable_selection(ctx)`.
    let turn = needs_facts("Turn");
    for fact in [
        "pcb.shown",
        "pcb.routing",
        "turn_in_hand",
        "selection_all_locked",
    ] {
        assert!(turn.contains(fact), "Turn names {fact}: {turn:?}");
    }
    for ((name, gate), named) in every_gate() {
        let missed = missed_by_the_probe(&gate, &named);
        assert!(
            missed.is_empty(),
            "the probe misses {missed:?} of {name}; raise SAMPLES"
        );
    }
}

/// The scan reads every gate: each `Needs` gate but `Planned` names some
/// fact, and every fact the probe sees in a gate is one the scan named, so a
/// source shape the scan does not parse fails here instead of passing.
#[test]
fn the_source_scan_names_every_fact_the_probe_sees() {
    for ((name, gate), named) in every_gate() {
        let is_needs = name.starts_with("Some(") && !name.starts_with("explains");
        if is_needs && name != "Some(Planned)" {
            assert!(!named.is_empty(), "the scan names no fact of {name}");
        }
        let unnamed: Vec<&str> = facts_read_by(&gate)
            .into_iter()
            .filter(|fact| !named.contains(*fact))
            .collect();
        assert!(
            unnamed.is_empty(),
            "the probe sees {unnamed:?} in {name}, which the scan does not name"
        );
    }
}

/// A deep conjunction: `has_schematic || (eleven facts all set)`. Each of
/// the eleven decides it only when the other ten hold and `has_schematic`
/// does not, about 1 sample in 4096.
fn deep(ctx: &CommandContext) -> bool {
    ctx.has_schematic
        || (ctx.has_pcb
            && ctx.pcb_view_turned
            && ctx.undocked_has_canvas
            && ctx.has_sheets
            && ctx.has_selection
            && ctx.drawing
            && ctx.can_undo
            && ctx.can_redo
            && ctx.is_tiled
            && ctx.pcb.any
            && ctx.pcb.pasteable)
}

/// The negative control: the sampled probe cannot find the deep facts, and
/// the exhaustive check names them.
#[test]
fn the_exhaustive_check_catches_a_deep_conjunction_the_probe_misses() {
    let named = facts_named(fn_body(THIS_SOURCE, "deep"), THIS_SOURCE);
    assert_eq!(named.len(), 12, "the control names twelve facts: {named:?}");
    let missed = missed_by_the_probe(&deep, &named);
    assert!(
        !missed.is_empty(),
        "the probe found all eleven deep facts; the control no longer controls"
    );
    assert!(!missed.contains("has_schematic"));
}

/// The probe finds exactly the facts a gate reads.
#[test]
fn the_probe_reads_what_a_gate_reads() {
    assert_eq!(
        read_by(|ctx| ctx.has_selection && ctx.pcb.pasteable),
        BTreeSet::from([Input::Selection, Input::Clipboard])
    );
    assert_eq!(
        read_by(|ctx| ctx.drawing || ctx.can_tile_windows),
        BTreeSet::from([Input::InProgressEdit, Input::Window])
    );
    assert!(read_by(|_| true).is_empty());
    // A conjunction of six facts: each decides it only when the other five hold.
    assert_eq!(
        facts_read_by(|ctx| ctx.has_schematic
            && ctx.has_selection
            && ctx.drawing
            && ctx.is_tiled
            && ctx.pcb.pasteable
            && ctx.pcb.any),
        BTreeSet::from([
            "has_schematic",
            "has_selection",
            "drawing",
            "is_tiled",
            "pcb.pasteable",
            "pcb.any",
        ])
    );
}

/// The probe sets `pcb.footprints` to 0 or 2 only, which is sound while no
/// gate tells 1 from 0 or 2.
#[test]
fn no_gate_reads_the_footprint_count() {
    for ((name, gate), _) in every_gate() {
        for ctx in samples() {
            let answers: Vec<bool> = [0, 1, 2]
                .into_iter()
                .map(|footprints| {
                    let mut ctx = ctx;
                    ctx.pcb.footprints = footprints;
                    gate(&ctx)
                })
                .collect();
            assert!(
                answers.iter().all(|answer| *answer == answers[0]),
                "{name} reads the footprint count; give the probe that fact"
            );
        }
    }
}
