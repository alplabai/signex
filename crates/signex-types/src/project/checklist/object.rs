//! What a design-object link names (mockup row 206: "a part, a pin, a net, a
//! sheet, a board object; click cross-probes"). The link keeps the text as
//! typed or picked; read here it is `part:<designator>`, `pin:<part>.<pin>`,
//! `net:<name>`, `sheet:<file>` or `board`, and any other text is matched
//! as a designator, then as a net name. A sheet answers which of its items
//! the target names, and which parts and nets it offers the link picker.

use crate::schematic::{SchematicSheet, SelectedItem, SelectedKind};

/// What an object link names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectTarget {
    /// A part by designator.
    Part(String),
    /// A pin of a part: the part is what is shown.
    Pin { part: String, pin: String },
    /// A net by name.
    Net(String),
    /// A sheet by file name.
    Sheet(String),
    /// The project's board.
    Board,
    /// Text in none of the forms above.
    Text(String),
}

/// `rest` trimmed, when that leaves something.
fn named(rest: &str) -> Option<String> {
    let rest = rest.trim();
    (!rest.is_empty()).then(|| rest.to_string())
}

impl ObjectTarget {
    /// The target `text` names.
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        if text == "board" {
            return Self::Board;
        }
        let parsed = match text.split_once(':') {
            Some(("part", rest)) => named(rest).map(Self::Part),
            Some(("net", rest)) => named(rest).map(Self::Net),
            Some(("sheet", rest)) => named(rest).map(Self::Sheet),
            Some(("pin", rest)) => rest.trim().split_once('.').and_then(|(part, pin)| {
                Some(Self::Pin {
                    part: named(part)?,
                    pin: named(pin)?,
                })
            }),
            _ => None,
        };
        parsed.unwrap_or_else(|| Self::Text(text.to_string()))
    }

    /// The text [`Self::parse`] reads back as this target.
    pub fn to_text(&self) -> String {
        match self {
            Self::Part(part) => format!("part:{part}"),
            Self::Pin { part, pin } => format!("pin:{part}.{pin}"),
            Self::Net(net) => format!("net:{net}"),
            Self::Sheet(sheet) => format!("sheet:{sheet}"),
            Self::Board => "board".into(),
            Self::Text(text) => text.clone(),
        }
    }

    /// The item on `sheet` the target names: a part's or a pin's symbol, a
    /// net's first label. Free text is tried as a designator (whole, then
    /// its first word), then as a net name. A sheet or the board names no
    /// item.
    pub fn find_on(&self, sheet: &SchematicSheet) -> Option<SelectedItem> {
        match self {
            Self::Part(part) | Self::Pin { part, .. } => part_on(sheet, part),
            Self::Net(net) => net_on(sheet, net),
            Self::Sheet(_) | Self::Board => None,
            Self::Text(text) => {
                let first = text
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .find(|word| !word.is_empty())
                    .unwrap_or_default();
                part_on(sheet, text)
                    .or_else(|| part_on(sheet, first))
                    .or_else(|| net_on(sheet, text))
            }
        }
    }
}

/// The symbol with designator `part` on `sheet`.
fn part_on(sheet: &SchematicSheet, part: &str) -> Option<SelectedItem> {
    let part = part.trim();
    if part.is_empty() {
        return None;
    }
    sheet
        .symbols
        .iter()
        .find(|symbol| symbol.reference == part)
        .map(|symbol| SelectedItem::new(symbol.uuid, SelectedKind::Symbol))
}

/// The first label named `net` on `sheet`.
fn net_on(sheet: &SchematicSheet, net: &str) -> Option<SelectedItem> {
    let net = net.trim();
    if net.is_empty() {
        return None;
    }
    sheet
        .labels
        .iter()
        .find(|label| label.text == net)
        .map(|label| SelectedItem::new(label.uuid, SelectedKind::Label))
}

/// The parts and nets of a sheet a link can name, each sorted and once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetTargets {
    pub parts: Vec<String>,
    pub nets: Vec<String>,
}

/// What `sheet` offers the link picker: its annotated designators (power
/// ports, `#…`, and unannotated `…?` left out) and its label names.
pub fn sheet_targets(sheet: &SchematicSheet) -> SheetTargets {
    let mut parts: Vec<String> = sheet
        .symbols
        .iter()
        .map(|symbol| symbol.reference.trim())
        .filter(|part| !part.is_empty() && !part.starts_with('#') && !part.ends_with('?'))
        .map(str::to_string)
        .collect();
    parts.sort_unstable();
    parts.dedup();
    let mut nets: Vec<String> = sheet
        .labels
        .iter()
        .map(|label| label.text.trim())
        .filter(|net| !net.is_empty())
        .map(str::to_string)
        .collect();
    nets.sort_unstable();
    nets.dedup();
    SheetTargets { parts, nets }
}
