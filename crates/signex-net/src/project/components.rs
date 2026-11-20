//! The netlist by component (#1274): every part of the stitched project with
//! the net on each of its connected pins, and the pins on no net. Read from
//! the same stitch as the rest of [`NetIndex`]; nothing here re-derives a net.

use std::collections::{HashMap, HashSet};

use signex_types::designator::compare_references;
use signex_types::part_fields::evaluated_value;
use signex_types::schematic::{InstancePath, instance_pins};
use signex_types::variables::{Resolver, TextTable};
use uuid::Uuid;

use super::members::NetMemberKind;
use super::net_index::{NetAnchor, NetIndex};
use super::{Occ, SheetKey};
use crate::build::terminal_pin_id;
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
    /// Every pin of the unit, wired or not, in library order.
    pub pins: Vec<PlacedPin>,
}

/// A pin of a placed unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedPin {
    /// What a terminal on this pin carries as its pin: the number, else the
    /// name.
    pub pin: String,
    /// The pin's name; empty when the library gives none.
    pub name: String,
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
    /// The pin's name; empty when the library gives none.
    pub name: String,
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
    /// and is listed in [`unconnected`](Self::unconnected) instead.
    pub pins: Vec<ComponentPin>,
    /// The pins on no net, by pin number.
    pub unconnected: Vec<UnconnectedPin>,
}

/// A pin of a part that is on no net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconnectedPin {
    pub pin: String,
    /// The pin's name; empty when the library gives none.
    pub name: String,
    /// The pin on its unit.
    pub anchor: NetAnchor,
}

impl ComponentReport {
    /// Whether the designator names one part; a part not annotated yet is
    /// one symbol, known only by its [`anchor`](Self::anchor).
    pub fn is_annotated(&self) -> bool {
        !needs_annotation(&self.designator)
    }

    /// Every pin of the part, wired or not.
    pub fn pin_count(&self) -> usize {
        self.pins.len() + self.unconnected.len()
    }
}

/// Every non-power symbol of `occs`, in walk then document order.
pub(super) fn placed_symbols(occs: &[Occ<'_>]) -> Vec<PlacedSymbol> {
    let mut out = Vec::new();
    for occ in occs {
        // The value as shown: an `=` value evaluated on its sheet (#1986).
        let sheet_variables = TextTable(&occ.sheet.variables);
        let variables: [&dyn Resolver; 1] = [&sheet_variables];
        let symbols = &occ.sheet.symbols;
        for s in symbols.iter().filter(|s| !s.is_power) {
            let pins = occ
                .sheet
                .resolve_lib_symbol(s)
                .map(|lib| {
                    instance_pins(&lib, s)
                        .map(|lp| PlacedPin {
                            pin: terminal_pin_id(lp),
                            name: lp.pin.name.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(PlacedSymbol {
                sheet: occ.key.clone(),
                occurrence: occ.path.clone(),
                id: s.uuid,
                designator: s.designator_in(&occ.path).trim().to_string(),
                value: evaluated_value(symbols, s, &variables, false),
                footprint: s.footprint.clone(),
                pins,
            });
        }
    }
    out
}

/// A placed symbol in one placement.
type PlacementKey<'a> = (&'a SheetKey, &'a InstancePath, Uuid);

/// A pin of a placed symbol in one placement.
type PinKey<'a> = (PlacementKey<'a>, &'a str);

impl NetIndex {
    /// Every part of the project by designator, with the net on each
    /// connected pin and the pins on no net. The units of one designator are
    /// one part; a symbol not annotated yet is a part of its own.
    pub fn components(&self) -> Vec<ComponentReport> {
        let (mut reports, of_symbol) = self.parts_by_designator();
        let names: HashMap<PinKey<'_>, &str> = self
            .placed_symbols()
            .iter()
            .flat_map(|placed| {
                let key = (&placed.sheet, &placed.occurrence, placed.id);
                placed
                    .pins
                    .iter()
                    .map(move |p| ((key, p.pin.as_str()), p.name.as_str()))
            })
            .collect();
        let seen = self.add_connected_pins(&mut reports, &of_symbol, &names);
        for placed in self.placed_symbols() {
            let key = (&placed.sheet, &placed.occurrence, placed.id);
            let Some(&at) = of_symbol.get(&key) else {
                continue;
            };
            let mut listed: HashSet<&str> = HashSet::new();
            for p in &placed.pins {
                if seen.contains(&(key, p.pin.as_str())) || !listed.insert(p.pin.as_str()) {
                    continue;
                }
                reports[at].unconnected.push(UnconnectedPin {
                    pin: p.pin.clone(),
                    name: p.name.clone(),
                    anchor: NetAnchor {
                        sheet: placed.sheet.clone(),
                        id: placed.id,
                        occurrence: placed.occurrence.clone(),
                        pin: Some(p.pin.clone()),
                    },
                });
            }
        }
        for report in &mut reports {
            report.pins.sort_by(|a, b| {
                compare_references(&a.pin, &b.pin).then_with(|| a.net_name.cmp(&b.net_name))
            });
            report
                .unconnected
                .sort_by(|a, b| compare_references(&a.pin, &b.pin));
        }
        reports.sort_by(|a, b| compare_references(&a.designator, &b.designator));
        reports
    }

    /// How many parts [`components`](Self::components) lists, without
    /// reading their pins.
    pub fn component_count(&self) -> usize {
        self.parts_by_designator().0.len()
    }

    /// One report per part, with no pins yet, and the report of each placed
    /// symbol.
    fn parts_by_designator(&self) -> (Vec<ComponentReport>, HashMap<PlacementKey<'_>, usize>) {
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
                        unconnected: Vec::new(),
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
        (reports, of_symbol)
    }

    /// Add each pin on a net to its part, once per placed pin, named from
    /// `names`; returns the pins added.
    fn add_connected_pins<'a>(
        &'a self,
        reports: &mut [ComponentReport],
        of_symbol: &HashMap<PlacementKey<'a>, usize>,
        names: &HashMap<PinKey<'a>, &str>,
    ) -> HashSet<PinKey<'a>> {
        let mut seen: HashSet<PinKey<'a>> = HashSet::new();
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
                let name = names.get(&(key, pin.as_str())).copied().unwrap_or("");
                reports[at].pins.push(ComponentPin {
                    pin: pin.clone(),
                    name: name.to_string(),
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
        seen
    }
}
