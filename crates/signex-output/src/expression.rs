use std::collections::HashMap;

use signex_types::net::Netlist;
use signex_types::schematic::SymbolInstance;

use crate::SheetSnapshot;

#[derive(Debug, Clone, Default)]
pub struct ExpressionTables {
    pub global_refdes: HashMap<String, String>,
    pub net_name_by_symbol_pin: HashMap<String, HashMap<String, String>>,
}

/// Builds the PDF/preview expression tables. Net names come from the
/// project's authoritative `Netlist` (ADR-0002 D7) rather than a
/// PDF-local re-derivation — `netlist` is `None` when the caller didn't
/// derive one (e.g. a PDF-only export with no netlist attached), in which
/// case `net_name_by_symbol_pin` is empty rather than guessing.
pub fn build_expression_tables(
    sheets: &[SheetSnapshot],
    netlist: Option<&Netlist>,
) -> ExpressionTables {
    ExpressionTables {
        global_refdes: build_global_refdes_lookup(sheets),
        net_name_by_symbol_pin: build_pin_net_lookup(netlist),
    }
}

pub fn sheet_cell_value(sheet: &SheetSnapshot) -> String {
    let page = sheet.schematic.root_sheet_page.trim();
    if page.is_empty() {
        sheet.sheet_number.to_string()
    } else {
        page.to_string()
    }
}

fn build_global_refdes_lookup(sheets: &[SheetSnapshot]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for sheet in sheets {
        for sym in &sheet.schematic.symbols {
            if sym.reference.is_empty() {
                continue;
            }

            out.entry(sym.uuid.to_string())
                .or_insert_with(|| sym.reference.clone());
            out.entry(sym.reference.clone())
                .or_insert_with(|| sym.reference.clone());

            for instance in &sym.instances {
                insert_instance_keys(&mut out, instance, &sym.reference);
            }
        }
    }
    out
}

fn insert_instance_keys(
    out: &mut HashMap<String, String>,
    instance: &SymbolInstance,
    reference: &str,
) {
    if instance.path.is_empty() {
        return;
    }
    out.entry(instance.path.clone())
        .or_insert_with(|| reference.to_string());
    let trimmed = instance.path.trim_matches('/');
    if !trimmed.is_empty() {
        out.entry(trimmed.to_string())
            .or_insert_with(|| reference.to_string());
    }
}

/// Reads net names straight off the authoritative `Netlist` (ADR-0002 D7)
/// instead of re-deriving connectivity: one `HashMap` walk over the
/// netlist's terminals, keyed by symbol uuid then pin number. `None` (no
/// netlist attached to this export) yields an empty table rather than a
/// guess — the whole point of reading the contract instead of re-deriving
/// it is that "unknown" stays unknown.
fn build_pin_net_lookup(netlist: Option<&Netlist>) -> HashMap<String, HashMap<String, String>> {
    let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
    let Some(netlist) = netlist else {
        return result;
    };

    for net in &netlist.nets {
        for term in &net.terminals {
            result
                .entry(term.symbol.to_string())
                .or_default()
                .insert(term.pin.clone(), net.name.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::net::{Net, NetId, Terminal};

    #[test]
    fn no_netlist_yields_empty_lookup() {
        assert!(build_pin_net_lookup(None).is_empty());
    }

    #[test]
    fn pin_net_lookup_reads_names_from_the_netlist() {
        let symbol_uuid = uuid::Uuid::new_v4();
        let netlist = Netlist {
            nets: vec![Net {
                id: NetId(1),
                name: "+3V3".to_string(),
                class: None,
                wires: Vec::new(),
                junctions: Vec::new(),
                terminals: vec![Terminal {
                    symbol: symbol_uuid,
                    reference: "R1".to_string(),
                    pin: "1".to_string(),
                }],
            }],
        };

        let lookup = build_pin_net_lookup(Some(&netlist));
        let pin_map = lookup
            .get(&symbol_uuid.to_string())
            .expect("symbol net map should exist");
        assert_eq!(pin_map.get("1").cloned(), Some("+3V3".to_string()));
    }
}
