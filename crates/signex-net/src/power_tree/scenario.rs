//! The scenario a power tree is read in (mockup row 171's totals line):
//! "Max load", every fitted load at the current it states, the one scenario
//! there is. What the line says: how many of the sources feed a load in it,
//! and the rail with the least headroom against its limit.

use super::tree::PowerTree;

/// The one scenario: every fitted load at its stated current.
pub const MAX_LOAD_SCENARIO: &str = "max_load";

/// The totals of a tree in its scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioSummary {
    /// Sources that are not converters, on any rail.
    pub sources: usize,
    /// Of those, the ones whose rail feeds a load in the scenario; the rest
    /// are alternate sources not in it.
    pub active_sources: usize,
    /// The rail with the least headroom and that headroom's share of its
    /// limit (negative when the limit is exceeded); `None` when no rail has
    /// a limit.
    pub worst_rail: Option<(String, f64)>,
}

/// The totals of `tree` in the Max load scenario.
pub fn scenario_summary(tree: &PowerTree) -> ScenarioSummary {
    let mut sources = 0;
    let mut active_sources = 0;
    let mut worst_rail: Option<(String, f64)> = None;
    for rail in &tree.rails {
        let fed = rail.loads.iter().any(|load| load.current_a != Some(0.0));
        let own = rail.sources.iter().filter(|s| !s.is_converter).count();
        sources += own;
        if fed {
            active_sources += own;
        }
        let share = rail
            .limit_a()
            .filter(|limit| *limit > 0.0)
            .and_then(|limit| rail.headroom_a().map(|left| left / limit));
        if let Some(share) = share
            && worst_rail.as_ref().is_none_or(|(_, worst)| share < *worst)
        {
            worst_rail = Some((rail.net.clone(), share));
        }
    }
    ScenarioSummary {
        sources,
        active_sources,
        worst_rail,
    }
}
