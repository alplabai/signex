//! The board net classes a schematic's class directives ask for (ROADMAP
//! Advanced Schematic exit criterion 3): each class a Net Blanket or a Net
//! Parameters directive gives becomes a class of the project's net-class
//! table, the board's classes, with the nets the netlist puts in it as exact
//! members and the rules its first Net Parameters directive sets as the
//! class's board rules. Update PCB applies the table edit in its Execute
//! (`signex-app` `app::update_pcb::classes_from_schematic`).

use signex_types::diff_pair_class::DiffPairClass;
use signex_types::net::Netlist;
use signex_types::net_class::{NetClass, NetClassBoardRules, first_matching_class};
use signex_types::schematic::{InstancePath, NetRules, SchDirective, SchematicSheet};

use super::NetClassCommand;

/// One class the directives give, with the nets the netlist puts in it.
#[derive(Debug, Clone, PartialEq)]
pub struct SchematicClass {
    pub name: String,
    /// Member net names, in netlist order.
    pub nets: Vec<String>,
    /// The rules of the first Net Parameters directive naming the class.
    pub rules: NetRules,
    /// The diff-pair class that directive names.
    pub diff_pair_class: Option<String>,
    /// The track width of the first Net Blanket naming the class that sets
    /// one: the width when no Net Parameters directive sets it.
    pub blanket_width: Option<f64>,
}

impl SchematicClass {
    fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            nets: Vec::new(),
            rules: NetRules::default(),
            diff_pair_class: None,
            blanket_width: None,
        }
    }
}

/// The classes the directives of the placed `sheets` give, in walk and
/// directive order, with their member nets in the stitched `netlist`.
pub fn schematic_classes<'s>(
    sheets: impl IntoIterator<Item = (&'s InstancePath, &'s SchematicSheet)>,
    netlist: &Netlist,
) -> Vec<SchematicClass> {
    let mut out: Vec<SchematicClass> = Vec::new();
    let mut parametered: Vec<String> = Vec::new();
    for (_, sheet) in sheets {
        for directive in &sheet.directives {
            let Some(name) = directive.net_class() else {
                continue;
            };
            let at = match out.iter().position(|c| c.name == name) {
                Some(at) => at,
                None => {
                    out.push(SchematicClass::named(name));
                    out.len() - 1
                }
            };
            if let SchDirective::Blanket {
                track_width: Some(width),
                ..
            } = directive
            {
                out[at].blanket_width.get_or_insert(*width);
            }
        }
        let local = signex_net::build_netlist(sheet);
        for on in signex_net::net_parameters(sheet, &local) {
            let Some(name) = on.net_class else {
                continue;
            };
            if parametered.contains(&name) {
                continue;
            }
            if let Some(entry) = out.iter_mut().find(|c| c.name == name) {
                entry.rules = on.rules;
                entry.diff_pair_class = on.diff_pair_class;
            }
            parametered.push(name);
        }
    }
    for net in &netlist.nets {
        let Some(class) = net.class.as_ref() else {
            continue;
        };
        if let Some(entry) = out.iter_mut().find(|c| c.name == class.0) {
            entry.nets.push(net.name.clone());
        }
    }
    out
}

/// The board rules `class` asks for over `was`: each rule the directive
/// sets replaces the table's, the others stay; the diff-pair width and gap
/// come from the project's diff-pair class it names, when that is defined.
fn wanted_rules(
    class: &SchematicClass,
    was: NetClassBoardRules,
    diff_pair_classes: &[DiffPairClass],
) -> NetClassBoardRules {
    let rules = &class.rules;
    let pair = (class.diff_pair_class.as_deref())
        .and_then(|name| diff_pair_classes.iter().find(|p| p.name == name));
    NetClassBoardRules {
        clearance: rules.clearance_mm.unwrap_or(was.clearance),
        trace_width: (rules.width_mm.or(class.blanket_width)).unwrap_or(was.trace_width),
        via_diameter: rules.via_diameter_mm.unwrap_or(was.via_diameter),
        via_drill: rules.via_hole_mm.unwrap_or(was.via_drill),
        diff_pair_gap: pair.and_then(|p| p.gap_mm).unwrap_or(was.diff_pair_gap),
        diff_pair_width: pair.and_then(|p| p.width_mm).unwrap_or(was.diff_pair_width),
    }
}

/// The edit that puts `classes` into the net-class `table`: a missing class
/// is added at the end, a net the table does not already put in its class
/// is assigned to it (out of every other class's member list; a pattern of
/// an earlier class still wins), and the class's board rules are set.
/// `None` when the table already says all of it.
pub fn schematic_class_edit(
    table: &[NetClass],
    classes: &[SchematicClass],
    diff_pair_classes: &[DiffPairClass],
) -> Option<NetClassCommand> {
    let mut commands = Vec::new();
    for class in classes {
        let Some(existing) = table.iter().find(|c| c.name == class.name) else {
            let mut added = NetClass::named(&class.name);
            added.nets.clone_from(&class.nets);
            added.set_board_rules(wanted_rules(class, added.board_rules(), diff_pair_classes));
            commands.push(NetClassCommand::Add { class: added });
            continue;
        };
        for net in &class.nets {
            let classed = first_matching_class(table, net).is_some_and(|c| c.name == class.name);
            if !classed {
                commands.push(NetClassCommand::AssignNet {
                    net: net.clone(),
                    class: Some(class.name.clone()),
                });
            }
        }
        let was = existing.board_rules();
        let rules = wanted_rules(class, was, diff_pair_classes);
        if rules != was {
            commands.push(NetClassCommand::SetBoardRules {
                name: class.name.clone(),
                rules,
            });
        }
    }
    (!commands.is_empty()).then_some(NetClassCommand::Batch { commands })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_class_table::apply_net_class_command;

    fn class(name: &str, nets: &[&str], rules: NetRules) -> SchematicClass {
        SchematicClass {
            name: name.to_string(),
            nets: nets.iter().map(|n| n.to_string()).collect(),
            rules,
            diff_pair_class: None,
            blanket_width: None,
        }
    }

    fn applied(
        table: &[NetClass],
        classes: &[SchematicClass],
        pairs: &[DiffPairClass],
    ) -> Vec<NetClass> {
        let mut table = table.to_vec();
        let edit = schematic_class_edit(&table, classes, pairs).expect("an edit");
        apply_net_class_command(&mut table, edit).expect("the edit applies");
        table
    }

    /// A class the table lacks is added with its nets, its directive rules
    /// and the width and gap of the diff-pair class its directive names.
    #[test]
    fn a_new_class_is_added_with_its_nets_and_rules() {
        let mut hs = class(
            "HS",
            &["CLK"],
            NetRules {
                width_mm: Some(0.2),
                clearance_mm: Some(0.15),
                via_diameter_mm: Some(0.6),
                via_hole_mm: Some(0.3),
                current_a: Some(1.0),
            },
        );
        hs.diff_pair_class = Some("USB".to_string());
        let usb = DiffPairClass {
            name: "USB".to_string(),
            width_mm: Some(0.25),
            gap_mm: Some(0.1),
            ..DiffPairClass::named("USB")
        };
        let table = applied(&[], std::slice::from_ref(&hs), &[usb]);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].name, "HS");
        assert_eq!(table[0].nets, ["CLK"]);
        assert_eq!(
            table[0].board_rules(),
            NetClassBoardRules {
                clearance: 0.15,
                trace_width: 0.2,
                via_diameter: 0.6,
                via_drill: 0.3,
                diff_pair_gap: 0.1,
                diff_pair_width: 0.25,
            }
        );
        assert_eq!(
            first_matching_class(&table, "CLK").map(|c| c.name.as_str()),
            Some("HS")
        );
    }

    /// An existing class takes its nets from other classes and the rules the
    /// directives set; a rule they leave unset keeps the table's value, and
    /// a second run changes nothing.
    #[test]
    fn an_existing_class_takes_its_nets_and_rules_once() {
        let mut power = NetClass::named("POWER");
        power.clearance = 0.1;
        power.trace_width = 0.3;
        let mut other = NetClass::named("OTHER");
        other.nets = vec!["VIN".to_string()];
        let wanted = class(
            "POWER",
            &["VIN"],
            NetRules {
                width_mm: Some(0.5),
                ..NetRules::default()
            },
        );
        let table = applied(&[other, power], std::slice::from_ref(&wanted), &[]);
        assert_eq!(
            first_matching_class(&table, "VIN").map(|c| c.name.as_str()),
            Some("POWER")
        );
        let power = table.iter().find(|c| c.name == "POWER").expect("POWER");
        assert_eq!(power.trace_width, 0.5);
        assert_eq!(
            power.clearance, 0.1,
            "an unset rule keeps the table's value"
        );
        assert_eq!(schematic_class_edit(&table, &[wanted], &[]), None);
    }

    /// A net an earlier class's pattern already puts in the class, with the
    /// rules as asked, needs no edit.
    #[test]
    fn a_class_the_table_already_says_needs_no_edit() {
        let mut power = NetClass::named("POWER");
        power.patterns = vec!["V*".to_string()];
        let wanted = class("POWER", &["VIN"], NetRules::default());
        assert_eq!(schematic_class_edit(&[power], &[wanted], &[]), None);
    }
}
