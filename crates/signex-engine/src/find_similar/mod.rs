//! Find Similar Objects (#1257): pick a seed object, say which of its
//! properties must match (Same) and which are free (Any), and get every
//! object on a sheet that agrees on the Same ones.
//!
//! Matching works on property *values*, not on the seed's identity, so the
//! same criteria can be run over any other sheet of the project.

use signex_types::designator::alphabetic_prefix;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

/// One comparable property of a schematic object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SimilarProperty {
    /// What the object is: a symbol, a wire, a label...
    Kind,
    Value,
    Footprint,
    /// The library symbol the part was placed from.
    LibraryPart,
    /// The letters a designator starts with (`R` for `R12`).
    DesignatorPrefix,
    /// The text of a net, global, hierarchical or power label.
    NetName,
    /// Net, global, hierarchical or power.
    LabelType,
    /// The text of a text note.
    Text,
    /// A sheet symbol's or sheet pin's name.
    Name,
    /// The file a sheet symbol points at.
    FileName,
}

impl SimilarProperty {
    /// The name the dialog shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Kind => "Kind",
            Self::Value => "Value",
            Self::Footprint => "Footprint",
            Self::LibraryPart => "Library part",
            Self::DesignatorPrefix => "Designator prefix",
            Self::NetName => "Net name",
            Self::LabelType => "Label type",
            Self::Text => "Text",
            Self::Name => "Name",
            Self::FileName => "File name",
        }
    }
}

/// A property and the value it has on one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimilarValue {
    pub property: SimilarProperty,
    pub value: String,
}

fn val(property: SimilarProperty, value: impl Into<String>) -> SimilarValue {
    SimilarValue {
        property,
        value: value.into(),
    }
}

/// The human name of an object kind.
pub fn kind_label(kind: SelectedKind) -> &'static str {
    match kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            "Symbol"
        }
        SelectedKind::Wire => "Wire",
        SelectedKind::Bus => "Bus",
        SelectedKind::BusEntry => "Bus entry",
        SelectedKind::Junction => "Junction",
        SelectedKind::NoConnect => "No connect",
        SelectedKind::NoErcDirective => "No ERC directive",
        SelectedKind::Label => "Label",
        SelectedKind::SheetPin => "Sheet pin",
        SelectedKind::TextNote => "Text note",
        SelectedKind::ChildSheet => "Sheet symbol",
        SelectedKind::Drawing => "Drawing",
    }
}

/// What to call `item` in the dialog: its designator or text where it has
/// one (`Symbol R1`, `Label VCC`), else its kind.
pub fn describe(sheet: &SchematicSheet, item: &SelectedItem) -> String {
    let kind = kind_label(item.kind);
    let name = match item.kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => sheet
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .map(|s| s.reference.clone()),
        SelectedKind::Label => sheet
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)
            .map(|l| l.text.clone()),
        SelectedKind::TextNote => sheet
            .text_notes
            .iter()
            .find(|t| t.uuid == item.uuid)
            .map(|t| t.text.clone()),
        SelectedKind::ChildSheet => sheet
            .child_sheets
            .iter()
            .find(|c| c.uuid == item.uuid)
            .map(|c| c.name.clone()),
        _ => None,
    };
    match name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => format!("{kind} {name}"),
        _ => kind.to_owned(),
    }
}

/// Every selectable object on the sheet with its comparable properties,
/// built in one pass over the sheet. File order within each kind.
fn entries(sheet: &SchematicSheet) -> Vec<(SelectedItem, Vec<SimilarValue>)> {
    use SelectedKind as K;
    let mut out = Vec::new();
    let mut push = |uuid, kind, extra: Vec<SimilarValue>| {
        let mut props = vec![val(SimilarProperty::Kind, kind_label(kind))];
        props.extend(extra);
        out.push((SelectedItem::new(uuid, kind), props));
    };
    for s in &sheet.symbols {
        push(
            s.uuid,
            K::Symbol,
            vec![
                val(SimilarProperty::Value, s.value.trim()),
                val(SimilarProperty::Footprint, s.footprint.trim()),
                val(SimilarProperty::LibraryPart, s.lib_id.as_str()),
                val(
                    SimilarProperty::DesignatorPrefix,
                    alphabetic_prefix(&s.reference),
                ),
            ],
        );
    }
    for w in &sheet.wires {
        push(w.uuid, K::Wire, Vec::new());
    }
    for b in &sheet.buses {
        push(b.uuid, K::Bus, Vec::new());
    }
    for e in &sheet.bus_entries {
        push(e.uuid, K::BusEntry, Vec::new());
    }
    for j in &sheet.junctions {
        push(j.uuid, K::Junction, Vec::new());
    }
    for n in &sheet.no_connects {
        push(n.uuid, K::NoConnect, Vec::new());
    }
    for n in &sheet.no_erc_directives {
        push(n.uuid, K::NoErcDirective, Vec::new());
    }
    for l in &sheet.labels {
        push(
            l.uuid,
            K::Label,
            vec![
                val(SimilarProperty::NetName, l.text.trim()),
                val(SimilarProperty::LabelType, format!("{:?}", l.label_type)),
            ],
        );
    }
    for t in &sheet.text_notes {
        push(
            t.uuid,
            K::TextNote,
            vec![val(SimilarProperty::Text, t.text.trim())],
        );
    }
    for c in &sheet.child_sheets {
        push(
            c.uuid,
            K::ChildSheet,
            vec![
                val(SimilarProperty::Name, c.name.trim()),
                val(SimilarProperty::FileName, c.filename.as_str()),
            ],
        );
        for p in &c.pins {
            push(
                p.uuid,
                K::SheetPin,
                vec![val(SimilarProperty::Name, p.name.trim())],
            );
        }
    }
    for d in &sheet.drawings {
        push(d.uuid(), K::Drawing, Vec::new());
    }
    out
}

/// The comparable properties of `item` on `sheet`, `Kind` first. Empty when
/// the item is not on the sheet. A symbol's reference and value fields stand
/// for the symbol.
pub fn properties_of(sheet: &SchematicSheet, item: &SelectedItem) -> Vec<SimilarValue> {
    let kind = match item.kind {
        SelectedKind::SymbolRefField | SelectedKind::SymbolValField => SelectedKind::Symbol,
        other => other,
    };
    let wanted = SelectedItem::new(item.uuid, kind);
    entries(sheet)
        .into_iter()
        .find_map(|(i, props)| (i == wanted).then_some(props))
        .unwrap_or_default()
}

/// Every object on `sheet` whose value for each of `criteria` equals the
/// criterion's. An object that has no such property does not match. Empty
/// criteria match everything on the sheet. One pass over the sheet.
pub fn find_similar(sheet: &SchematicSheet, criteria: &[SimilarValue]) -> Vec<SelectedItem> {
    entries(sheet)
        .into_iter()
        .filter(|(_, have)| criteria.iter().all(|c| have.contains(c)))
        .map(|(item, _)| item)
        .collect()
}

#[cfg(test)]
mod tests;
