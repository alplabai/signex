//! The rules as a live checklist (#2398): what the last run found per
//! written rule, and the board objects a rule applies to (its scope
//! preview).

use std::collections::BTreeMap;

use signex_types::pcb::PcbBoard;

use pcb_geom::{ItemRef, collect_copper};

use crate::user_rules::{CopperObject, LimitKind};
use crate::{DrcReport, DrcRules};

/// What one run found for one written rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuleResult {
    pub violations: usize,
    /// The furthest past its limit: `(measured, required)` in nm.
    pub worst: Option<(i64, i64)>,
}

impl RuleResult {
    /// How far the worst finding is past its limit, negative; `None` with
    /// no finding.
    pub fn margin_nm(&self) -> Option<i64> {
        self.worst
            .map(|(measured, required)| -(measured - required).abs())
    }
}

impl DrcReport {
    /// The findings of each written rule, by rule id (`user::<name>`).
    pub fn rule_results(&self) -> BTreeMap<String, RuleResult> {
        let mut out: BTreeMap<String, RuleResult> = BTreeMap::new();
        for v in &self.violations {
            let Some(rule) = &v.rule else {
                continue;
            };
            let entry = out.entry(rule.clone()).or_default();
            entry.violations += 1;
            let past = (v.measured_nm - v.required_nm).abs();
            if entry.worst.is_none_or(|(m, r)| past > (m - r).abs()) {
                entry.worst = Some((v.measured_nm, v.required_nm));
            }
        }
        out
    }
}

impl DrcRules {
    /// The first net of `board` written rule `name` takes in: the net a
    /// "Why this value?" sample is shown for.
    pub fn first_net_of(&self, board: &PcbBoard, name: &str) -> Option<u32> {
        let scoped = self.user.iter().find(|s| s.rule.name == name)?;
        board
            .nets
            .iter()
            .map(|n| n.number)
            .find(|&net| scoped.covers_net(net))
    }

    /// [`Self::first_net_of`], and for a class-pair rule (#2398) the first
    /// net across from it, so "Why this value?" resolves the pair.
    pub fn first_pair_of(&self, board: &PcbBoard, name: &str) -> Option<(u32, Option<u32>)> {
        let scoped = self.user.iter().find(|s| s.rule.name == name)?;
        let nets = || board.nets.iter().map(|n| n.number);
        if !scoped.is_pair_scope() {
            return nets()
                .find(|&net| scoped.covers_net(net))
                .map(|net| (net, None));
        }
        nets().find_map(|net| {
            nets()
                .find(|&other| scoped.covers_partner(net, Some(other)))
                .map(|other| (net, Some(other)))
        })
    }

    /// The copper items written rule `name` applies to on `board`: tracks
    /// for a width rule, vias for a via rule, and the tracks, pads and vias
    /// of its object pair (all three without one) for a clearance rule,
    /// on its nets and layers. `None` when no rule has that name.
    pub fn scope_items(&self, board: &PcbBoard, name: &str) -> Option<Vec<ItemRef>> {
        let scoped = self.user.iter().find(|s| s.rule.name == name)?;
        let rule = &scoped.rule;
        let wants = |object: CopperObject| {
            let clearance = rule.limit(LimitKind::MinClearance).is_some()
                && rule.pair.is_none_or(|pair| pair.contains(&object));
            let width = object == CopperObject::Track
                && (rule.limit(LimitKind::MinTrackWidth).is_some()
                    || rule.limit(LimitKind::MaxTrackWidth).is_some());
            let via = object == CopperObject::Via
                && (rule.limit(LimitKind::MinViaDiameter).is_some()
                    || rule.limit(LimitKind::MinViaDrill).is_some());
            clearance || width || via
        };
        let mut items: Vec<ItemRef> = collect_copper(board)
            .into_iter()
            .filter(|c| CopperObject::of(&c.item).is_some_and(wants))
            .filter(|c| scoped.covers(c.net, self.on_board(c.layers)))
            .map(|c| c.item)
            .collect();
        items.sort_unstable();
        items.dedup();
        Some(items)
    }
}
