//! The objects of one sheet, as a person reads them: pure functions over a
//! [`SchematicSheet`], so `resolve` and `find` need no app state. Every
//! object that carries a uuid is one [`Element`] with a `kind` and the human
//! fields that apply to it (designator, value, text, net, position).

use serde_json::{Map, Value, json};
use signex_types::schematic::{LabelType, SchematicSheet};
use uuid::Uuid;

use crate::app::handlers::canvas::net_color_click::{point_on_segment, wire_at};

/// One object of a sheet.
#[derive(Debug, Clone)]
pub(super) struct Element {
    pub id: Uuid,
    pub kind: &'static str,
    pub designator: Option<String>,
    pub value: Option<String>,
    pub text: Option<String>,
    /// The net name the object itself carries (a power port, a label).
    pub own_net: Option<String>,
    pub fields: Map<String, Value>,
}

impl Element {
    pub(super) fn new(id: Uuid, kind: &'static str) -> Self {
        Self {
            id,
            kind,
            designator: None,
            value: None,
            text: None,
            own_net: None,
            fields: Map::new(),
        }
    }

    pub(super) fn at(mut self, x: f64, y: f64) -> Self {
        self.fields.insert("position".into(), json!([x, y]));
        self
    }

    pub(super) fn field(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.fields.insert(name.into(), value.into());
        self
    }

    /// The element as an answer: id, kind and its human fields.
    pub(super) fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id.to_string()));
        map.insert("kind".into(), json!(self.kind));
        for (name, value) in [
            ("designator", &self.designator),
            ("value", &self.value),
            ("text", &self.text),
        ] {
            if let Some(value) = value {
                map.insert(name.into(), json!(value));
            }
        }
        if let Some(net) = &self.own_net {
            map.insert("net".into(), json!(net));
        }
        map.extend(self.fields.clone());
        Value::Object(map)
    }
}

/// Every kind of element a sheet holds, in the words `kind=` takes.
pub(super) const ELEMENT_KINDS: &[&str] = &[
    "symbol",
    "power_port",
    "pin",
    "wire",
    "bus",
    "bus_entry",
    "junction",
    "label",
    "no_connect",
    "text",
    "sheet_entry",
    "sheet_pin",
];

fn label_kind(kind: LabelType) -> &'static str {
    match kind {
        LabelType::Net => "net",
        LabelType::Global => "global",
        LabelType::Hierarchical => "hierarchical",
        LabelType::Power => "power",
    }
}

/// Every element of `sheet`, grouped by kind in a fixed order.
pub(super) fn sheet_elements(sheet: &SchematicSheet) -> Vec<Element> {
    let mut out = Vec::new();
    for symbol in &sheet.symbols {
        let (x, y) = (symbol.position.x, symbol.position.y);
        if symbol.is_power {
            let mut element = Element::new(symbol.uuid, "power_port").at(x, y);
            element.own_net = Some(symbol.value.clone());
            element.value = Some(symbol.value.clone());
            out.push(element.field("lib_id", symbol.lib_id.clone()));
            continue;
        }
        let mut element = Element::new(symbol.uuid, "symbol").at(x, y);
        element.designator = Some(symbol.reference.clone());
        element.value = Some(symbol.value.clone());
        out.push(
            element
                .field("lib_id", symbol.lib_id.clone())
                .field("footprint", symbol.footprint.clone()),
        );
        let lib = sheet.lib_symbols.get(&symbol.lib_id);
        let mut pins: Vec<(&String, &Uuid)> = symbol.pin_uuids.iter().collect();
        pins.sort();
        for (key, id) in pins {
            let name = lib
                .and_then(|lib| {
                    lib.pins
                        .iter()
                        .find(|lp| signex_types::schematic::pin_key(lp) == key.as_str())
                })
                .map(|lp| lp.pin.name.clone());
            // The pin names its symbol; the designator is the symbol's alone,
            // so a search by designator finds the part, not each of its pins.
            out.push(
                Element::new(*id, "pin")
                    .field("number", key.clone())
                    .field("name", name)
                    .field("symbol", symbol.uuid.to_string())
                    .field("symbol_designator", symbol.reference.clone()),
            );
        }
    }
    for wire in &sheet.wires {
        out.push(
            Element::new(wire.uuid, "wire")
                .at(wire.start.x, wire.start.y)
                .field("end", json!([wire.end.x, wire.end.y])),
        );
    }
    for bus in &sheet.buses {
        out.push(
            Element::new(bus.uuid, "bus")
                .at(bus.start.x, bus.start.y)
                .field("end", json!([bus.end.x, bus.end.y])),
        );
    }
    for entry in &sheet.bus_entries {
        out.push(Element::new(entry.uuid, "bus_entry").at(entry.position.x, entry.position.y));
    }
    for junction in &sheet.junctions {
        out.push(
            Element::new(junction.uuid, "junction").at(junction.position.x, junction.position.y),
        );
    }
    for label in &sheet.labels {
        let mut element = Element::new(label.uuid, "label")
            .at(label.position.x, label.position.y)
            .field("label_kind", label_kind(label.label_type));
        element.text = Some(label.text.clone());
        element.own_net = Some(label.text.clone());
        out.push(element);
    }
    for no_connect in &sheet.no_connects {
        out.push(
            Element::new(no_connect.uuid, "no_connect")
                .at(no_connect.position.x, no_connect.position.y),
        );
    }
    for note in &sheet.text_notes {
        let mut element = Element::new(note.uuid, "text").at(note.position.x, note.position.y);
        element.text = Some(note.text.clone());
        out.push(element);
    }
    for child in &sheet.child_sheets {
        let mut element = Element::new(child.uuid, "sheet_entry")
            .at(child.position.x, child.position.y)
            .field("name", child.name.clone())
            .field("filename", child.filename.clone());
        element.text = Some(child.name.clone());
        out.push(element);
        for pin in &child.pins {
            let mut element = Element::new(pin.uuid, "sheet_pin")
                .at(pin.position.x, pin.position.y)
                .field("name", pin.name.clone())
                .field("direction", pin.direction.clone())
                .field("sheet_entry", child.uuid.to_string());
            element.text = Some(pin.name.clone());
            out.push(element);
        }
    }
    out
}

/// The name of the net the wire `wire` is on: the label or power port that
/// touches the net, the smallest when several do; `None` for a net nothing
/// names. Net membership is the connectivity core's, as for a colour pick.
pub(super) fn net_name_of_wire(sheet: &SchematicSheet, wire: Uuid) -> Option<String> {
    let flood = signex_net::flood_net_elements(sheet, wire)?;
    let on_net = |x: f64, y: f64| {
        flood.wires.iter().any(|id| {
            sheet
                .wires
                .iter()
                .find(|w| w.uuid == *id)
                .is_some_and(|w| point_on_segment(x, y, w.start.x, w.start.y, w.end.x, w.end.y))
        })
    };
    sheet
        .labels
        .iter()
        .filter(|l| on_net(l.position.x, l.position.y))
        .map(|l| l.text.clone())
        .chain(
            sheet
                .symbols
                .iter()
                .filter(|s| s.is_power && on_net(s.position.x, s.position.y))
                .map(|s| s.value.clone()),
        )
        .min()
}

/// Every element on the net called `name` (case-insensitive): the labels and
/// power ports with that name, and every wire and junction joined to one.
pub(super) fn elements_on_net(sheet: &SchematicSheet, name: &str) -> Vec<Uuid> {
    let wanted = name.to_lowercase();
    let anchors: Vec<(f64, f64, Uuid)> = sheet
        .labels
        .iter()
        .filter(|l| l.text.to_lowercase() == wanted)
        .map(|l| (l.position.x, l.position.y, l.uuid))
        .chain(
            sheet
                .symbols
                .iter()
                .filter(|s| s.is_power && s.value.to_lowercase() == wanted)
                .map(|s| (s.position.x, s.position.y, s.uuid)),
        )
        .collect();
    let mut ids: Vec<Uuid> = anchors.iter().map(|a| a.2).collect();
    for (x, y, _) in &anchors {
        if let Some(wire) = wire_at(sheet, *x, *y)
            && let Some(flood) = signex_net::flood_net_elements(sheet, wire)
        {
            ids.extend(flood.wires);
            ids.extend(flood.junctions);
        }
    }
    ids.sort();
    ids.dedup();
    ids
}
