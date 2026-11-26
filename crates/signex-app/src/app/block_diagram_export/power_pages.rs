//! The Power tree and Budget table pages of a block diagram PDF (mockup row
//! 171, "Export PDF…" Pages): the project's power tree, read by the run
//! behind `query_power_tree`, as rows. The Power tree page is an outline,
//! each rail followed by its sources and loads; the Budget table has one row
//! per rail with its limit, load and headroom. The words come in from the
//! catalog, worded when the export was planned.

use signex_net::power_tree::{PowerRail, PowerTopology};
use signex_output::{BlockDiagramTable, BlockDiagramTableRow};

use crate::app::core::block_diagram_panes::PowerTreeRead;

/// The pages' words in the user's language.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PowerPageWords {
    pub power_heading: String,
    pub budget_heading: String,
    /// Item, role, voltage, current, limit, share.
    pub power_columns: Vec<String>,
    /// Rail, voltage, source, limit, load, headroom.
    pub budget_columns: Vec<String>,
    pub rail: String,
    pub source: String,
    pub converter: String,
    pub load: String,
    /// A value not known.
    pub unknown: String,
}

fn amps(value: Option<f64>, words: &PowerPageWords) -> String {
    value.map_or_else(|| words.unknown.clone(), |a| format!("{a:.2} A"))
}

fn volts(rail: &PowerRail, words: &PowerPageWords) -> String {
    rail.voltage
        .map_or_else(|| words.unknown.clone(), |v| format!("{:.2} V", v.volts))
}

fn percent(share: Option<f64>, words: &PowerPageWords) -> String {
    share.map_or_else(|| words.unknown.clone(), |s| format!("{:.0} %", s * 100.0))
}

fn part_name(topology: &PowerTopology, at: usize) -> String {
    topology.parts.get(at).map_or_else(String::new, |part| {
        format!("{} {}", part.designator, part.value)
            .trim()
            .to_string()
    })
}

fn row(cells: Vec<String>, indent: u8, strong: bool) -> BlockDiagramTableRow {
    BlockDiagramTableRow {
        cells,
        indent,
        strong,
    }
}

/// The Power tree page: each rail, then its sources and its loads.
pub fn power_tree_table(read: &PowerTreeRead, words: &PowerPageWords) -> BlockDiagramTable {
    let mut rows = Vec::new();
    for rail in &read.tree.rails {
        rows.push(row(
            vec![
                rail.net.clone(),
                words.rail.clone(),
                volts(rail, words),
                amps(Some(rail.total_a), words),
                amps(rail.limit_a(), words),
                String::new(),
            ],
            0,
            true,
        ));
        for source in &rail.sources {
            let role = if source.is_converter {
                &words.converter
            } else {
                &words.source
            };
            rows.push(row(
                vec![
                    part_name(&read.topology, source.part),
                    role.clone(),
                    String::new(),
                    String::new(),
                    amps(source.limit_a, words),
                    String::new(),
                ],
                1,
                false,
            ));
        }
        for load in &rail.loads {
            let share = load
                .current_a
                .filter(|_| rail.total_a > 0.0)
                .map(|current| current / rail.total_a);
            rows.push(row(
                vec![
                    part_name(&read.topology, load.part),
                    words.load.clone(),
                    String::new(),
                    amps(load.current_a, words),
                    String::new(),
                    percent(share, words),
                ],
                1,
                false,
            ));
        }
    }
    BlockDiagramTable {
        heading: words.power_heading.clone(),
        columns: words.power_columns.clone(),
        rows,
    }
}

/// The Budget table page: one row per rail.
pub fn budget_table(read: &PowerTreeRead, words: &PowerPageWords) -> BlockDiagramTable {
    let rows = read
        .tree
        .rails
        .iter()
        .map(|rail| {
            let source = match rail.sources.as_slice() {
                [one] => part_name(&read.topology, one.part),
                _ => words.unknown.clone(),
            };
            let headroom = rail
                .limit_a()
                .filter(|limit| *limit > 0.0)
                .and_then(|limit| rail.headroom_a().map(|left| left / limit));
            row(
                vec![
                    rail.net.clone(),
                    volts(rail, words),
                    source,
                    amps(rail.limit_a(), words),
                    amps(Some(rail.total_a), words),
                    percent(headroom, words),
                ],
                0,
                false,
            )
        })
        .collect();
    BlockDiagramTable {
        heading: words.budget_heading.clone(),
        columns: words.budget_columns.clone(),
        rows,
    }
}
