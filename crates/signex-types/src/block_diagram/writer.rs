//! The `.snxblk` writer. Every byte is written here, not by a serializer,
//! so the same diagram always gives the same text: keys in a fixed order,
//! objects in diagram order, specs sorted by name, an optional key left out
//! when it is empty. The layout is in `docs/architecture/snxblk-format.md`.

use uuid::Uuid;

use super::{
    Block, BlockDiagram, BlockDiagramFileError, BlockGroup, BlockPort, Connection,
    SNXBLK_FORMAT_V1, direction_token, link_state_token, shape_token, side_token,
};
use crate::format::mm_as_stored_nm;
use crate::theme::Color;

/// The text of `diagram` as its file holds it. Refused for a diagram with a
/// fault or a length the file cannot hold.
pub fn encode_block_diagram(diagram: &BlockDiagram) -> Result<String, BlockDiagramFileError> {
    if let Some(fault) = diagram.fault() {
        return Err(fault.into());
    }
    let mut out = String::new();
    key(&mut out, "format", &quote(SNXBLK_FORMAT_V1));
    key(&mut out, "diagram_id", &quote(&diagram.id.to_string()));
    for group in &diagram.groups {
        write_group(&mut out, group);
    }
    for block in &diagram.blocks {
        write_block(&mut out, block)?;
    }
    for link in &diagram.connections {
        write_connection(&mut out, link);
    }
    Ok(out)
}

fn write_group(out: &mut String, group: &BlockGroup) {
    out.push_str("\n[[groups]]\n");
    key(out, "id", &id(group.id));
    key(out, "name", &quote(&group.name));
    optional_color(out, group.color);
}

fn write_block(out: &mut String, block: &Block) -> Result<(), BlockDiagramFileError> {
    out.push_str("\n[[blocks]]\n");
    key(out, "id", &id(block.id));
    key(out, "name", &quote(&block.name));
    optional_text(out, "function", &block.function);
    optional_text(out, "note", &block.note);
    key(out, "shape", &quote(shape_token(block.shape)));
    let lengths = [
        ("x_nm", block.position.x),
        ("y_nm", block.position.y),
        ("width_nm", block.width_mm),
        ("height_nm", block.height_mm),
    ];
    for (name, mm) in lengths {
        let value = nm(mm, || format!("{name} of block {}", block.id))?;
        key(out, name, &value);
    }
    optional_color(out, block.color);
    if let Some(group) = block.group {
        key(out, "group", &id(group));
    }
    if !block.rails.is_empty() {
        let rails: Vec<String> = block.rails.iter().map(|r| quote(r)).collect();
        key(out, "rails", &format!("[{}]", rails.join(", ")));
    }
    if !block.specs.is_empty() {
        let specs: Vec<String> = block
            .specs
            .iter()
            .map(|(name, value)| format!("{} = {}", quote(name), quote(value)))
            .collect();
        key(out, "specs", &format!("{{ {} }}", specs.join(", ")));
    }
    if let Some(link) = &block.sheet {
        key(out, "sheet", &quote(&link.sheet.to_string()));
        key(out, "sheet_path", &quote(&link.path));
        key(out, "sheet_state", &quote(link_state_token(link.state)));
    }
    for port in &block.ports {
        write_port(out, port)?;
    }
    Ok(())
}

fn write_port(out: &mut String, port: &BlockPort) -> Result<(), BlockDiagramFileError> {
    out.push_str("\n[[blocks.ports]]\n");
    key(out, "id", &id(port.id));
    key(out, "name", &quote(&port.name));
    key(out, "side", &quote(side_token(port.side)));
    let offset = nm(port.offset_mm, || format!("offset_nm of port {}", port.id))?;
    key(out, "offset_nm", &offset);
    key(out, "direction", &quote(direction_token(port.direction)));
    Ok(())
}

fn write_connection(out: &mut String, link: &Connection) {
    out.push_str("\n[[connections]]\n");
    key(out, "id", &id(link.id));
    key(out, "from_block", &id(link.from.block));
    if let Some(port) = link.from.port {
        key(out, "from_port", &id(port));
    }
    key(out, "to_block", &id(link.to.block));
    if let Some(port) = link.to.port {
        key(out, "to_port", &id(port));
    }
    key(out, "interface", &quote(link.interface.token()));
    optional_text(out, "label", &link.label);
    if let Some(source) = &link.derived_from {
        key(out, "derived_from", &quote(source));
    }
}

fn key(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push_str(" = ");
    out.push_str(value);
    out.push('\n');
}

fn optional_text(out: &mut String, name: &str, text: &str) {
    if !text.is_empty() {
        key(out, name, &quote(text));
    }
}

fn optional_color(out: &mut String, color: Option<Color>) {
    if let Some(color) = color {
        key(out, "color", &quote(&color.to_hex()));
    }
}

fn id(id: Uuid) -> String {
    quote(&id.hyphenated().to_string())
}

fn nm(mm: f64, field: impl FnOnce() -> String) -> Result<String, BlockDiagramFileError> {
    mm_as_stored_nm(mm)
        .map(|nm| nm.to_string())
        .ok_or_else(|| BlockDiagramFileError::Length(field()))
}

/// `text` as a TOML basic string: quote, backslash and every control
/// character escaped, everything else as it is.
pub(super) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c.is_control() && (c as u32) < 0x80 => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
