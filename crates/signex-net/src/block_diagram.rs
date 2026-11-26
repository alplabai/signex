//! The block diagram the schematic hierarchy implies (#2968, mockup row
//! 171): one block per sheet, its sheet entries as ports, and one
//! connection per signal group that crosses sheets. Read off the compiled
//! design (`compile_design`), so no net is derived again here. Merging the
//! result into a saved diagram is `signex_types::block_diagram`'s
//! `regenerate_block_diagram`. The grouping rules are in
//! `docs/architecture/snxblk-format.md`, "Generate and regenerate".

use std::collections::{HashMap, HashSet};

use signex_types::block_diagram::{
    DerivedBlock, DerivedConnection, DerivedDiagram, DerivedEnd, DerivedPort, Interface,
    PortDirection,
};
use signex_types::document_id::DocumentId;
use signex_types::project::ChannelNaming;
use signex_types::schematic::{
    BusName, BusTerm, ChildSheet, LabelType, SchematicSheet, parse_bus_name, repeated_entry_name,
};

use crate::{CompiledDesign, ProjectGraph, SheetKey, compile_design};

/// The diagram `graph` implies. `paths` gives each sheet's path relative to
/// the project folder, `/`-separated; a sheet missing from it goes by its
/// key. A sheet that only places other sheets and has no parts of its own
/// (a top sheet) is the frame the blocks sit in, not a block. A sheet whose
/// file carries no document id yet cannot be linked and is left out.
pub fn derive_block_diagram(
    graph: &ProjectGraph,
    naming: &ChannelNaming,
    paths: &HashMap<SheetKey, String>,
) -> DerivedDiagram {
    let design = compile_design(graph, naming);
    let sheets = BlockSheets::of(&design, graph, paths);
    let connections = connections(&design, graph, &sheets);
    DerivedDiagram {
        blocks: sheets.blocks,
        connections,
    }
}

/// The sheets that are blocks, in walk order, and which block each sheet
/// occurrence belongs to.
struct BlockSheets {
    blocks: Vec<DerivedBlock>,
    /// Sheet key -> index into `blocks`.
    index: HashMap<SheetKey, usize>,
}

impl BlockSheets {
    fn of(
        design: &CompiledDesign,
        graph: &ProjectGraph,
        paths: &HashMap<SheetKey, String>,
    ) -> Self {
        let placed = first_placements(design, graph);
        let mut blocks = Vec::new();
        let mut index = HashMap::new();
        let mut seen: HashSet<&SheetKey> = HashSet::new();
        for row in &design.sheets {
            if !seen.insert(&row.key) {
                continue;
            }
            let Some(sheet) = graph.sheets.get(&row.key) else {
                continue;
            };
            let Some(id) = DocumentId::from_uuid(sheet.uuid) else {
                continue;
            };
            if is_frame(design, &row.key, sheet) {
                continue;
            }
            let ports = match placed.get(&row.key) {
                Some(symbol) => entry_ports(symbol),
                None => label_ports(sheet),
            };
            let path = paths
                .get(&row.key)
                .cloned()
                .unwrap_or_else(|| row.key.as_str().replace('\\', "/"));
            index.insert(row.key.clone(), blocks.len());
            blocks.push(DerivedBlock {
                sheet: id,
                path,
                name: row.name.clone(),
                depth: row.depth,
                rails: Vec::new(),
                ports,
            });
        }
        // A block's rails: the power nets touching any of its occurrences.
        for row in design.sheets.iter().filter(|r| r.repeat.is_none()) {
            let Some(&at) = index.get(&row.key) else {
                continue;
            };
            let rails: &mut Vec<String> = &mut blocks[at].rails;
            for net in row.net_names.iter().filter(|n| n.is_power) {
                if !rails.contains(&net.name) {
                    rails.push(net.name.clone());
                }
            }
        }
        Self { blocks, index }
    }
}

/// The sheet symbol that first places each sheet, in walk order.
fn first_placements<'a>(
    design: &CompiledDesign,
    graph: &ProjectGraph<'a>,
) -> HashMap<SheetKey, &'a ChildSheet> {
    let mut placed = HashMap::new();
    for row in &design.sheets {
        let (Some(sheet), Some(children)) =
            (graph.sheets.get(&row.key), graph.resolved.get(&row.key))
        else {
            continue;
        };
        for symbol in &sheet.child_sheets {
            if let Some(child) = children.get(&symbol.filename) {
                placed.entry(child.clone()).or_insert(symbol);
            }
        }
    }
    placed
}

/// A sheet that places other sheets and has no parts on any occurrence.
fn is_frame(design: &CompiledDesign, key: &SheetKey, sheet: &SchematicSheet) -> bool {
    !sheet.child_sheets.is_empty()
        && design
            .sheets
            .iter()
            .filter(|row| row.key == *key)
            .all(|row| row.parts == 0)
}

/// The ports of a sheet placed by `symbol`: its entries, a repeated entry
/// by the net it repeats.
fn entry_ports(symbol: &ChildSheet) -> Vec<DerivedPort> {
    symbol
        .pins
        .iter()
        .map(|pin| DerivedPort {
            name: repeated_entry_name(&pin.name)
                .unwrap_or(&pin.name)
                .trim()
                .to_string(),
            direction: direction_of(&pin.direction),
        })
        .collect()
}

/// The ports of a sheet no symbol places: its Hierarchical labels, each
/// name once.
fn label_ports(sheet: &SchematicSheet) -> Vec<DerivedPort> {
    let mut seen = HashSet::new();
    sheet
        .labels
        .iter()
        .filter(|l| l.label_type == LabelType::Hierarchical)
        .filter(|l| seen.insert(l.text.trim()))
        .map(|l| DerivedPort {
            name: l.text.trim().to_string(),
            direction: direction_of(&l.shape),
        })
        .collect()
}

/// A sheet entry's or port's direction text as a port direction.
fn direction_of(text: &str) -> PortDirection {
    let text = text.trim();
    if text.eq_ignore_ascii_case("input") {
        PortDirection::Input
    } else if text.eq_ignore_ascii_case("output") {
        PortDirection::Output
    } else {
        PortDirection::Bidirectional
    }
}

/// How a crossing net is grouped into a connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GroupKind {
    /// A member of a bus some entry or label names; by the bus text.
    Bus(String),
    /// A power net, on its own.
    Power(String),
    /// A signal of a harness (`TYPE.ENTRY`); by the harness type.
    Harness(String),
    /// Any other net, by the name before its last `_`.
    Prefix(String),
}

struct Group {
    kind: GroupKind,
    nets: Vec<String>,
    /// Indices into the blocks, ascending.
    blocks: Vec<usize>,
}

/// One connection per group and block it reaches beyond the first.
fn connections(
    design: &CompiledDesign,
    graph: &ProjectGraph,
    sheets: &BlockSheets,
) -> Vec<DerivedConnection> {
    let crossing = crossing_nets(design, sheets);
    let buses = bus_names(design, graph);
    let mut groups: Vec<Group> = Vec::new();
    for net in &design.nets {
        let Some(blocks) = crossing.get(net.name.as_str()) else {
            continue;
        };
        let kind = group_kind(&net.name, net.is_power, &buses);
        let at = match groups.iter().position(|g| g.kind == kind) {
            Some(at) => at,
            None => {
                groups.push(Group {
                    kind,
                    nets: Vec::new(),
                    blocks: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[at];
        group.nets.push(net.name.clone());
        for b in blocks {
            if !group.blocks.contains(b) {
                group.blocks.push(*b);
            }
        }
        group.blocks.sort_unstable();
    }
    groups
        .iter()
        .flat_map(|g| group_connections(g, &sheets.blocks))
        .collect()
}

/// Net name -> the blocks it touches, for each net touching two or more.
fn crossing_nets<'d>(
    design: &'d CompiledDesign,
    sheets: &BlockSheets,
) -> HashMap<&'d str, Vec<usize>> {
    let mut touched: HashMap<&str, Vec<usize>> = HashMap::new();
    for row in design.sheets.iter().filter(|r| r.repeat.is_none()) {
        let Some(&block) = sheets.index.get(&row.key) else {
            continue;
        };
        for net in &row.net_names {
            let blocks = touched.entry(net.name.as_str()).or_default();
            if !blocks.contains(&block) {
                blocks.push(block);
            }
        }
    }
    touched.retain(|_, blocks| blocks.len() > 1);
    touched
}

/// Every bus name an entry or a label of the design writes, each once.
fn bus_names(design: &CompiledDesign, graph: &ProjectGraph) -> Vec<(String, BusName)> {
    let mut out: Vec<(String, BusName)> = Vec::new();
    for key in design.sheet_keys() {
        let Some(sheet) = graph.sheets.get(key) else {
            continue;
        };
        let entries = sheet
            .child_sheets
            .iter()
            .flat_map(|s| s.pins.iter().map(|p| p.name.as_str()));
        let labels = sheet.labels.iter().map(|l| l.text.as_str());
        for text in entries.chain(labels) {
            let text = text.trim();
            if out.iter().any(|(t, _)| t == text) {
                continue;
            }
            if let Ok(bus) = parse_bus_name(text) {
                out.push((text.to_string(), bus));
            }
        }
    }
    out
}

fn group_kind(name: &str, is_power: bool, buses: &[(String, BusName)]) -> GroupKind {
    if let Some((text, _)) = buses.iter().find(|(_, bus)| bus.contains(name)) {
        return GroupKind::Bus(text.clone());
    }
    if is_power {
        return GroupKind::Power(name.to_string());
    }
    if let Some((kind, entry)) = name.split_once('.')
        && is_identifier(kind)
        && !entry.is_empty()
    {
        return GroupKind::Harness(kind.to_string());
    }
    GroupKind::Prefix(prefix_of(name).to_string())
}

fn is_identifier(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The text before the last `_` when there is text on both sides, else the
/// whole name: `I2C_SDA` is `I2C`, `VOUT_3V3_CH1` is `VOUT_3V3`.
fn prefix_of(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((prefix, rest)) if !prefix.is_empty() && !rest.is_empty() => prefix,
        _ => name,
    }
}

/// The protocol a prefix names (`SPI`, `I2C1`, `USB`), its trailing digits
/// ignored.
fn protocol_of(prefix: &str) -> Option<Interface> {
    let stem = prefix.trim_end_matches(|c: char| c.is_ascii_digit());
    let stem = if stem.is_empty() { prefix } else { stem };
    let named = |p: &str| match Interface::from_token(p) {
        i @ (Interface::Spi
        | Interface::I2c
        | Interface::Uart
        | Interface::Usb
        | Interface::Lvds) => Some(i),
        _ => None,
    };
    named(prefix).or_else(|| named(stem))
}

/// The group's name, interface and label.
fn describe(group: &Group) -> (String, Interface, String) {
    match &group.kind {
        GroupKind::Bus(text) => (text.clone(), Interface::Bus, text.clone()),
        GroupKind::Power(name) => (name.clone(), Interface::Power, name.clone()),
        GroupKind::Harness(kind) => (kind.clone(), Interface::Bus, kind.clone()),
        GroupKind::Prefix(prefix) => match group.nets.as_slice() {
            [net] => {
                let interface = protocol_of(prefix).unwrap_or(Interface::Signal);
                (net.clone(), interface, net.clone())
            }
            nets => match protocol_of(prefix) {
                Some(interface) => {
                    let ends: Vec<&str> = nets
                        .iter()
                        .map(|n| n.strip_prefix(prefix.as_str()).unwrap_or(n))
                        .map(|n| n.trim_start_matches('_'))
                        .collect();
                    (prefix.clone(), interface, ends.join(", "))
                }
                None => (prefix.clone(), Interface::Signal, prefix.clone()),
            },
        },
    }
}

/// The group's connections: from the first block whose port for the group
/// is an output (else the first block) to each other block.
fn group_connections(group: &Group, blocks: &[DerivedBlock]) -> Vec<DerivedConnection> {
    let (name, interface, label) = describe(group);
    let port_of = |b: usize| {
        blocks
            .get(b)
            .and_then(|block| port_for(group, &name, block))
    };
    let source = group
        .blocks
        .iter()
        .copied()
        .find(|&b| port_of(b).is_some_and(|p| p.direction == PortDirection::Output))
        .or_else(|| group.blocks.first().copied());
    let Some(source) = source else {
        return Vec::new();
    };
    let end = |b: usize| DerivedEnd {
        sheet: blocks[b].sheet,
        port: port_of(b).map(|p| p.name.clone()),
    };
    group
        .blocks
        .iter()
        .copied()
        .filter(|&b| b != source && b < blocks.len())
        .map(|b| DerivedConnection {
            group: name.clone(),
            interface: interface.clone(),
            label: label.clone(),
            from: end(source),
            to: end(b),
        })
        .collect()
}

/// The port of `block` the group runs through: one named as the group or one
/// of its nets, else a bus entry that carries one of its nets or shares its
/// prefix.
fn port_for<'b>(group: &Group, name: &str, block: &'b DerivedBlock) -> Option<&'b DerivedPort> {
    let named = |p: &&DerivedPort| p.name == name || group.nets.contains(&p.name);
    let carries = |p: &&DerivedPort| {
        parse_bus_name(&p.name).is_ok_and(|bus| {
            group.nets.iter().any(|n| bus.contains(n)) || shares_prefix(&bus, name)
        })
    };
    block
        .ports
        .iter()
        .find(named)
        .or_else(|| block.ports.iter().find(carries))
}

fn shares_prefix(bus: &BusName, name: &str) -> bool {
    !bus.is_group && matches!(bus.terms.as_slice(), [BusTerm::Vector(v)] if v.prefix == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus(text: &str) -> (String, BusName) {
        (text.to_string(), parse_bus_name(text).unwrap())
    }

    fn group(kind: GroupKind, nets: &[&str]) -> Group {
        Group {
            kind,
            nets: nets.iter().map(|n| n.to_string()).collect(),
            blocks: Vec::new(),
        }
    }

    #[test]
    fn a_net_groups_by_bus_then_power_then_harness_then_prefix() {
        let buses = [bus("D[0..7]")];
        let kind = |name: &str, power: bool| group_kind(name, power, &buses);
        assert_eq!(kind("D3", false), GroupKind::Bus("D[0..7]".into()));
        assert_eq!(kind("VIN_12V", true), GroupKind::Power("VIN_12V".into()));
        assert_eq!(
            kind("SPI_BUS.SCK", false),
            GroupKind::Harness("SPI_BUS".into())
        );
        assert_eq!(kind("I2C_SDA", false), GroupKind::Prefix("I2C".into()));
        assert_eq!(
            kind("VOUT_3V3_CH1", false),
            GroupKind::Prefix("VOUT_3V3".into())
        );
        assert_eq!(kind("EN", false), GroupKind::Prefix("EN".into()));
        assert_eq!(kind("_X", false), GroupKind::Prefix("_X".into()));
    }

    #[test]
    fn a_protocol_prefix_names_the_interface_and_lists_the_signals() {
        let i2c = group(GroupKind::Prefix("I2C".into()), &["I2C_SCL", "I2C_SDA"]);
        assert_eq!(
            describe(&i2c),
            ("I2C".into(), Interface::I2c, "SCL, SDA".into())
        );
        let spi = group(
            GroupKind::Prefix("SPI1".into()),
            &["SPI1_MISO", "SPI1_MOSI"],
        );
        assert_eq!(describe(&spi).1, Interface::Spi);
        let usb = group(GroupKind::Prefix("USB".into()), &["USB_DP"]);
        assert_eq!(
            describe(&usb),
            ("USB_DP".into(), Interface::Usb, "USB_DP".into())
        );
        let led = group(GroupKind::Prefix("LED".into()), &["LED_G", "LED_R"]);
        assert_eq!(
            describe(&led),
            ("LED".into(), Interface::Signal, "LED".into())
        );
    }

    #[test]
    fn buses_and_harnesses_are_labelled_with_their_name() {
        let bus = group(GroupKind::Bus("D[0..7]".into()), &["D0", "D1"]);
        assert_eq!(
            describe(&bus),
            ("D[0..7]".into(), Interface::Bus, "D[0..7]".into())
        );
        let harness = group(GroupKind::Harness("SPI_BUS".into()), &["SPI_BUS.SCK"]);
        assert_eq!(
            describe(&harness),
            ("SPI_BUS".into(), Interface::Bus, "SPI_BUS".into())
        );
        let power = group(GroupKind::Power("VBUS".into()), &["VBUS"]);
        assert_eq!(describe(&power).1, Interface::Power);
    }

    #[test]
    fn a_power_word_is_not_a_protocol() {
        assert_eq!(protocol_of("POWER"), None);
        assert_eq!(protocol_of("UART2"), Some(Interface::Uart));
        assert_eq!(protocol_of("LVDS"), Some(Interface::Lvds));
    }
}
