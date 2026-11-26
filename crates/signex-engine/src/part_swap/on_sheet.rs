//! A pin or gate swap started on the schematic (#1261). Pure: it reads one
//! sheet and returns the sheet as it should be, which the caller installs as
//! one undo step. Only the sheet changes; Update PCB then gives the pads the
//! nets their pins now have (`crate::pcb::eco`), so the swap reaches the
//! board through the change list like any other schematic edit.
//!
//! Which pins and gates may trade is the part's rule
//! (`signex_types::part_swap`), the same one a swap made on the board reads,
//! and the sheet is rewired by the same functions (`rewire_pins`,
//! `swap_units`). Rules: `docs/PCB_TOOLS.md`, "Pin and gate swap".

use signex_types::part_swap::SwapGroups;
use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol};
use uuid::Uuid;

use super::{PlacedPin, SheetSwapRefusal, gate_pin_pairs, rewire_pins, swap_units, unit_of_pin};

/// What a swap started on the schematic trades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetSwapRequest {
    /// Two pins of the part, by number.
    Pins(String, String),
    /// Two gates of the part, by unit number.
    Gates(u32, u32),
}

/// A swap started on the schematic, planned and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSwap {
    pub designator: String,
    /// Each pin whose net moves, with the pin it trades with: the two pins,
    /// or every pin pair of the two gates.
    pub pairs: Vec<(String, String)>,
    /// The sheet after the swap; `None` when nothing ends on either pin.
    pub sheet: Option<SchematicSheet>,
}

/// The part `symbol` (any placed unit of it) with the swap `request` made on
/// `sheet`. Refused, with nothing built, for a symbol that is not on the
/// sheet, a locked unit, members outside one swap group of the part, gates
/// not both placed on this sheet or not drawn alike, and anything
/// `rewire_pins` refuses.
pub fn swap_on_sheet(
    sheet: &SchematicSheet,
    symbol: Uuid,
    request: &SheetSwapRequest,
) -> Result<SheetSwap, SheetSwapRefusal> {
    let part = PlacedPart::of(sheet, symbol)?;
    if let Some(locked) = part.units.iter().find(|s| s.locked) {
        return Err(SheetSwapRefusal::SymbolLocked(locked.reference.clone()));
    }
    let designator = part.designator();
    match request {
        SheetSwapRequest::Pins(a, b) => {
            check_pin_group(part.first(), &designator, a, b)?;
            let pin_a = PlacedPin {
                symbol: unit_holding(&part.units, &part.lib, a, &designator)?,
                number: a,
            };
            let pin_b = PlacedPin {
                symbol: unit_holding(&part.units, &part.lib, b, &designator)?,
                number: b,
            };
            let rewired = rewire_pins(sheet, pin_a, pin_b)?;
            Ok(SheetSwap {
                designator,
                pairs: vec![(a.clone(), b.clone())],
                sheet: rewired,
            })
        }
        SheetSwapRequest::Gates(a, b) => {
            check_gate_group(part.first(), &designator, *a, *b)?;
            let pairs = gate_pin_pairs(&part.lib, &designator, *a, *b)?;
            let placed = |unit: u32| part.units.iter().find(|s| s.unit == unit);
            let (Some(at_a), Some(at_b)) = (placed(*a), placed(*b)) else {
                let missing = if placed(*a).is_none() { *a } else { *b };
                return Err(SheetSwapRefusal::GateNotPlaced {
                    designator,
                    unit: missing,
                });
            };
            let swapped = swap_units(sheet, &[(at_a.uuid, *b), (at_b.uuid, *a)])?;
            Ok(SheetSwap {
                designator,
                pairs,
                sheet: Some(swapped),
            })
        }
    }
}

/// The pin swap a click on pin `pin` of `symbol` asks for: that pin and the
/// one other member of its pin swap group. Refused when the pin is in no
/// group, or its group offers more than one pin to trade with (the caller
/// then names both pins).
pub fn pins_of_clicked_pin(
    sheet: &SchematicSheet,
    symbol: Uuid,
    pin: &str,
) -> Result<SheetSwapRequest, SheetSwapRefusal> {
    let part = PlacedPart::of(sheet, symbol)?;
    let designator = part.designator();
    let groups = SwapGroups::pins_of(part.first());
    let others = partners(&groups, pin);
    match others.as_slice() {
        [] => Err(SheetSwapRefusal::PinInNoGroup {
            designator,
            pin: pin.trim().to_string(),
        }),
        [other] => Ok(SheetSwapRequest::Pins(
            pin.trim().to_string(),
            other.clone(),
        )),
        many => Err(SheetSwapRefusal::ManyPartners {
            designator,
            member: format!("pin {}", pin.trim()),
            others: many.join(", "),
        }),
    }
}

/// The gate swap a selection of placed units asks for: two units of one
/// part trade with each other; one unit trades with the one other gate of
/// its gate swap group placed on the sheet. `selected` lists the selected
/// symbols; the answer names the part by its first unit's uuid.
pub fn gates_of_selection(
    sheet: &SchematicSheet,
    selected: &[Uuid],
) -> Result<(Uuid, SheetSwapRequest), SheetSwapRefusal> {
    let Some(first) = selected.first() else {
        return Err(SheetSwapRefusal::NotOnePart);
    };
    let part = PlacedPart::of(sheet, *first)?;
    let designator = part.designator();
    let unit_of = |id: &Uuid| part.units.iter().find(|s| s.uuid == *id).map(|s| s.unit);
    let units: Option<Vec<u32>> = selected.iter().map(unit_of).collect();
    match units.as_deref() {
        Some([a, b]) if a != b => Ok((*first, SheetSwapRequest::Gates(*a, *b))),
        Some([a]) => {
            let groups = SwapGroups::gates_of(part.first());
            let placed: Vec<String> = partners(&groups, &a.to_string())
                .into_iter()
                .filter(|other| part.units.iter().any(|s| s.unit.to_string() == *other))
                .collect();
            match placed.as_slice() {
                [] => Err(SheetSwapRefusal::GateInNoGroup {
                    designator,
                    unit: *a,
                }),
                [other] => {
                    let b = other.parse().map_err(|_| SheetSwapRefusal::NotOnePart)?;
                    Ok((*first, SheetSwapRequest::Gates(*a, b)))
                }
                many => Err(SheetSwapRefusal::ManyPartners {
                    designator,
                    member: format!("gate {a}"),
                    others: many.join(", "),
                }),
            }
        }
        _ => Err(SheetSwapRefusal::NotOnePart),
    }
}

/// Refused unless pins `a` and `b` are two members of one pin swap group of
/// the part `first` is a unit of.
pub fn check_pin_group(
    first: &Symbol,
    designator: &str,
    a: &str,
    b: &str,
) -> Result<(), SheetSwapRefusal> {
    if SwapGroups::pins_of(first).same_group(a, b) {
        return Ok(());
    }
    Err(SheetSwapRefusal::PinsNotGrouped {
        designator: designator.to_string(),
        a: a.to_string(),
        b: b.to_string(),
    })
}

/// Refused unless gates `a` and `b` are two members of one gate swap group
/// of the part `first` is a unit of.
pub fn check_gate_group(
    first: &Symbol,
    designator: &str,
    a: u32,
    b: u32,
) -> Result<(), SheetSwapRefusal> {
    if SwapGroups::gates_of(first).same_group(&a.to_string(), &b.to_string()) {
        return Ok(());
    }
    Err(SheetSwapRefusal::GatesNotGrouped {
        designator: designator.to_string(),
        a,
        b,
    })
}

/// The placed unit that draws pin `pin`: the unit of its gate, or the first
/// unit for a pin every gate shares.
pub fn unit_holding(
    units: &[&Symbol],
    lib: &LibSymbol,
    pin: &str,
    designator: &str,
) -> Result<Uuid, SheetSwapRefusal> {
    let unit = unit_of_pin(lib, pin).ok_or_else(|| SheetSwapRefusal::UnknownPin {
        designator: designator.to_string(),
        pin: pin.to_string(),
    })?;
    units
        .iter()
        .find(|s| unit == 0 || s.unit == unit)
        .map(|s| s.uuid)
        .ok_or_else(|| SheetSwapRefusal::PinGateNotPlaced {
            designator: designator.to_string(),
            pin: pin.to_string(),
        })
}

/// The other members of the group that lists `member`.
fn partners(groups: &SwapGroups, member: &str) -> Vec<String> {
    let member = member.trim();
    groups
        .group_of(member)
        .map(|i| {
            groups.groups()[i]
                .iter()
                .filter(|m| m.as_str() != member)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The placed units of one part on a sheet, with its library drawing.
struct PlacedPart<'a> {
    units: Vec<&'a Symbol>,
    lib: LibSymbol,
}

impl<'a> PlacedPart<'a> {
    fn of(sheet: &'a SchematicSheet, symbol: Uuid) -> Result<Self, SheetSwapRefusal> {
        let units: Vec<&Symbol> = crate::placed_unit_ids(sheet, symbol)
            .into_iter()
            .filter_map(|id| sheet.symbols.iter().find(|s| s.uuid == id))
            .filter(|s| !s.is_power)
            .collect();
        let first = *units.first().ok_or(SheetSwapRefusal::NoSymbol(symbol))?;
        let lib = sheet
            .resolve_lib_symbol(first)
            .map(|lib| lib.into_owned())
            .ok_or_else(|| SheetSwapRefusal::NoLibrarySymbol(first.reference.clone()))?;
        Ok(Self { units, lib })
    }

    fn first(&self) -> &'a Symbol {
        self.units[0]
    }

    fn designator(&self) -> String {
        self.first().reference.clone()
    }
}

#[cfg(test)]
#[path = "on_sheet_tests.rs"]
mod tests;
