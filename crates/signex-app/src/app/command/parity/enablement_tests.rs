//! Enablement pinned per catalog id over the probe's sampled contexts
//! (ADR-0006 slice 7): `testdata/enablement.golden.tsv` was written before
//! `Needs` was split into Core and Ui, so the split keeps every answer.

use super::super::CommandContext;
use super::super::enablement::is_enabled;
use super::inputs::{FACTS, facts_read_by, samples};
use super::needs::{self, NeedReads};
use crate::keymap::{AppCommandId, all_command_ids};

const GOLDEN: &str = include_str!("testdata/enablement.golden.tsv");

/// FNV-1a over the enablement bits of `command` in every sampled context.
fn enablement_hash(command: &AppCommandId) -> u64 {
    samples().fold(0xcbf2_9ce4_8422_2325, |hash, ctx| {
        let bit = u64::from(is_enabled(command, &ctx));
        (hash ^ bit).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn table_text() -> String {
    let mut ids: Vec<&str> = all_command_ids().collect();
    ids.sort_unstable();
    let mut text = String::from("# id\tgate\tenablement over the sampled contexts (FNV-1a)\n");
    for id in ids {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        let gate = needs::needs(&command).map_or_else(|| "-".to_string(), |g| format!("{g:?}"));
        let hash = enablement_hash(&command);
        text.push_str(&format!("{id}\t{gate}\t{hash:016x}\n"));
    }
    text
}

/// Every fact a gate of the catalog reads lands in exactly one half of the
/// context, so a `CoreOnly` need cannot read a window fact and none is lost.
#[test]
fn every_fact_a_gate_reads_is_in_exactly_one_half_of_the_context() {
    let mut read = std::collections::BTreeSet::new();
    for id in all_command_ids() {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        if let Some(gate) = needs::needs(&command) {
            read.extend(facts_read_by(|ctx| gate.is_met(ctx)));
        }
    }
    let base = CommandContext::default();
    for (name, _, set) in FACTS.iter().filter(|(name, _, _)| read.contains(name)) {
        let mut ctx = base;
        set(&mut ctx, true);
        let in_core = ctx.core() != base.core();
        let in_ui = ctx.ui() != base.ui();
        assert!(in_core != in_ui, "`{name}`: core {in_core}, ui {in_ui}");
    }
}

/// A `CoreOnly` need reads no fact of the window's half.
#[test]
fn a_core_need_reads_only_core_facts() {
    for id in all_command_ids() {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        let Some(gate) = needs::needs(&command).filter(|g| g.reads() == NeedReads::CoreOnly) else {
            continue;
        };
        for name in facts_read_by(|ctx| gate.is_met(ctx)) {
            let (_, _, set) = FACTS
                .iter()
                .find(|(fact, _, _)| *fact == name)
                .expect("fact");
            let mut ctx = CommandContext::default();
            set(&mut ctx, true);
            assert_eq!(
                ctx.ui(),
                CommandContext::default().ui(),
                "{gate:?} reads `{name}`"
            );
        }
    }
}

#[test]
fn enablement_of_every_id_matches_the_golden_from_before_the_split() {
    let got = table_text();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let path = format!(
            "{}/src/app/command/parity/testdata/enablement.golden.tsv",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(&path, &got).expect("write golden");
        return;
    }
    let differ: Vec<&str> = GOLDEN
        .lines()
        .zip(got.lines())
        .filter(|(want, live)| want != live)
        .map(|(want, _)| want)
        .collect();
    assert!(
        differ.is_empty() && GOLDEN.lines().count() == got.lines().count(),
        "enablement changed for:\n{}",
        differ.join("\n")
    );
}
