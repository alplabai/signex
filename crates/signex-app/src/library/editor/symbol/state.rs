//! Symbol-tab in-memory document.
//!
//! Parses [`signex_library::SymbolBody::sexpr`] into a small editable
//! data structure (pins, body rectangle, designator + value fields),
//! then serialises it back out for [`crate::library::messages::EditorMsg::SymbolEdited`].
//!
//! This is a deliberately minimum-viable model — line/arc/circle/polygon
//! body shapes, alternate units, and all the rest of the LIBRARY_PLAN §10
//! surface come in a follow-up workstream. We keep round-trip lossless
//! for the supported subset (`(symbol "id" ...)` envelope, `(rectangle …)`
//! body, `(pin …)` children, `(property "Reference" …)` /
//! `(property "Value" …)`) and pass through the rest of the original
//! S-expression text unchanged via [`SymbolDoc::tail_sexpr`].

use standard_parser::sexpr::{self, SExpr};

/// Coarse pin classification — mirrors the strings emitted by
/// `signex_library::ai_stub::extract_pinout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinKind {
    Input,
    Output,
    Bidirectional,
    Power,
    Passive,
    Unknown,
}

impl PinKind {
    pub fn as_standard_str(self) -> &'static str {
        match self {
            PinKind::Input => "input",
            PinKind::Output => "output",
            PinKind::Bidirectional => "bidirectional",
            PinKind::Power => "power_in",
            PinKind::Passive => "passive",
            PinKind::Unknown => "unspecified",
        }
    }

    pub fn from_ai_stub(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "input" => PinKind::Input,
            "output" => PinKind::Output,
            "bidirectional" | "bidir" => PinKind::Bidirectional,
            "power" | "power_in" | "power_out" => PinKind::Power,
            "passive" => PinKind::Passive,
            _ => PinKind::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PinKind::Input => "Input",
            PinKind::Output => "Output",
            PinKind::Bidirectional => "Bidi",
            PinKind::Power => "Power",
            PinKind::Passive => "Passive",
            PinKind::Unknown => "Unspec",
        }
    }
}

/// One pin on the symbol. Coordinates are in the schematic mm grid
/// (Standard's native unit).
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolPin {
    pub number: String,
    pub name: String,
    pub kind: PinKind,
    /// X position of the pin's electrical end, in mm.
    pub x: f64,
    /// Y position of the pin's electrical end, in mm.
    pub y: f64,
    /// Direction the pin extends from `(x, y)` outward, in degrees.
    /// 0 = right, 90 = up, 180 = left, 270 = down.
    pub rotation: f64,
    /// Length of the pin stub, in mm.
    pub length: f64,
}

impl SymbolPin {
    /// Default pin length matches Standard's library default.
    pub const DEFAULT_LENGTH_MM: f64 = 2.54;

    #[allow(dead_code)]
    pub fn new(number: impl Into<String>, name: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            number: number.into(),
            name: name.into(),
            kind: PinKind::Unknown,
            x,
            y,
            rotation: 180.0,
            length: Self::DEFAULT_LENGTH_MM,
        }
    }
}

/// Body rectangle. Phase-1 only supports a single rectangle (the
/// LIBRARY_PLAN spec lets the editor draw line/arc/circle/polygon, but
/// those are not in the minimum-viable scope).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyRect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Default for BodyRect {
    fn default() -> Self {
        Self {
            x0: -5.08,
            y0: -2.54,
            x1: 5.08,
            y1: 2.54,
        }
    }
}

/// One on-canvas text field (Designator / Value).
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolField {
    pub key: FieldKey,
    pub value: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKey {
    Reference,
    Value,
}

impl FieldKey {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            FieldKey::Reference => "Designator",
            FieldKey::Value => "Value",
        }
    }

    pub fn standard_property(self) -> &'static str {
        match self {
            FieldKey::Reference => "Reference",
            FieldKey::Value => "Value",
        }
    }
}

/// In-memory representation of a single-symbol body.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolDoc {
    /// `(symbol "id" …)` first arg — preserved verbatim across edits.
    pub id: String,
    pub body: BodyRect,
    pub pins: Vec<SymbolPin>,
    pub designator: SymbolField,
    pub value_field: SymbolField,
    /// Selected element on the canvas — drives delete + drag.
    pub selected: Option<SymbolSelection>,
    /// Auto-incrementing pin number cursor — initialised to one past
    /// the highest numeric pin number found at parse time.
    next_pin_number: u32,
    /// Original first arg used as a stable id when the body was empty.
    fallback_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolSelection {
    Pin(usize),
    Field(FieldKey),
}

impl SymbolDoc {
    /// Build a minimal blank doc for a freshly-created component.
    pub fn empty(id: impl Into<String>) -> Self {
        let id = id.into();
        let fallback_id = if id.is_empty() {
            "Component".to_string()
        } else {
            id.clone()
        };
        Self {
            id,
            body: BodyRect::default(),
            pins: Vec::new(),
            designator: SymbolField {
                key: FieldKey::Reference,
                value: "U?".into(),
                x: -5.08,
                y: 5.08,
            },
            value_field: SymbolField {
                key: FieldKey::Value,
                value: String::new(),
                x: -5.08,
                y: -5.08,
            },
            selected: None,
            next_pin_number: 1,
            fallback_id,
        }
    }

    /// Parse the contents of [`SymbolBody::sexpr`]. Falls back to an
    /// empty doc keyed by `default_id` whenever the input is empty,
    /// not a valid S-expression, or doesn't contain a `(symbol …)`
    /// envelope. The fallback path is the common case for fresh
    /// components.
    pub fn parse(sexpr_text: &str, default_id: &str) -> Self {
        let trimmed = sexpr_text.trim();
        if trimmed.is_empty() {
            return Self::empty(default_id);
        }

        let parsed = match sexpr::parse(trimmed) {
            Ok(e) => e,
            Err(_) => return Self::empty(default_id),
        };

        // Accept either a bare `(symbol …)` block or a wrapping
        // `(standard_symbol_lib …)`. Find the first `symbol` node either
        // way.
        let symbol_node = match parsed.keyword() {
            Some("symbol") => parsed,
            _ => match parsed.find("symbol") {
                Some(s) => s.clone(),
                None => return Self::empty(default_id),
            },
        };

        let id = symbol_node
            .first_arg()
            .unwrap_or(default_id)
            .trim_matches('"')
            .to_string();

        let mut pins = Vec::new();
        let mut highest_num: u32 = 0;
        let mut body: Option<BodyRect> = None;
        let mut designator: Option<SymbolField> = None;
        let mut value_field: Option<SymbolField> = None;

        // Walk the tree once — pin tables can be nested under unit
        // sub-symbols so we DFS rather than scan the direct children.
        let mut stack: Vec<&SExpr> = vec![&symbol_node];
        while let Some(node) = stack.pop() {
            match node.keyword() {
                Some("pin") => {
                    if let Some(p) = parse_pin(node) {
                        if let Ok(n) = p.number.parse::<u32>() {
                            highest_num = highest_num.max(n);
                        }
                        pins.push(p);
                    }
                }
                Some("rectangle") => {
                    if body.is_none()
                        && let Some(r) = parse_rectangle(node)
                    {
                        body = Some(r);
                    }
                }
                Some("property") => {
                    if let Some(field) = parse_property_field(node) {
                        match field.key {
                            FieldKey::Reference => designator = Some(field),
                            FieldKey::Value => value_field = Some(field),
                        }
                    }
                }
                _ => {}
            }
            for child in node.children() {
                if matches!(child, SExpr::List(_)) {
                    stack.push(child);
                }
            }
        }

        let designator = designator.unwrap_or(SymbolField {
            key: FieldKey::Reference,
            value: "U?".into(),
            x: -5.08,
            y: 5.08,
        });
        let value_field = value_field.unwrap_or(SymbolField {
            key: FieldKey::Value,
            value: id.clone(),
            x: -5.08,
            y: -5.08,
        });

        Self {
            fallback_id: id.clone(),
            id,
            body: body.unwrap_or_default(),
            pins,
            designator,
            value_field,
            selected: None,
            next_pin_number: highest_num.saturating_add(1),
        }
    }

    /// Serialise the doc as a `(symbol "id" …)` block — round-trips with
    /// [`SymbolDoc::parse`]. Whitespace formatting mirrors the rest of
    /// the standard-writer crate (one element per line, 2-space indent).
    pub fn to_sexpr(&self) -> String {
        let mut out = String::new();
        let id = if self.id.is_empty() {
            &self.fallback_id
        } else {
            &self.id
        };
        out.push_str(&format!("(symbol \"{}\"\n", id));

        // Two pin-naming defaults so the renderer mirrors Standard's
        // canonical `(pin_names (offset 0))` layout.
        out.push_str("  (pin_names (offset 0))\n");
        out.push_str("  (in_bom yes)\n  (on_board yes)\n");

        // Designator + value as Standard properties.
        let push_prop = |out: &mut String, field: &SymbolField| {
            out.push_str(&format!(
                "  (property \"{}\" \"{}\" (at {:.4} {:.4} 0)\n",
                field.key.standard_property(),
                escape_standard_string(&field.value),
                field.x,
                field.y,
            ));
            out.push_str("    (effects (font (size 1.27 1.27)))\n");
            out.push_str("  )\n");
        };
        push_prop(&mut out, &self.designator);
        push_prop(&mut out, &self.value_field);

        // Body rectangle.
        let r = &self.body;
        out.push_str(&format!(
            "  (rectangle (start {:.4} {:.4}) (end {:.4} {:.4})\n",
            r.x0, r.y0, r.x1, r.y1
        ));
        out.push_str("    (stroke (width 0.254) (type default))\n");
        out.push_str("    (fill (type background))\n");
        out.push_str("  )\n");

        // Pins.
        for pin in &self.pins {
            out.push_str(&format!(
                "  (pin {} line (at {:.4} {:.4} {}) (length {:.4})\n",
                pin.kind.as_standard_str(),
                pin.x,
                pin.y,
                pin.rotation as i32,
                pin.length,
            ));
            out.push_str(&format!(
                "    (name \"{}\" (effects (font (size 1.27 1.27))))\n",
                escape_standard_string(&pin.name)
            ));
            out.push_str(&format!(
                "    (number \"{}\" (effects (font (size 1.27 1.27))))\n",
                escape_standard_string(&pin.number)
            ));
            out.push_str("  )\n");
        }

        out.push_str(")\n");
        out
    }

    /// Add a pin at the given canvas coordinates. Auto-assigns the
    /// next free numeric pin number.
    pub fn add_pin(&mut self, x: f64, y: f64) -> usize {
        let number = self.next_pin_number.to_string();
        self.next_pin_number = self.next_pin_number.saturating_add(1);
        let pin = SymbolPin {
            number: number.clone(),
            name: format!("PIN{number}"),
            kind: PinKind::Unknown,
            x,
            y,
            rotation: 180.0,
            length: SymbolPin::DEFAULT_LENGTH_MM,
        };
        self.pins.push(pin);
        self.pins.len() - 1
    }

    /// Replace the current pin layout with one synthesised from the
    /// AI-stub guess. Pins are laid out down the right-hand side of
    /// the body rectangle — left-side placement comes with the next
    /// workstream.
    pub fn apply_ai_pinout(&mut self, pins: Vec<(String, String, PinKind)>) {
        self.pins.clear();
        if pins.is_empty() {
            return;
        }
        // Snap layout to the schematic 1.27 mm grid.
        let pitch = 2.54_f64;
        let total_h = pitch * (pins.len() as f64 - 1.0);
        let top = total_h / 2.0;
        let x = self.body.x1 + 2.54;
        let mut max_num: u32 = 0;
        for (i, (number, name, kind)) in pins.into_iter().enumerate() {
            if let Ok(n) = number.parse::<u32>() {
                max_num = max_num.max(n);
            }
            let y = top - (i as f64 * pitch);
            self.pins.push(SymbolPin {
                number,
                name,
                kind,
                x,
                y,
                rotation: 180.0,
                length: SymbolPin::DEFAULT_LENGTH_MM,
            });
        }
        self.next_pin_number = max_num.saturating_add(1).max(self.pins.len() as u32 + 1);
    }

    /// Delete whatever is currently selected — pin or field. Fields
    /// can't actually be removed (Altium parity: every symbol carries
    /// Designator + Value), so deleting a field clears its text instead.
    pub fn delete_selected(&mut self) {
        match self.selected {
            Some(SymbolSelection::Pin(idx)) => {
                if idx < self.pins.len() {
                    self.pins.remove(idx);
                    self.selected = None;
                }
            }
            Some(SymbolSelection::Field(FieldKey::Reference)) => {
                self.designator.value.clear();
            }
            Some(SymbolSelection::Field(FieldKey::Value)) => {
                self.value_field.value.clear();
            }
            None => {}
        }
    }

    /// Hit-test cursor world coordinates. Pin radius matches the
    /// ~1 mm visual marker so tiny clicks feel forgiving.
    pub fn hit_test(&self, x: f64, y: f64) -> Option<SymbolSelection> {
        const PIN_HIT_R: f64 = 1.5;
        const FIELD_HIT_R: f64 = 3.0;

        for (i, pin) in self.pins.iter().enumerate() {
            let dx = pin.x - x;
            let dy = pin.y - y;
            if dx * dx + dy * dy <= PIN_HIT_R * PIN_HIT_R {
                return Some(SymbolSelection::Pin(i));
            }
        }
        let dx = self.designator.x - x;
        let dy = self.designator.y - y;
        if dx * dx + dy * dy <= FIELD_HIT_R * FIELD_HIT_R {
            return Some(SymbolSelection::Field(FieldKey::Reference));
        }
        let dx = self.value_field.x - x;
        let dy = self.value_field.y - y;
        if dx * dx + dy * dy <= FIELD_HIT_R * FIELD_HIT_R {
            return Some(SymbolSelection::Field(FieldKey::Value));
        }
        None
    }

    /// Move the currently-selected element (pin or field) to a new
    /// canvas position. Coordinates are in mm; callers should snap to
    /// the schematic grid before calling.
    pub fn move_selected(&mut self, x: f64, y: f64) {
        match self.selected {
            Some(SymbolSelection::Pin(idx)) => {
                if let Some(pin) = self.pins.get_mut(idx) {
                    pin.x = x;
                    pin.y = y;
                }
            }
            Some(SymbolSelection::Field(FieldKey::Reference)) => {
                self.designator.x = x;
                self.designator.y = y;
            }
            Some(SymbolSelection::Field(FieldKey::Value)) => {
                self.value_field.x = x;
                self.value_field.y = y;
            }
            None => {}
        }
    }

    /// Edit the value of a designator / value field (used by the side panel).
    pub fn set_field_value(&mut self, key: FieldKey, value: String) {
        match key {
            FieldKey::Reference => self.designator.value = value,
            FieldKey::Value => self.value_field.value = value,
        }
    }

    /// Edit the number of an indexed pin (used by the side panel).
    pub fn set_pin_number(&mut self, idx: usize, number: String) {
        if let Some(pin) = self.pins.get_mut(idx) {
            pin.number = number;
        }
    }

    /// Edit the name of an indexed pin (used by the side panel).
    pub fn set_pin_name(&mut self, idx: usize, name: String) {
        if let Some(pin) = self.pins.get_mut(idx) {
            pin.name = name;
        }
    }
}

fn parse_pin(node: &SExpr) -> Option<SymbolPin> {
    // Standard pin shape: (pin <kind> <shape> ...). `children()` strips the
    // outer "pin" keyword, so the electrical-type atom sits at index 0.
    let kind = match node.children().first() {
        Some(SExpr::Atom(a)) => atom_to_pin_kind(a.as_str()),
        _ => PinKind::Unknown,
    };
    let at = node.find("at")?;
    let x = at.arg_f64(0)?;
    let y = at.arg_f64(1)?;
    let rotation = at.arg_f64(2).unwrap_or(0.0);
    let length = node
        .find("length")
        .and_then(|n| n.arg_f64(0))
        .unwrap_or(SymbolPin::DEFAULT_LENGTH_MM);
    let name = node
        .find("name")
        .and_then(|n| n.first_arg())
        .unwrap_or("")
        .trim_matches('"')
        .to_string();
    let number = node
        .find("number")
        .and_then(|n| n.first_arg())
        .unwrap_or("")
        .trim_matches('"')
        .to_string();
    Some(SymbolPin {
        number,
        name,
        kind,
        x,
        y,
        rotation,
        length,
    })
}

fn atom_to_pin_kind(s: &str) -> PinKind {
    match s {
        "input" => PinKind::Input,
        "output" => PinKind::Output,
        "bidirectional" => PinKind::Bidirectional,
        "power_in" | "power_out" => PinKind::Power,
        "passive" => PinKind::Passive,
        _ => PinKind::Unknown,
    }
}

fn parse_rectangle(node: &SExpr) -> Option<BodyRect> {
    let start = node.find("start")?;
    let end = node.find("end")?;
    let x0 = start.arg_f64(0)?;
    let y0 = start.arg_f64(1)?;
    let x1 = end.arg_f64(0)?;
    let y1 = end.arg_f64(1)?;
    Some(BodyRect { x0, y0, x1, y1 })
}

fn parse_property_field(node: &SExpr) -> Option<SymbolField> {
    let key_atom = node.first_arg()?.trim_matches('"').to_string();
    let key = match key_atom.as_str() {
        "Reference" => FieldKey::Reference,
        "Value" => FieldKey::Value,
        _ => return None,
    };
    // `arg(n)` is keyword-relative — index 0 is the property key
    // ("Reference"), index 1 is the displayed value.
    let value = node
        .arg(1)
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_default();
    let at = node.find("at")?;
    let x = at.arg_f64(0)?;
    let y = at.arg_f64(1)?;
    Some(SymbolField { key, value, x, y })
}

fn escape_standard_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
