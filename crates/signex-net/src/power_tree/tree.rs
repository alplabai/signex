//! The power tree (#1313): every rail with its sources, converters and
//! loads, and the current each draws, built from a [`PowerTopology`]. A
//! converter's input current is worked out from what its outputs deliver,
//! so the budget runs up the tree from the loads to the sources. Rules:
//! `docs/POWER_TREE.md`.

use std::collections::{BTreeMap, BTreeSet};

use super::quantity::{
    EFFICIENCY_PARAMETER, LOAD_PARAMETER, RailRole, SUPPLY_PARAMETER, parse_current_a,
    parse_efficiency, rail_parameter, voltage_from_net_name,
};
use super::topology::{PowerPart, PowerTopology};

/// A rail's voltage as the project lists it (its rail list, #2943).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListedRail {
    /// Volts; never 0.
    pub voltage: f64,
    /// Plus or minus this percentage of `voltage`.
    pub tolerance_percent: Option<f64>,
}

/// Where a rail's voltage was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoltageFrom {
    /// The project's rail list.
    Project,
    /// The net name (`VOUT_3V3`).
    NetName,
}

/// A rail's voltage and where it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailVoltage {
    pub volts: f64,
    pub tolerance_percent: Option<f64>,
    pub from: VoltageFrom,
}

/// Where a load's current came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentFrom {
    /// The part's `Rail Load <net>` parameter.
    Parameter,
    /// Worked out from what a converter's outputs deliver.
    Converter,
}

/// A part that supplies a rail.
#[derive(Debug, Clone, PartialEq)]
pub struct RailSource {
    /// The part's place in [`PowerTopology::parts`].
    pub part: usize,
    /// The most current it can supply (`Rail Supply <net>`), in A; `None`
    /// when the parameter is no current.
    pub limit_a: Option<f64>,
    /// Whether the part is a converter fed from another rail.
    pub is_converter: bool,
}

/// A part that draws from a rail.
#[derive(Debug, Clone, PartialEq)]
pub struct RailLoad {
    /// The part's place in [`PowerTopology::parts`].
    pub part: usize,
    /// What it draws, in A; `None` when nothing tells.
    pub current_a: Option<f64>,
    pub from: Option<CurrentFrom>,
}

/// What keeps a rail's or a converter's budget from being told in full.
#[derive(Debug, Clone, PartialEq)]
pub enum PowerProblem {
    /// The rail has no voltage: the project lists none and its name spells
    /// none.
    NoVoltage,
    /// No fitted part supplies the rail.
    NoSource,
    /// More than one part supplies the rail, so its limit is not one part's.
    SeveralSources(usize),
    /// A part on the rail says nothing of the current it draws.
    UnknownLoad { designator: String },
    /// A parameter's text is no current (or no efficiency).
    UnreadParameter {
        designator: String,
        parameter: String,
        text: String,
    },
    /// A converter's input current cannot be worked out: it has no
    /// efficiency, more than one input rail, or an output it does not
    /// supply alone or whose budget is not complete.
    UnknownInput { designator: String },
}

/// One rail of the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerRail {
    pub net: String,
    pub voltage: Option<RailVoltage>,
    pub sources: Vec<RailSource>,
    /// In designator order.
    pub loads: Vec<RailLoad>,
    /// What the loads that tell draw together, in A.
    pub total_a: f64,
    /// Whether every load tells its current, so `total_a` is the whole draw.
    pub is_complete: bool,
    /// The rails the rail's converters are fed from, by name.
    pub fed_from: BTreeSet<String>,
    pub problems: Vec<PowerProblem>,
}

impl PowerRail {
    /// The rail's limit: its one source's, `None` with no source or several.
    pub fn limit_a(&self) -> Option<f64> {
        match self.sources.as_slice() {
            [source] => source.limit_a,
            _ => None,
        }
    }

    /// What is left of the limit, in A; negative when it is exceeded.
    pub fn headroom_a(&self) -> Option<f64> {
        self.limit_a().map(|limit| limit - self.total_a)
    }
}

/// One converter: a part that supplies a rail and is fed from another.
#[derive(Debug, Clone, PartialEq)]
pub struct Converter {
    /// The part's place in [`PowerTopology::parts`].
    pub part: usize,
    pub inputs: BTreeSet<String>,
    pub outputs: BTreeSet<String>,
    /// As a fraction (0.88).
    pub efficiency: Option<f64>,
    /// What its outputs deliver, in W, when every output's budget is told.
    pub output_w: Option<f64>,
    /// What it draws from its input, in W.
    pub input_w: Option<f64>,
}

impl Converter {
    /// What the converter turns into heat, in W.
    pub fn loss_w(&self) -> Option<f64> {
        Some(self.input_w? - self.output_w?)
    }
}

/// The power tree of a project.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PowerTree {
    /// Every rail, by name.
    pub rails: Vec<PowerRail>,
    /// Every converter, in designator order.
    pub converters: Vec<Converter>,
    /// The rails no converter feeds: the tops of the tree, by name.
    pub roots: Vec<String>,
}

impl PowerTree {
    /// The rail named `net`.
    pub fn rail(&self, net: &str) -> Option<&PowerRail> {
        self.rails.iter().find(|rail| rail.net == net)
    }
}

/// What one part says about the rails it touches.
struct PartRails {
    supplies: BTreeMap<String, Result<f64, String>>,
    loads: BTreeMap<String, Result<f64, String>>,
    touches: BTreeSet<String>,
}

impl PartRails {
    fn of(part: &PowerPart, rails: &BTreeSet<String>) -> Self {
        let mut supplies = BTreeMap::new();
        let mut loads = BTreeMap::new();
        for (parameter, text) in &part.parameters {
            let Some((net, role)) = rail_parameter(parameter) else {
                continue;
            };
            if !rails.contains(net) {
                continue;
            }
            let current = parse_current_a(text).ok_or_else(|| text.clone());
            match role {
                RailRole::Supply => supplies.insert(net.to_string(), current),
                RailRole::Load => loads.insert(net.to_string(), current),
            };
        }
        let mut touches: BTreeSet<String> = part.pins_on.intersection(rails).cloned().collect();
        touches.extend(supplies.keys().cloned());
        touches.extend(loads.keys().cloned());
        Self {
            supplies,
            loads,
            touches,
        }
    }

    /// The rails the part is fed from when it supplies another.
    fn inputs(&self) -> BTreeSet<String> {
        self.touches
            .iter()
            .filter(|net| !self.supplies.contains_key(*net))
            .cloned()
            .collect()
    }
}

/// Build the power tree of `topology`. `listed` are the project's own
/// rails with their voltages; a rail it does not list takes the voltage its
/// name spells.
pub fn power_tree(topology: &PowerTopology, listed: &BTreeMap<String, ListedRail>) -> PowerTree {
    let said: Vec<PartRails> = topology
        .parts
        .iter()
        .map(|part| PartRails::of(part, &topology.rails))
        .collect();
    let mut builder = Builder {
        topology,
        said: &said,
        voltages: topology
            .rails
            .iter()
            .filter_map(|net| Some((net.clone(), rail_voltage(net, listed)?)))
            .collect(),
        totals: BTreeMap::new(),
    };
    let rails: Vec<PowerRail> = topology.rails.iter().map(|net| builder.rail(net)).collect();
    let converters = builder.converters();
    let roots = rails
        .iter()
        .filter(|rail| rail.fed_from.is_empty())
        .map(|rail| rail.net.clone())
        .collect();
    PowerTree {
        rails,
        converters,
        roots,
    }
}

fn rail_voltage(net: &str, listed: &BTreeMap<String, ListedRail>) -> Option<RailVoltage> {
    if let Some(rail) = listed.get(net) {
        return Some(RailVoltage {
            volts: rail.voltage,
            tolerance_percent: rail.tolerance_percent,
            from: VoltageFrom::Project,
        });
    }
    voltage_from_net_name(net).map(|volts| RailVoltage {
        volts,
        tolerance_percent: None,
        from: VoltageFrom::NetName,
    })
}

/// A rail's total while the tree is worked out: `None` while it is being
/// worked out, so a loop of converters reads as not told.
type Total = Option<(f64, bool)>;

struct Builder<'a> {
    topology: &'a PowerTopology,
    said: &'a [PartRails],
    voltages: BTreeMap<String, RailVoltage>,
    totals: BTreeMap<String, Total>,
}

impl Builder<'_> {
    fn rail(&mut self, net: &str) -> PowerRail {
        let mut problems = Vec::new();
        let voltage = self.voltages.get(net).copied();
        if voltage.is_none() {
            problems.push(PowerProblem::NoVoltage);
        }
        let sources = self.sources(net, &mut problems);
        match sources.len() {
            0 => problems.push(PowerProblem::NoSource),
            1 => {}
            many => problems.push(PowerProblem::SeveralSources(many)),
        }
        let loads = self.loads(net, &mut problems);
        let (total_a, is_complete) = sum(&loads);
        let fed_from = sources
            .iter()
            .filter(|source| source.is_converter)
            .flat_map(|source| self.said[source.part].inputs())
            .collect();
        PowerRail {
            net: net.to_string(),
            voltage,
            sources,
            loads,
            total_a,
            is_complete,
            fed_from,
            problems,
        }
    }

    fn sources(&self, net: &str, problems: &mut Vec<PowerProblem>) -> Vec<RailSource> {
        let mut out = Vec::new();
        for (part, said) in self.said.iter().enumerate() {
            let Some(limit) = said.supplies.get(net) else {
                continue;
            };
            if let Err(text) = limit {
                problems.push(self.unread(part, SUPPLY_PARAMETER, net, text));
            }
            out.push(RailSource {
                part,
                limit_a: limit.as_ref().ok().copied(),
                is_converter: !said.inputs().is_empty(),
            });
        }
        out
    }

    fn loads(&mut self, net: &str, problems: &mut Vec<PowerProblem>) -> Vec<RailLoad> {
        let all = self.said;
        let mut out = Vec::new();
        for (part, said) in all.iter().enumerate() {
            if !said.touches.contains(net) || said.supplies.contains_key(net) {
                continue;
            }
            let (current_a, from) = match said.loads.get(net) {
                Some(Ok(current)) => (Some(*current), Some(CurrentFrom::Parameter)),
                Some(Err(text)) => {
                    problems.push(self.unread(part, LOAD_PARAMETER, net, text));
                    (None, None)
                }
                None if !said.supplies.is_empty() => {
                    if let Some(Err(text)) = self.efficiency(part) {
                        problems.push(PowerProblem::UnreadParameter {
                            designator: self.designator(part),
                            parameter: EFFICIENCY_PARAMETER.to_string(),
                            text,
                        });
                    }
                    let drawn = self.converter_input_a(part, net);
                    if drawn.is_none() {
                        problems.push(PowerProblem::UnknownInput {
                            designator: self.designator(part),
                        });
                    }
                    (drawn, drawn.map(|_| CurrentFrom::Converter))
                }
                None => {
                    problems.push(PowerProblem::UnknownLoad {
                        designator: self.designator(part),
                    });
                    (None, None)
                }
            };
            out.push(RailLoad {
                part,
                current_a,
                from,
            });
        }
        out
    }

    /// What converter `part` draws from its one input `input`, in A.
    fn converter_input_a(&mut self, part: usize, input: &str) -> Option<f64> {
        if self.said[part].inputs().len() != 1 {
            return None;
        }
        let efficiency = self.efficiency(part).and_then(Result::ok)?;
        let input_volts = self.voltages.get(input)?.volts.abs();
        let output_w = self.output_w(part)?;
        (input_volts > 0.0).then_some(output_w / efficiency / input_volts)
    }

    /// What converter `part`'s outputs deliver, in W: each output rail's
    /// whole draw times its voltage. `None` unless the part is each output's
    /// only source and each output's budget is complete.
    fn output_w(&mut self, part: usize) -> Option<f64> {
        let outputs: Vec<String> = self.said[part].supplies.keys().cloned().collect();
        let mut watts = 0.0;
        for output in outputs {
            let sole = self
                .said
                .iter()
                .enumerate()
                .all(|(other, said)| other == part || !said.supplies.contains_key(&output));
            let volts = self.voltages.get(&output)?.volts.abs();
            let (total, is_complete) = self.total(&output)?;
            if !sole || !is_complete {
                return None;
            }
            watts += total * volts;
        }
        Some(watts)
    }

    /// The whole draw of rail `net` and whether it is complete, worked out
    /// once; `None` inside a loop of converters.
    fn total(&mut self, net: &str) -> Total {
        if let Some(known) = self.totals.get(net) {
            return *known;
        }
        self.totals.insert(net.to_string(), None);
        let mut ignored = Vec::new();
        let loads = self.loads(net, &mut ignored);
        let total = Some(sum(&loads));
        self.totals.insert(net.to_string(), total);
        total
    }

    fn efficiency(&self, part: usize) -> Option<Result<f64, String>> {
        let text = self.topology.parts[part]
            .parameters
            .get(EFFICIENCY_PARAMETER)?;
        Some(parse_efficiency(text).ok_or_else(|| text.clone()))
    }

    fn converters(&mut self) -> Vec<Converter> {
        let all = self.said;
        let mut out = Vec::new();
        for (part, said) in all.iter().enumerate() {
            let inputs = said.inputs();
            if said.supplies.is_empty() || inputs.is_empty() {
                continue;
            }
            let outputs = said.supplies.keys().cloned().collect();
            let efficiency = self.efficiency(part).and_then(Result::ok);
            let output_w = self.output_w(part);
            let input_w = match (output_w, efficiency) {
                (Some(watts), Some(efficiency)) if inputs.len() == 1 => Some(watts / efficiency),
                _ => None,
            };
            out.push(Converter {
                part,
                inputs,
                outputs,
                efficiency,
                output_w,
                input_w,
            });
        }
        out
    }

    fn unread(&self, part: usize, prefix: &str, net: &str, text: &str) -> PowerProblem {
        PowerProblem::UnreadParameter {
            designator: self.designator(part),
            parameter: format!("{prefix}{net}"),
            text: text.to_string(),
        }
    }

    fn designator(&self, part: usize) -> String {
        self.topology.parts[part].designator.clone()
    }
}

/// What the loads that tell draw together, and whether every load tells.
fn sum(loads: &[RailLoad]) -> (f64, bool) {
    let total: f64 = loads.iter().filter_map(|load| load.current_a).sum();
    (total, loads.iter().all(|load| load.current_a.is_some()))
}
