//! Protel-format (`.NET`) netlist emitter — a one-way export for downstream
//! PCB tools.
//!
//! The format is plain text. Each component is a `[ ... ]` block and each net
//! is a `( ... )` block, one field per line, CRLF line endings:
//!
//! ```text
//! [
//! R1
//! R_0603
//! 10k
//!
//!
//!
//! ]
//! (
//! GND
//! R1-1
//! U2-3
//! )
//! ```
//!
//! Connectivity is never re-derived here: nets and terminals are read straight
//! off the authoritative [`Netlist`] (ADR-0002 D7), in its stable id/terminal
//! order. Only the designator → footprint/value join walks the sheet symbols,
//! and it walks `ExportContext::netlist_sheets` — every sheet the netlist was
//! derived from, hierarchical children included — not the printed pages, so a
//! terminal in a net block always has its component block.

use std::collections::{BTreeSet, HashMap, HashSet};

use signex_types::net::Netlist;
use signex_types::schematic::Symbol;

use super::NetlistError;
use crate::ExportContext;

/// Line terminator the format expects.
const EOL: &str = "\r\n";

/// Blank padding lines closing a component block (the format reserves them for
/// per-component fields Signex does not populate).
const COMPONENT_PADDING_LINES: usize = 3;

/// One `[ ... ]` block's payload.
struct Component {
    designator: String,
    footprint: String,
    comment: String,
}

/// Emit the Protel netlist for `netlist`, joining footprint/value from the
/// symbols in `ctx.netlist_sheets`. The output is always a clean, valid Protel
/// file: the format has no comment syntax, so an incomplete export is not
/// marked in the file — the caller reports it to the user instead.
pub(super) fn emit(ctx: &ExportContext, netlist: &Netlist) -> Result<Vec<u8>, NetlistError> {
    check_designators(ctx)?;
    let mut out = String::new();
    for component in collect_components(ctx) {
        push_component(&mut out, &component);
    }
    let unplaced = unplaced_references(ctx);
    for net in &netlist.nets {
        push_line(&mut out, "(");
        let name = field(&net.name);
        if name.is_empty() {
            push_line(&mut out, &format!("NET{}", net.id.0));
        } else {
            push_line(&mut out, &name);
        }
        for t in net
            .terminals
            .iter()
            .filter(|t| !is_unplaced(&unplaced, &t.reference))
        {
            push_line(
                &mut out,
                &format!("{}-{}", field(&t.reference), field(&t.pin)),
            );
        }
        push_line(&mut out, ")");
    }
    Ok(out.into_bytes())
}

/// Refuse a project whose designators cannot name one part each. The block
/// structure keys terminals on the designator alone, so an unannotated `R?` or
/// two parts sharing `R1` would silently merge into one component and wire
/// pins of different parts together. These are the conditions the annotate and
/// "Annotate Schematics..." and "Reset Duplicate Schematic Designators..."
/// menu commands act on; a multi-unit part (one designator, distinct units,
/// one library symbol) is not a duplicate. A real part with a blank designator
/// is refused too: it has no name to key its terminals on.
fn check_designators(ctx: &ExportContext) -> Result<(), NetlistError> {
    let mut unannotated = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    let mut blank = 0;
    let mut units: HashSet<(String, i64)> = HashSet::new();
    let mut lib_ids: HashMap<String, &str> = HashMap::new();
    for sym in ctx.netlist_sheets.iter().flat_map(|s| &s.symbols) {
        if is_skipped_symbol(sym) {
            continue;
        }
        let designator = field(&sym.reference);
        if designator.is_empty() {
            blank += 1;
            continue;
        }
        if designator.ends_with('?') {
            unannotated.insert(designator);
            continue;
        }
        if !units.insert((designator.clone(), i64::from(sym.unit))) {
            duplicates.insert(designator.clone());
        }
        if *lib_ids.entry(designator.clone()).or_insert(&sym.lib_id) != sym.lib_id {
            duplicates.insert(designator);
        }
    }
    if blank > 0 {
        return Err(NetlistError::BlankDesignators(blank));
    }
    if !unannotated.is_empty() {
        return Err(NetlistError::UnannotatedDesignators(join(&unannotated)));
    }
    if !duplicates.is_empty() {
        return Err(NetlistError::DuplicateDesignators(join(&duplicates)));
    }
    Ok(())
}

fn join(set: &BTreeSet<String>) -> String {
    set.iter().cloned().collect::<Vec<_>>().join(", ")
}

/// Components in sheet order then symbol order, one per designator (a
/// multi-unit part shares its designator across units). Power symbols are
/// skipped with the same predicate as the BOM.
fn collect_components(ctx: &ExportContext) -> Vec<Component> {
    let mut seen = HashSet::new();
    let mut components = Vec::new();
    for sheet in &ctx.netlist_sheets {
        for sym in &sheet.symbols {
            if is_skipped_symbol(sym) {
                continue;
            }
            let designator = field(&sym.reference);
            if !seen.insert(designator.clone()) {
                continue;
            }
            components.push(Component {
                designator,
                footprint: field(&sym.footprint),
                comment: field(&sym.value),
            });
        }
    }
    components
}

/// True for a symbol that gets no component block and whose terminals stay
/// out of the nets: a power symbol (same predicate as the BOM) or one that is
/// not on the board. A real part with no designator is not skipped — it is
/// refused by [`check_designators`].
fn is_skipped_symbol(sym: &Symbol) -> bool {
    sym.reference.starts_with("#PWR") || sym.is_power || !sym.on_board
}

/// Designators of the skipped symbols. The netlist still carries their
/// terminals (a GND port is a pin on the GND net), but naming one in a net
/// block would point at a component the file never declares.
fn unplaced_references(ctx: &ExportContext) -> HashSet<String> {
    let symbols = || ctx.netlist_sheets.iter().flat_map(|s| &s.symbols);
    // A designator that also names a real board part stays in the nets.
    let placed: HashSet<String> = symbols()
        .filter(|sym| !is_skipped_symbol(sym))
        .map(|sym| field(&sym.reference))
        .collect();
    symbols()
        .filter(|sym| is_skipped_symbol(sym))
        .map(|sym| field(&sym.reference))
        .filter(|reference| !placed.contains(reference))
        .collect()
}

fn is_unplaced(unplaced: &HashSet<String>, reference: &str) -> bool {
    let reference = field(reference);
    reference.is_empty() || reference.starts_with("#PWR") || unplaced.contains(&reference)
}

fn push_component(out: &mut String, c: &Component) {
    push_line(out, "[");
    push_line(out, &c.designator);
    push_line(out, &c.footprint);
    push_line(out, &c.comment);
    for _ in 0..COMPONENT_PADDING_LINES {
        push_line(out, "");
    }
    push_line(out, "]");
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push_str(EOL);
}

/// A field is one line: line breaks and other control characters would split
/// it and corrupt the block structure, so they become spaces; ends are trimmed.
/// A field that is nothing but a block delimiter (`[`, `]`, `(`, `)`) would
/// open or close a block early, so it gets a `_` prefix.
fn field(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if matches!(trimmed, "[" | "]" | "(" | ")") {
        return format!("_{trimmed}");
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Exporter, NetlistExporter, NetlistFormat, NetlistOptions, SheetSnapshot};
    use signex_types::net::{Net, NetId, Terminal};
    use signex_types::schematic::{SchematicSheet, Symbol};
    use uuid::Uuid;

    fn sheet(symbols: Vec<Symbol>) -> SheetSnapshot {
        SheetSnapshot {
            path: "a.snxsch".into(),
            schematic: SchematicSheet {
                symbols,
                ..Default::default()
            },
            sheet_name: "a".into(),
            sheet_number: 1,
            sheet_count: 1,
        }
    }

    fn symbol(reference: &str, value: &str, footprint: &str, is_power: bool) -> Symbol {
        Symbol {
            lib_id: "Device:X".to_string(),
            reference: reference.to_string(),
            value: value.to_string(),
            footprint: footprint.to_string(),
            is_power,
            ..Default::default()
        }
    }

    fn net(id: u32, name: &str, terminals: &[(&str, &str)]) -> Net {
        Net {
            id: NetId(id),
            name: name.to_string(),
            class: None,
            wires: Vec::new(),
            junctions: Vec::new(),
            terminals: terminals
                .iter()
                .map(|(r, p)| Terminal {
                    symbol: Uuid::nil(),
                    reference: r.to_string(),
                    pin: p.to_string(),
                })
                .collect(),
        }
    }

    fn ctx(symbols: Vec<Symbol>, nets: Vec<Net>) -> ExportContext {
        let snapshot = sheet(symbols);
        ExportContext {
            netlist_sheets: vec![snapshot.schematic.clone()],
            sheets: vec![snapshot],
            metadata: Default::default(),
            netlist: Some(Netlist { nets }),
        }
    }

    fn add_sheet(c: &mut ExportContext, symbols: Vec<Symbol>) {
        let snapshot = sheet(symbols);
        c.netlist_sheets.push(snapshot.schematic.clone());
        c.sheets.push(snapshot);
    }

    #[test]
    fn a_child_only_in_netlist_sheets_still_gets_its_component_block() {
        let mut c = ctx(vec![symbol("R1", "10k", "", false)], vec![]);
        c.netlist_sheets
            .push(sheet(vec![symbol("R_CHILD", "1k", "R_0402", false)]).schematic);
        c.netlist = Some(Netlist {
            nets: vec![net(1, "N", &[("R1", "1"), ("R_CHILD", "1")])],
        });
        let text = protel(&c, None);
        assert!(text.contains("[\r\nR_CHILD\r\nR_0402\r\n1k\r\n"), "{text}");
    }

    #[test]
    fn a_duplicate_inside_a_child_sheet_is_refused() {
        let mut c = ctx(vec![symbol("R1", "10k", "", false)], vec![]);
        c.netlist_sheets
            .push(sheet(vec![symbol("R1", "10k", "", false)]).schematic);
        assert!(matches!(
            export_err(&c),
            NetlistError::DuplicateDesignators(_)
        ));
    }

    #[test]
    fn a_power_symbol_in_a_child_sheet_stays_out_of_net_blocks() {
        let mut c = ctx(vec![symbol("R1", "10k", "", false)], vec![]);
        c.netlist_sheets
            .push(sheet(vec![symbol("GND1", "GND", "", true)]).schematic);
        c.netlist = Some(Netlist {
            nets: vec![net(1, "GND", &[("GND1", "1"), ("R1", "1")])],
        });
        let text = protel(&c, None);
        assert!(!text.contains("GND1"), "{text}");
    }

    fn protel(ctx: &ExportContext, note: Option<Vec<String>>) -> String {
        let opts = NetlistOptions {
            format: NetlistFormat::Protel,
            incomplete_note: note,
        };
        let out = NetlistExporter.export(ctx, &opts).expect("netlist present");
        String::from_utf8(out.bytes).unwrap()
    }

    fn fixture() -> ExportContext {
        ctx(
            vec![
                symbol("R1", "10k", "R_0603", false),
                symbol("U2", "MCU", "QFN-32", false),
                symbol("C1", "100n", "C_0402", false),
                symbol("#PWR01", "GND", "", true),
            ],
            vec![
                net(1, "GND", &[("C1", "2"), ("R1", "1"), ("U2", "3")]),
                net(2, "VCC", &[("C1", "1"), ("U2", "1")]),
            ],
        )
    }

    #[test]
    fn two_nets_three_components_match_the_golden_bytes() {
        let expected = [
            "[", "R1", "R_0603", "10k", "", "", "", "]", //
            "[", "U2", "QFN-32", "MCU", "", "", "", "]", //
            "[", "C1", "C_0402", "100n", "", "", "", "]", //
            "(", "GND", "C1-2", "R1-1", "U2-3", ")", //
            "(", "VCC", "C1-1", "U2-1", ")",
        ]
        .iter()
        .map(|l| format!("{l}\r\n"))
        .collect::<String>();
        assert_eq!(protel(&fixture(), None), expected);
    }

    #[test]
    fn power_port_and_unreferenced_terminals_never_dangle() {
        let c = ctx(
            vec![
                symbol("R1", "10k", "R_0603", false),
                symbol("#PWR01", "GND", "", true),
                symbol("", "GND", "", true),
            ],
            vec![net(1, "GND", &[("#PWR01", "1"), ("R1", "1"), ("", "4")])],
        );
        let text = protel(&c, None);
        assert!(text.ends_with("(\r\nGND\r\nR1-1\r\n)\r\n"), "{text}");
        assert!(!text.contains("#PWR01") && !text.contains("-4"), "{text}");
    }

    #[test]
    fn a_real_part_with_a_blank_designator_is_refused() {
        let c = ctx(
            vec![
                symbol("R1", "10k", "R_0603", false),
                symbol("", "X", "", false),
                symbol(" ", "Y", "", false),
            ],
            vec![net(1, "N", &[("R1", "1"), ("", "4")])],
        );
        assert!(matches!(export_err(&c), NetlistError::BlankDesignators(2)));
    }

    #[test]
    fn a_part_not_on_the_board_gets_no_block_and_no_terminals() {
        let off_board = Symbol {
            on_board: false,
            ..symbol("TP1", "TP", "", false)
        };
        let c = ctx(
            vec![symbol("R1", "10k", "R_0603", false), off_board],
            vec![net(1, "N", &[("R1", "1"), ("TP1", "1")])],
        );
        let text = protel(&c, None);
        assert!(!text.contains("TP1"), "{text}");
        assert!(text.ends_with("(\r\nN\r\nR1-1\r\n)\r\n"), "{text}");
        assert_eq!(text.matches("[\r\n").count(), 1, "{text}");
    }

    #[test]
    fn an_off_board_part_sharing_a_designator_does_not_drop_the_real_one() {
        let off_board = Symbol {
            on_board: false,
            ..symbol("R1", "10k", "", false)
        };
        let c = ctx(
            vec![symbol("R1", "10k", "R_0603", false), off_board],
            vec![net(1, "N", &[("R1", "1")])],
        );
        assert!(protel(&c, None).contains("R1-1"));
    }

    #[test]
    fn a_net_with_no_terminals_is_still_a_block() {
        let c = ctx(vec![], vec![net(7, "FLOATING", &[])]);
        assert_eq!(protel(&c, None), "(\r\nFLOATING\r\n)\r\n");
    }

    #[test]
    fn an_empty_footprint_leaves_its_line_blank() {
        let c = ctx(vec![symbol("R1", "10k", "", false)], vec![]);
        assert_eq!(protel(&c, None), "[\r\nR1\r\n\r\n10k\r\n\r\n\r\n\r\n]\r\n");
    }

    #[test]
    fn line_breaks_in_fields_cannot_split_a_block() {
        let c = ctx(
            vec![symbol("R1", "10k\r\n)", "R\n0603", false)],
            vec![net(1, "A\nB", &[("R1", "1")]), net(2, "", &[("R1", "2")])],
        );
        assert_eq!(
            protel(&c, None),
            "[\r\nR1\r\nR 0603\r\n10k  )\r\n\r\n\r\n\r\n]\r\n\
             (\r\nA B\r\nR1-1\r\n)\r\n(\r\nNET2\r\nR1-2\r\n)\r\n"
        );
    }

    #[test]
    fn multi_unit_designators_emit_one_component_block() {
        let c = ctx(
            vec![
                symbol("U1", "OPAMP", "SOIC-8", false),
                Symbol {
                    unit: 2,
                    ..symbol("U1", "OPAMP", "SOIC-8", false)
                },
            ],
            vec![],
        );
        assert_eq!(protel(&c, None).matches("[\r\n").count(), 1);
    }

    fn export_err(c: &ExportContext) -> NetlistError {
        let opts = NetlistOptions {
            format: NetlistFormat::Protel,
            incomplete_note: None,
        };
        NetlistExporter.export(c, &opts).expect_err("must refuse")
    }

    #[test]
    fn two_parts_sharing_a_designator_are_refused() {
        let c = ctx(
            vec![
                symbol("R1", "10k", "R_0603", false),
                symbol("R1", "22k", "R_0603", false),
            ],
            vec![net(1, "A", &[("R1", "1")])],
        );
        assert!(matches!(
            export_err(&c),
            NetlistError::DuplicateDesignators(ref d) if d == "R1"
        ));
    }

    #[test]
    fn a_designator_repeated_across_sheets_is_refused() {
        let mut c = ctx(vec![symbol("R1", "10k", "", false)], vec![]);
        add_sheet(&mut c, vec![symbol("R1", "10k", "", false)]);
        assert!(matches!(
            export_err(&c),
            NetlistError::DuplicateDesignators(_)
        ));
    }

    #[test]
    fn an_unannotated_designator_is_refused() {
        let c = ctx(
            vec![
                symbol("R?", "10k", "", false),
                symbol("R?", "1k", "", false),
            ],
            vec![],
        );
        assert!(matches!(
            export_err(&c),
            NetlistError::UnannotatedDesignators(ref d) if d == "R?"
        ));
    }

    #[test]
    fn a_value_that_is_only_a_delimiter_cannot_close_a_block() {
        let c = ctx(
            vec![symbol("R1", "]", ")", false)],
            vec![net(1, ")", &[("R1", "1")])],
        );
        assert_eq!(
            protel(&c, None),
            "[\r\nR1\r\n_)\r\n_]\r\n\r\n\r\n\r\n]\r\n(\r\n_)\r\nR1-1\r\n)\r\n"
        );
    }

    #[test]
    fn a_multi_sheet_project_lists_every_net_and_terminal_once() {
        let mut c = ctx(
            vec![
                symbol("R1", "10k", "R_0603", false),
                symbol("#PWR01", "GND", "", true),
            ],
            vec![
                net(
                    1,
                    "GND",
                    &[("#PWR01", "1"), ("R1", "2"), ("C1", "2"), ("U1", "3")],
                ),
                net(2, "SIG", &[("R1", "1"), ("U1", "1")]),
            ],
        );
        add_sheet(
            &mut c,
            vec![
                symbol("C1", "100n", "C_0402", false),
                symbol("U1", "MCU", "QFN-32", false),
                symbol("#PWR02", "GND", "", true),
            ],
        );
        let text = protel(&c, None);
        let lines: Vec<&str> = text.split("\r\n").collect();
        let count = |l: &str| lines.iter().filter(|x| **x == l).count();
        for name in ["GND", "SIG"] {
            assert_eq!(count(name), 1, "{text}");
        }
        for terminal in ["R1-2", "C1-2", "U1-3", "R1-1", "U1-1"] {
            assert_eq!(count(terminal), 1, "{terminal}\n{text}");
        }
        assert_eq!(text.matches("[\r\n").count(), 3, "{text}");
        assert_eq!(text.matches("(\r\n").count(), 2, "{text}");
        assert!(!text.contains("#PWR"), "{text}");
    }

    #[test]
    fn export_twice_is_byte_identical() {
        let c = fixture();
        assert_eq!(protel(&c, None), protel(&c, None));
    }

    #[test]
    fn an_incomplete_note_never_puts_a_comment_in_the_file() {
        let note = Some(vec!["page 'b.snxsch' omitted".to_string()]);
        let text = protel(&fixture(), note);
        assert_eq!(text, protel(&fixture(), None), "the file is unchanged");
        assert!(text.starts_with("[\r\n"), "{text}");
        assert!(
            !text.contains("INCOMPLETE") && !text.contains('#'),
            "{text}"
        );
    }
}
