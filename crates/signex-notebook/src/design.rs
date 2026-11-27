//! What the design holds that a notebook reference can name: its parts with
//! their pins, and its nets with their pins and sheets (mockup row 196).
//! The app and the command line build one [`DesignIndex`] from the
//! project's sheets and its netlist; the notebook only reads it.

use uuid::Uuid;

/// One part of the design, under the designator the schematic shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DesignPart {
    pub designator: String,
    /// The part's id. A part placed by a sheet used several times carries
    /// the same id under each of its designators.
    pub id: Uuid,
    /// The value field (`4.7µH`), empty when it has none.
    pub value: String,
    /// The part's description (`buck regulator`), empty when it has none.
    pub description: String,
    /// The footprint name, empty when it has none.
    pub footprint: String,
    /// The file name of the sheet the part is on (`power.snxsch`).
    pub sheet: String,
    /// The variants the part is fitted in; empty when the project has none.
    pub fitted_in: Vec<String>,
    /// The project's variant count, so a card can say "fitted in all 3".
    pub variant_count: usize,
    pub pins: Vec<DesignPin>,
}

/// One pin of a part.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DesignPin {
    /// The pin number (`3`).
    pub number: String,
    /// The pin name (`SW`), empty when it has none.
    pub name: String,
    /// The net the pin is on, `None` when it is unconnected.
    pub net: Option<String>,
}

/// A pin named by its part and number: `U3.3`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct PinName {
    pub component: String,
    pub pin: String,
}

impl std::fmt::Display for PinName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.component, self.pin)
    }
}

/// One net of the design.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DesignNet {
    pub name: String,
    /// Every pin on the net, sorted.
    pub pins: Vec<PinName>,
    /// The file names of the sheets the net is on, sorted.
    pub sheets: Vec<String>,
}

impl DesignNet {
    /// The parts with a pin on the net, each once, in pin order.
    pub fn parts(&self) -> Vec<&str> {
        let mut parts: Vec<&str> = Vec::new();
        for pin in &self.pins {
            if !parts.contains(&pin.component.as_str()) {
                parts.push(&pin.component);
            }
        }
        parts
    }
}

/// The parts and nets of one design, sorted by name for lookups.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct DesignIndex {
    parts: Vec<DesignPart>,
    nets: Vec<DesignNet>,
}

impl DesignIndex {
    /// The index of `parts` and `nets`. Each list is sorted by name; a
    /// second part or net under a name already listed is dropped.
    pub fn new(mut parts: Vec<DesignPart>, mut nets: Vec<DesignNet>) -> Self {
        parts.sort_by(|a, b| natural_order(&a.designator, &b.designator));
        parts.dedup_by(|b, a| a.designator == b.designator);
        nets.sort_by(|a, b| natural_order(&a.name, &b.name));
        nets.dedup_by(|b, a| a.name == b.name);
        for net in &mut nets {
            net.pins.sort();
            net.pins.dedup();
            net.sheets.sort();
            net.sheets.dedup();
        }
        Self { parts, nets }
    }

    /// Every part, by designator.
    pub fn parts(&self) -> &[DesignPart] {
        &self.parts
    }

    /// Every net, by name.
    pub fn nets(&self) -> &[DesignNet] {
        &self.nets
    }

    /// The part shown as `designator`.
    pub fn part(&self, designator: &str) -> Option<&DesignPart> {
        self.parts.iter().find(|part| part.designator == designator)
    }

    /// The parts carrying `id`, by designator.
    pub fn parts_with_id(&self, id: Uuid) -> impl Iterator<Item = &DesignPart> {
        self.parts.iter().filter(move |part| part.id == id)
    }

    /// The net named `name`.
    pub fn net(&self, name: &str) -> Option<&DesignNet> {
        self.nets.iter().find(|net| net.name == name)
    }

    /// The nets a part's pins are on, each once, in pin order.
    pub fn nets_of<'a>(&self, part: &'a DesignPart) -> Vec<&'a str> {
        let mut nets: Vec<&str> = Vec::new();
        for net in part.pins.iter().filter_map(|pin| pin.net.as_deref()) {
            if !nets.contains(&net) {
                nets.push(net);
            }
        }
        nets
    }
}

impl DesignPart {
    /// The pin `pin` names: by number first, then by name.
    pub fn pin(&self, pin: &str) -> Option<&DesignPin> {
        self.pins.iter().find(|p| p.number == pin).or_else(|| {
            self.pins
                .iter()
                .find(|p| !p.name.is_empty() && p.name == pin)
        })
    }

    /// The pin numbers, in the part's order.
    pub fn pin_numbers(&self) -> Vec<String> {
        self.pins.iter().map(|pin| pin.number.clone()).collect()
    }
}

/// Names compared so `R2` sorts before `R10`: runs of digits by value,
/// everything else by character.
pub fn natural_order(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (da, ra) = split_digits(a);
                let (db, rb) = split_digits(b);
                let by_value = da
                    .trim_start_matches('0')
                    .len()
                    .cmp(&db.trim_start_matches('0').len())
                    .then_with(|| da.trim_start_matches('0').cmp(db.trim_start_matches('0')))
                    .then_with(|| da.len().cmp(&db.len()));
                if by_value != std::cmp::Ordering::Equal {
                    return by_value;
                }
                (a, b) = (ra, rb);
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
            }
        }
    }
}

fn split_digits(text: &str) -> (&str, &str) {
    let end = text
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map_or(text.len(), |(i, _)| i);
    text.split_at(end)
}
