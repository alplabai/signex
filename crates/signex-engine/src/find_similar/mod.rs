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

/// The comparable properties of `item` on `sheet`, `Kind` first. Empty when
/// the item is not on the sheet.
pub fn properties_of(sheet: &SchematicSheet, item: &SelectedItem) -> Vec<SimilarValue> {
    let id = item.uuid;
    let mut out = vec![val(SimilarProperty::Kind, kind_label(item.kind))];
    match item.kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            let Some(s) = sheet.symbols.iter().find(|s| s.uuid == id) else {
                return Vec::new();
            };
            out.push(val(SimilarProperty::Value, s.value.trim()));
            out.push(val(SimilarProperty::Footprint, s.footprint.trim()));
            out.push(val(SimilarProperty::LibraryPart, s.lib_id.as_str()));
            out.push(val(
                SimilarProperty::DesignatorPrefix,
                alphabetic_prefix(&s.reference),
            ));
        }
        SelectedKind::Label => {
            let Some(l) = sheet.labels.iter().find(|l| l.uuid == id) else {
                return Vec::new();
            };
            out.push(val(SimilarProperty::NetName, l.text.trim()));
            out.push(val(
                SimilarProperty::LabelType,
                format!("{:?}", l.label_type),
            ));
        }
        SelectedKind::TextNote => {
            let Some(t) = sheet.text_notes.iter().find(|t| t.uuid == id) else {
                return Vec::new();
            };
            out.push(val(SimilarProperty::Text, t.text.trim()));
        }
        SelectedKind::SheetPin => {
            let Some(p) = sheet
                .child_sheets
                .iter()
                .flat_map(|c| &c.pins)
                .find(|p| p.uuid == id)
            else {
                return Vec::new();
            };
            out.push(val(SimilarProperty::Name, p.name.trim()));
        }
        SelectedKind::ChildSheet => {
            let Some(c) = sheet.child_sheets.iter().find(|c| c.uuid == id) else {
                return Vec::new();
            };
            out.push(val(SimilarProperty::Name, c.name.trim()));
            out.push(val(SimilarProperty::FileName, c.filename.as_str()));
        }
        _ => {
            if !exists(sheet, item) {
                return Vec::new();
            }
        }
    }
    out
}

/// Whether a kind with no comparable property besides `Kind` is on the sheet.
fn exists(sheet: &SchematicSheet, item: &SelectedItem) -> bool {
    all_items(sheet).any(|i| &i == item)
}

/// Every selectable object on the sheet, in file order within each kind.
pub fn all_items(sheet: &SchematicSheet) -> impl Iterator<Item = SelectedItem> + '_ {
    use SelectedKind as K;
    let item = |uuid, kind| SelectedItem::new(uuid, kind);
    sheet
        .symbols
        .iter()
        .map(move |s| item(s.uuid, K::Symbol))
        .chain(sheet.wires.iter().map(move |w| item(w.uuid, K::Wire)))
        .chain(sheet.buses.iter().map(move |b| item(b.uuid, K::Bus)))
        .chain(
            sheet
                .bus_entries
                .iter()
                .map(move |e| item(e.uuid, K::BusEntry)),
        )
        .chain(
            sheet
                .junctions
                .iter()
                .map(move |j| item(j.uuid, K::Junction)),
        )
        .chain(
            sheet
                .no_connects
                .iter()
                .map(move |n| item(n.uuid, K::NoConnect)),
        )
        .chain(
            sheet
                .no_erc_directives
                .iter()
                .map(move |n| item(n.uuid, K::NoErcDirective)),
        )
        .chain(sheet.labels.iter().map(move |l| item(l.uuid, K::Label)))
        .chain(
            sheet
                .text_notes
                .iter()
                .map(move |t| item(t.uuid, K::TextNote)),
        )
        .chain(sheet.child_sheets.iter().flat_map(move |c| {
            std::iter::once(item(c.uuid, K::ChildSheet))
                .chain(c.pins.iter().map(move |p| item(p.uuid, K::SheetPin)))
        }))
        .chain(
            sheet
                .drawings
                .iter()
                .map(move |d| item(d.uuid(), K::Drawing)),
        )
}

/// Every object on `sheet` whose value for each of `criteria` equals the
/// criterion's. An object that has no such property does not match. Empty
/// criteria match everything on the sheet.
pub fn find_similar(sheet: &SchematicSheet, criteria: &[SimilarValue]) -> Vec<SelectedItem> {
    all_items(sheet)
        .filter(|candidate| {
            let have = properties_of(sheet, candidate);
            criteria.iter().all(|c| have.contains(c))
        })
        .collect()
}

#[cfg(test)]
mod tests;
