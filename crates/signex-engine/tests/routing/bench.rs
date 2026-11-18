//! The router's numbers (#2483), per board: how many rounds a shove takes,
//! how long one routed leg takes from click to landed step, and the DRC
//! delta of every route (what the design rule check finds after it that it
//! did not find before must name the route's own copper).

use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use pcb_geom::collect_copper;
use serde_json::Map;
use signex_drc::DrcRules;
use signex_drc::findings::check_report;
use signex_engine::pcb::route::{RouteDraft, RouteMode};
use signex_engine::pcb::{BoardCommand, BoardEngine, BoardItemId};
use signex_types::check::CheckReport;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::fixtures::Fixture;
use crate::rng::Rng;
use crate::session::{Routed, ShoveTry, plan, route, rules_of, shove_rounds};

/// Rounds a shove may take on most actions.
pub const ROUNDS_LIMIT: usize = 8;
/// The share of shove actions that must stay within [`ROUNDS_LIMIT`].
pub const ROUNDS_SHARE: f64 = 0.95;
/// Median and 95th-percentile latency on the median board.
pub const MEDIAN_LIMIT: Duration = Duration::from_millis(16);
pub const P95_LIMIT: Duration = Duration::from_millis(33);

/// A board's measurements.
#[derive(Debug, Default)]
pub struct BoardBench {
    pub name: String,
    /// Netted copper items: what ranks the boards by size.
    pub copper: usize,
    pub landed: usize,
    pub refused: usize,
    /// How every shove action that tried a push ended.
    pub shoves: Vec<ShoveTry>,
    /// One per routed or refused leg.
    pub latencies: Vec<Duration>,
    /// Actions that broke a promise: a DRC delta off the route, or a route
    /// the router itself refused to land.
    pub breaks: Vec<String>,
}

impl BoardBench {
    /// The share of shove actions that cleared the way within
    /// [`ROUNDS_LIMIT`]; a shove that gave up never counts. 1 with none.
    pub fn converged_share(&self) -> f64 {
        if self.shoves.is_empty() {
            return 1.0;
        }
        let within = self
            .shoves
            .iter()
            .filter(|s| s.converged && s.rounds <= ROUNDS_LIMIT)
            .count();
        within as f64 / self.shoves.len() as f64
    }

    fn gave_up(&self) -> usize {
        self.shoves.iter().filter(|s| !s.converged).count()
    }

    pub fn percentile(&self, share: f64) -> Duration {
        let mut sorted = self.latencies.clone();
        sorted.sort_unstable();
        let Some(last) = sorted.len().checked_sub(1) else {
            return Duration::ZERO;
        };
        sorted[((last as f64) * share).round() as usize]
    }

    pub fn line(&self) -> String {
        format!(
            "{:<24} copper {:>5}  legs {:>4} landed / {:>4} refused  shove {:>4} actions, \
             {:>5.1}% converged within {ROUNDS_LIMIT} rounds, {} gave up, max {}  \
             latency median {:>7.2?} p95 {:>7.2?}  breaks {}",
            self.name,
            self.copper,
            self.landed,
            self.refused,
            self.shoves.len(),
            self.converged_share() * 100.0,
            self.gave_up(),
            self.shoves.iter().map(|s| s.rounds).max().unwrap_or(0),
            self.percentile(0.5),
            self.percentile(0.95),
            self.breaks.len(),
        )
    }
}

/// Route `actions` seeded one-leg actions on `fixture`. Before every other
/// action a random track is deleted (unmeasured), so a fully routed board
/// has ratsnest lines to route.
pub fn bench(fixture: &Fixture, seed: u64, actions: usize) -> BoardBench {
    let rules = rules_of(&fixture.board, &fixture.classes);
    let mut engine = BoardEngine::new(fixture.board.clone(), 1);
    let mut rng = Rng::new(seed);
    let mut next_id = 0u128;
    let mut out = BoardBench {
        name: fixture.name.clone(),
        copper: collect_copper(&fixture.board).len(),
        ..BoardBench::default()
    };
    for index in 0..actions {
        if rng.chance(50) {
            delete_a_track(&mut engine, &mut rng);
        }
        let mode = if rng.chance(50) {
            RouteMode::Shove
        } else {
            RouteMode::Walkaround
        };
        let Some(plan) = plan(engine.board(), &mut rng, mode, 1) else {
            continue;
        };
        if mode == RouteMode::Shove {
            out.shoves.extend(shove_rounds(
                engine.board(),
                &rules,
                &fixture.classes,
                &plan,
            ));
        }
        let before = drc(engine.board(), &rules, &fixture.name);
        let mut ids = || {
            next_id += 1;
            Uuid::from_u128(crate::soak::SOAK_IDS | next_id)
        };
        let started = Instant::now();
        let routed = route(&mut engine, &rules, &fixture.classes, &plan, &mut ids);
        out.latencies.push(started.elapsed());
        match routed {
            Ok(Routed::Landed(draft)) => {
                out.landed += 1;
                let after = drc(engine.board(), &rules, &fixture.name);
                if let Err(e) = drc_delta(&before, &after, &draft) {
                    out.breaks
                        .push(format!("seed {seed:#x} action {index}: {e}"));
                }
            }
            Ok(Routed::Refused) => out.refused += 1,
            Err(e) => out
                .breaks
                .push(format!("seed {seed:#x} action {index}: {e}")),
        }
    }
    out
}

fn delete_a_track(engine: &mut BoardEngine, rng: &mut Rng) {
    let ids: Vec<Uuid> = engine
        .board()
        .segments
        .iter()
        .filter(|s| !s.locked)
        .map(|s| s.uuid)
        .collect();
    if let Some(&id) = rng.pick(&ids) {
        engine
            .execute(BoardCommand::DeleteItems {
                items: vec![BoardItemId::Segment(id)],
            })
            .expect("an unlocked track deletes");
    }
}

/// The design rule check of `board` as a check report.
fn drc(board: &PcbBoard, rules: &DrcRules, name: &str) -> CheckReport {
    let report = signex_drc::run(board, rules);
    let names: BTreeMap<u32, String> = board
        .nets
        .iter()
        .map(|n| (n.number, n.name.clone()))
        .collect();
    check_report(&report, board, &names, name, Vec::new(), Map::new())
}

/// The findings `after` holds that `before` did not, other than unrouted
/// links (routing changes those by design), must each name a track or via
/// the route added: a route has no side effects. A finding is the same
/// finding when its rule and items are; its measured gap may move.
pub fn drc_delta(
    before: &CheckReport,
    after: &CheckReport,
    draft: &RouteDraft,
) -> Result<(), String> {
    let had: HashSet<(&str, &[String])> = before
        .findings
        .iter()
        .map(|f| (f.rule.as_str(), f.items.as_slice()))
        .collect();
    let own: HashSet<String> = draft
        .tracks
        .iter()
        .map(|t| t.uuid)
        .chain(draft.vias.iter().map(|v| v.uuid))
        .map(|id| id.to_string())
        .collect();
    let stray: Vec<String> = after
        .findings
        .iter()
        .filter(|f| f.rule != "builtin::unconnected")
        .filter(|f| !had.contains(&(f.rule.as_str(), f.items.as_slice())))
        .filter(|f| !f.items.iter().any(|item| own.contains(item)))
        .map(|f| f.message.clone())
        .collect();
    if stray.is_empty() {
        Ok(())
    } else {
        Err(format!("new findings off the route's copper: {stray:?}"))
    }
}
