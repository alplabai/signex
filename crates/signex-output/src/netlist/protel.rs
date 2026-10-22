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
//! order. Only the designator → footprint/value join walks the sheet symbols.

use std::collections::HashSet;

use signex_types::net::Netlist;

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
/// symbols in `ctx.sheets`. When `incomplete_note` is `Some`, a `# `-marked
/// header naming the omitted pages is written ahead of the first block (#431).
pub(super) fn emit(
    ctx: &ExportContext,
    netlist: &Netlist,
    incomplete_note: Option<&[String]>,
) -> Vec<u8> {
    let mut out = String::new();
    if let Some(note) = incomplete_note {
        out.push_str(&super::incomplete_header(note).replace('\n', EOL));
    }
    for component in collect_components(ctx) {
        push_component(&mut out, &component);
    }
    for net in &netlist.nets {
        push_line(&mut out, "(");
        let name = field(&net.name);
        if name.is_empty() {
            push_line(&mut out, &format!("NET{}", net.id.0));
        } else {
            push_line(&mut out, &name);
        }
        for t in &net.terminals {
            push_line(
                &mut out,
                &format!("{}-{}", field(&t.reference), field(&t.pin)),
            );
        }
        push_line(&mut out, ")");
    }
    out.into_bytes()
}

/// Components in sheet order then symbol order, one per designator (a
/// multi-unit part shares its designator across units). Power symbols are
/// skipped with the same predicate as the BOM.
fn collect_components(ctx: &ExportContext) -> Vec<Component> {
    let mut seen = HashSet::new();
    let mut components = Vec::new();
    for sheet in &ctx.sheets {
        for sym in &sheet.schematic.symbols {
            if sym.reference.starts_with("#PWR") || sym.is_power {
                continue;
            }
            let designator = field(&sym.reference);
            if designator.is_empty() || !seen.insert(designator.clone()) {
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
fn field(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Exporter, NetlistExporter, NetlistFormat, NetlistOptions, SheetSnapshot};
    use signex_types::net::{Net, NetId, Terminal};
    use signex_types::schematic::{Point, SchematicSheet, Symbol};
    use uuid::Uuid;

    fn sheet(symbols: Vec<Symbol>) -> SheetSnapshot {
        SheetSnapshot {
            path: "a.snxsch".into(),
            schematic: SchematicSheet {
                uuid: Uuid::nil(),
                version: 0,
                generator: String::new(),
                generator_version: String::new(),
                paper_size: String::new(),
                root_sheet_page: "1".into(),
                symbols,
                wires: vec![],
                junctions: vec![],
                labels: vec![],
                child_sheets: vec![],
                no_connects: vec![],
                text_notes: vec![],
                buses: vec![],
                bus_entries: vec![],
                drawings: vec![],
                no_erc_directives: vec![],
                title_block: Default::default(),
                lib_symbols: Default::default(),
            },
            sheet_name: "a".into(),
            sheet_number: 1,
            sheet_count: 1,
        }
    }

    fn symbol(reference: &str, value: &str, footprint: &str, is_power: bool) -> Symbol {
        Symbol {
            uuid: Uuid::nil(),
            lib_id: "Device:X".to_string(),
            reference: reference.to_string(),
            value: value.to_string(),
            footprint: footprint.to_string(),
            datasheet: String::new(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            fields_user_placed: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: Default::default(),
            custom_properties: Vec::new(),
            pin_uuids: Default::default(),
            instances: Vec::new(),
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
        ExportContext {
            sheets: vec![sheet(symbols)],
            metadata: Default::default(),
            netlist: Some(Netlist { nets }),
        }
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
                symbol("U1", "OPAMP", "SOIC-8", false),
            ],
            vec![],
        );
        assert_eq!(protel(&c, None).matches("[\r\n").count(), 1);
    }

    #[test]
    fn export_twice_is_byte_identical() {
        let c = fixture();
        assert_eq!(protel(&c, None), protel(&c, None));
    }

    #[test]
    fn incomplete_note_is_a_comment_header_before_the_first_block() {
        let text = protel(
            &fixture(),
            Some(vec!["page 'b.snxsch' omitted".to_string()]),
        );
        assert!(text.starts_with("# "), "{text}");
        assert!(text.contains("INCOMPLETE NETLIST") && text.contains("b.snxsch"));
        assert!(text.find("INCOMPLETE").unwrap() < text.find("[\r\n").unwrap());
        assert!(
            !text.replace("\r\n", "").contains('\n'),
            "mixed line endings"
        );
    }
}
