//! The netlist by component (#1274): every part of the stitched project with
//! the net on each of its connected pins. Read from the same stitch as the
//! rest of [`NetIndex`]; nothing here re-derives a net.

use std::collections::{HashMap, HashSet};

use signex_types::designator::compare_references;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::members::NetMemberKind;
use super::net_index::{NetAnchor, NetIndex};
use super::{Occ, SheetKey};
use crate::parts::needs_annotation;

/// One placed, non-power symbol of the walk, once per placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedSymbol {
    pub sheet: SheetKey,
    pub occurrence: InstancePath,
    pub id: Uuid,
    /// The designator at this placement, trimmed.
    pub designator: String,
    pub value: String,
    pub footprint: String,
}

impl PlacedSymbol {
    /// Whether the designator names one part: not blank and holding no `?`.
    pub fn is_annotated(&self) -> bool {
        !needs_annotation(&self.designator)
    }
}

/// A pin of a part and the net it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPin {
    pub pin: String,
    /// The net's position in the index.
    pub net: usize,
    pub net_name: String,
    /// Finds the pin again in a rebuilt index.
    pub anchor: NetAnchor,
}

/// One part with its connected pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentReport {
    pub designator: String,
    /// Value and footprint of the first unit placed.
    pub value: String,
    pub footprint: String,
    /// The first unit placed.
    pub anchor: NetAnchor,
    /// The pins on a net, by pin number. A pin wired to nothing is on no net
    /// and is not listed.
    pub pins: Vec<ComponentPin>,
}

impl ComponentReport {
    /// Whether the designator names one part; a part not annotated yet is
    /// one symbol, known only by its [`anchor`](Self::anchor).
    pub fn is_annotated(&self) -> bool {
        !needs_annotation(&self.designator)
    }
}

/// Every non-power symbol of `occs`, in walk then document order.
pub(super) fn placed_symbols(occs: &[Occ<'_>]) -> Vec<PlacedSymbol> {
    occs.iter()
        .flat_map(|occ| {
            occ.sheet
                .symbols
                .iter()
                .filter(|s| !s.is_power)
                .map(|s| PlacedSymbol {
                    sheet: occ.key.clone(),
                    occurrence: occ.path.clone(),
                    id: s.uuid,
                    designator: s.designator_in(&occ.path).trim().to_string(),
                    value: s.value.clone(),
                    footprint: s.footprint.clone(),
                })
        })
        .collect()
}

/// A placed symbol in one placement.
type PlacementKey<'a> = (&'a SheetKey, &'a InstancePath, Uuid);

impl NetIndex {
    /// Every part of the project by designator, with the net on each
    /// connected pin. The units of one designator are one part; a symbol not
    /// annotated yet is a part of its own.
    pub fn components(&self) -> Vec<ComponentReport> {
        let mut reports: Vec<ComponentReport> = Vec::new();
        let mut by_designator: HashMap<&str, usize> = HashMap::new();
        let mut of_symbol: HashMap<PlacementKey<'_>, usize> = HashMap::new();
        for placed in self.placed_symbols() {
            let known = if placed.is_annotated() {
                by_designator.get(placed.designator.as_str()).copied()
            } else {
                None
            };
            let at = match known {
                Some(at) => at,
                None => {
                    reports.push(ComponentReport {
                        designator: placed.designator.clone(),
                        value: placed.value.clone(),
                        footprint: placed.footprint.clone(),
                        anchor: NetAnchor::new(
                            placed.sheet.clone(),
                            placed.id,
                            placed.occurrence.clone(),
                        ),
                        pins: Vec::new(),
                    });
                    let at = reports.len() - 1;
                    if placed.is_annotated() {
                        by_designator.insert(placed.designator.as_str(), at);
                    }
                    at
                }
            };
            of_symbol.insert((&placed.sheet, &placed.occurrence, placed.id), at);
        }
        let mut seen: HashSet<(PlacementKey<'_>, &str)> = HashSet::new();
        for net in 0..self.net_count() {
            let Some(listed) = self.net(net) else {
                continue;
            };
            for m in &listed.members {
                let NetMemberKind::Pin { pin, .. } = &m.kind else {
                    continue;
                };
                let key = (&m.sheet, &m.occurrence, m.id);
                let Some(&at) = of_symbol.get(&key) else {
                    continue;
                };
                if !seen.insert((key, pin.as_str())) {
                    continue;
                }
                reports[at].pins.push(ComponentPin {
                    pin: pin.clone(),
                    net,
                    net_name: listed.name.clone(),
                    anchor: NetAnchor {
                        sheet: m.sheet.clone(),
                        id: m.id,
                        occurrence: m.occurrence.clone(),
                        pin: Some(pin.clone()),
                    },
                });
            }
        }
        for report in &mut reports {
            report.pins.sort_by(|a, b| {
                compare_references(&a.pin, &b.pin).then_with(|| a.net_name.cmp(&b.net_name))
            });
        }
        reports.sort_by(|a, b| compare_references(&a.designator, &b.designator));
        reports
    }
}
