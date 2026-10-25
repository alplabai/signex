use std::collections::HashMap;

use signex_types::net::Netlist;
use signex_types::schematic::InstancePath;

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
    // The bare uuid addresses a symbol the way it always did, so it stays
    // while every page copy resolves to one designator (a symbol on one page,
    // or a reused child without per-placement entries); it is dropped only
    // when the copies disagree, because then no single designator is right.
    let mut shared: HashMap<String, Option<&str>> = HashMap::new();
    for sheet in sheets {
        for sym in &sheet.schematic.symbols {
            shared
                .entry(sym.uuid.to_string())
                .and_modify(|seen| {
                    if *seen != Some(sym.reference.as_str()) {
                        *seen = None;
                    }
                })
                .or_insert(Some(sym.reference.as_str()));
        }
    }
    let mut out = HashMap::new();
    for sheet in sheets {
        for sym in &sheet.schematic.symbols {
            if sym.reference.is_empty() {
                continue;
            }
            let agreed = shared.get(&sym.uuid.to_string()).is_some_and(Option::is_some);

            // `sym.reference` is already the designator of this page's
            // placement (`SheetSnapshot::for_placement`).
            out.entry(symbol_occurrence_key(sym.uuid, &sheet.placement))
                .or_insert_with(|| sym.reference.clone());
            if agreed {
                out.entry(sym.uuid.to_string())
                    .or_insert_with(|| sym.reference.clone());
            }
            out.entry(sym.reference.clone())
                .or_insert_with(|| sym.reference.clone());

            // Entries under the resolver's own rule. Every copy of a symbol
            // carries the same `instances`, so the path keys are identical
            // across copies and cannot fight.
            for (path, reference) in sym.valid_instance_entries() {
                out.entry(format!("{}@{path}", sym.uuid))
                    .or_insert_with(|| reference.to_string());
                let text = path.to_string();
                out.entry(text.clone())
                    .or_insert_with(|| reference.to_string());
                let trimmed = text.trim_matches('/');
                if !trimmed.is_empty() {
                    out.entry(trimmed.to_string())
                        .or_insert_with(|| reference.to_string());
                }
            }
        }
    }
    out
}

/// Key of [`ExpressionTables::net_name_by_symbol_pin`]: the symbol uuid for a
/// symbol on a root page, `uuid@path` for a placement reached through a sheet
/// symbol, so two placements of one symbol never share a pin→net map.
///
/// A printed page looks its symbols up at its own `SheetSnapshot::placement`,
/// so a child printed once per placement finds each copy's nets.
pub fn symbol_occurrence_key(symbol: impl std::fmt::Display, occurrence: &InstancePath) -> String {
    if occurrence.is_root() {
        symbol.to_string()
    } else {
        format!("{symbol}@{occurrence}")
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
                .entry(symbol_occurrence_key(term.symbol, &term.occurrence))
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
    use signex_types::schematic::SymbolInstance;

    #[test]
    fn no_netlist_yields_empty_lookup() {
        assert!(build_pin_net_lookup(None).is_empty());
    }

    #[test]
    fn two_placements_of_one_symbol_keep_separate_pin_net_maps() {
        let symbol = uuid::Uuid::from_u128(5);
        let a = InstancePath::from_chain(vec![uuid::Uuid::from_u128(1)]);
        let b = InstancePath::from_chain(vec![uuid::Uuid::from_u128(2)]);
        let net = |id: u32, name: &str, occurrence: &InstancePath| Net {
            id: NetId(id),
            name: name.to_string(),
            class: None,
            wires: Vec::new(),
            junctions: Vec::new(),
            terminals: vec![Terminal {
                symbol,
                reference: "R1".to_string(),
                pin: "1".to_string(),
                occurrence: occurrence.clone(),
            }],
        };
        let netlist = Netlist {
            nets: vec![net(1, "A_NET", &a), net(2, "B_NET", &b)],
        };
        let lookup = build_pin_net_lookup(Some(&netlist));
        assert_eq!(lookup[&symbol_occurrence_key(symbol, &a)]["1"], "A_NET");
        assert_eq!(lookup[&symbol_occurrence_key(symbol, &b)]["1"], "B_NET");
    }

    fn snapshot(symbols: Vec<signex_types::schematic::Symbol>) -> SheetSnapshot {
        SheetSnapshot {
            path: "a.snxsch".into(),
            schematic: signex_types::schematic::SchematicSheet {
                symbols,
                ..Default::default()
            },
            sheet_name: "a".into(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }
    }

    fn entry(path: &str, reference: &str) -> SymbolInstance {
        SymbolInstance {
            project: "p".into(),
            path: path.into(),
            reference: reference.into(),
            unit: 1,
        }
    }

    #[test]
    fn a_page_copy_finds_its_own_placements_nets_by_exact_key() {
        let symbol = uuid::Uuid::from_u128(5);
        let a = InstancePath::from_chain(vec![uuid::Uuid::from_u128(1)]);
        let netlist = Netlist {
            nets: vec![Net {
                id: NetId(1),
                name: "SIG".to_string(),
                class: None,
                wires: Vec::new(),
                junctions: Vec::new(),
                terminals: vec![Terminal {
                    symbol,
                    reference: "R1".to_string(),
                    pin: "1".to_string(),
                    occurrence: a.clone(),
                }],
            }],
        };
        let lookup = build_pin_net_lookup(Some(&netlist));
        assert_eq!(lookup[&symbol_occurrence_key(symbol, &a)]["1"], "SIG");
    }

    #[test]
    fn page_copies_of_one_symbol_keep_their_own_designators() {
        let sym = signex_types::schematic::Symbol {
            uuid: uuid::Uuid::from_u128(9),
            reference: "R1".into(),
            ..Default::default()
        };
        let a = InstancePath::from_chain(vec![uuid::Uuid::from_u128(1)]);
        let b = InstancePath::from_chain(vec![uuid::Uuid::from_u128(2)]);
        let mut sym_a = sym.clone();
        sym_a.reference = "R1".into();
        let mut sym_b = sym;
        sym_b.reference = "R101".into();
        let mut page_a = snapshot(vec![sym_a]);
        page_a.placement = a.clone();
        let mut page_b = snapshot(vec![sym_b]);
        page_b.placement = b.clone();
        let refdes = build_global_refdes_lookup(&[page_a, page_b]);
        assert_eq!(
            refdes[&symbol_occurrence_key(uuid::Uuid::from_u128(9), &a)],
            "R1"
        );
        assert_eq!(
            refdes[&symbol_occurrence_key(uuid::Uuid::from_u128(9), &b)],
            "R101"
        );
    }

    #[test]
    fn two_symbols_with_entries_for_one_placement_do_not_collide() {
        let placement = InstancePath::from_chain(vec![uuid::Uuid::from_u128(1)]).to_string();
        let mk = |n: u128, own: &str, entry_ref: &str| signex_types::schematic::Symbol {
            uuid: uuid::Uuid::from_u128(n),
            reference: own.into(),
            instances: vec![entry(&placement, entry_ref)],
            ..Default::default()
        };
        let refdes =
            build_global_refdes_lookup(&[snapshot(vec![mk(8, "R1", "R101"), mk(9, "C1", "C101")])]);
        assert_eq!(
            refdes[&format!("{}@{placement}", uuid::Uuid::from_u128(8))],
            "R101"
        );
        assert_eq!(
            refdes[&format!("{}@{placement}", uuid::Uuid::from_u128(9))],
            "C101"
        );
    }

    /// A root-page note `${refdes:<uuid>}` naming a symbol that lives on a
    /// child page (a non-root placement) still resolves, as it did before page
    /// copies; and so does the entry's path.
    #[test]
    fn a_child_symbol_is_addressable_by_its_bare_uuid_and_entry_path() {
        let placement = InstancePath::from_chain(vec![uuid::Uuid::from_u128(1)]);
        let sym = signex_types::schematic::Symbol {
            uuid: uuid::Uuid::from_u128(9),
            reference: "R5".into(),
            instances: vec![entry(&placement.to_string(), "R5")],
            ..Default::default()
        };
        let mut child = snapshot(vec![sym]);
        child.placement = placement.clone();
        let refdes = build_global_refdes_lookup(&[snapshot(vec![]), child]);
        assert_eq!(refdes[&uuid::Uuid::from_u128(9).to_string()], "R5");
        assert_eq!(refdes[&placement.to_string()], "R5");
        assert_eq!(refdes[placement.to_string().trim_matches('/')], "R5");
    }

    /// A child reused twice with no instance entries prints once per
    /// placement, both copies resolving to the symbol's own designator: the
    /// bare uuid still addresses it.
    #[test]
    fn a_reused_child_without_entries_keeps_its_bare_uuid() {
        let sym = signex_types::schematic::Symbol {
            uuid: uuid::Uuid::from_u128(9),
            reference: "R5".into(),
            ..Default::default()
        };
        let mut c1 = snapshot(vec![sym.clone()]);
        c1.placement = InstancePath::from_chain(vec![uuid::Uuid::from_u128(1)]);
        let mut c2 = snapshot(vec![sym]);
        c2.placement = InstancePath::from_chain(vec![uuid::Uuid::from_u128(2)]);
        let refdes = build_global_refdes_lookup(&[snapshot(vec![]), c1, c2]);
        assert_eq!(
            refdes.get(&uuid::Uuid::from_u128(9).to_string()),
            Some(&"R5".to_string())
        );
    }

    #[test]
    fn blank_or_unparseable_entries_add_no_refdes_key() {
        let sym = signex_types::schematic::Symbol {
            uuid: uuid::Uuid::from_u128(9),
            reference: "R1".into(),
            instances: vec![entry("/", " "), entry("junk", "R9")],
            ..Default::default()
        };
        let refdes = build_global_refdes_lookup(&[snapshot(vec![sym])]);
        assert!(refdes.keys().all(|k| !k.contains('@')), "{refdes:?}");
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
                    occurrence: Default::default(),
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
