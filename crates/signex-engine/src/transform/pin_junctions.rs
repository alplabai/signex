//! A pin end that lands on the middle of a wire joins it through a junction
//! dot (maintainer decision 2026-11-18). The netlist still connects only at
//! wire ends, dots, labels and no-connects (#107); the edit that makes the
//! contact (placing or moving a part, drawing or moving a wire) mints the
//! dot, in its own undo step, through `reconcile_wire_junctions`. A sheet that
//! already has such a bare contact is never changed on open: it is reported,
//! and [`pin_junction_repairs`] is the one-step repair.

use std::collections::{BTreeMap, HashMap, HashSet};

use signex_net::{SheetConnectivity, point_on_segment, pt_key, uf::Key};
use signex_types::object_id::IdSource;
use signex_types::schematic::{
    Junction, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, SymbolTransform, Wire,
    instance_pins,
};

use crate::Command;

/// The pin tips of `symbol` on `sheet` (its own unit), empty when its library
/// body is missing.
pub(crate) fn symbol_pin_tips(sheet: &SchematicSheet, symbol: &Symbol) -> Vec<Point> {
    let Some(lib) = sheet.resolve_lib_symbol(symbol) else {
        return Vec::new();
    };
    let xform = SymbolTransform::from_symbol(symbol);
    instance_pins(&lib, symbol)
        .map(|lib_pin| xform.apply(lib_pin.pin.position))
        .collect()
}

/// Whether `wire` holds a pin end of `tips` other than the one at `k`, end or
/// interior.
fn holds_another(wire: &Wire, k: Key, tips: &[Point]) -> bool {
    let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
    tips.iter()
        .map(pt_key)
        .any(|t| t != k && point_on_segment(t, a, b))
}

/// The nets of a sheet, built only when a tap needs them.
struct Nets<'s> {
    sheet: &'s SchematicSheet,
    cache: Option<(SheetConnectivity, HashSet<Key>)>,
}

impl Nets<'_> {
    /// The logical nets (wires, dots, same-name labels and power ports, as
    /// the netlist has them), and the wire end, dot and label points that
    /// connect a pin.
    fn built(&mut self) -> &mut (SheetConnectivity, HashSet<Key>) {
        let sheet = self.sheet;
        self.cache.get_or_insert_with(|| {
            let anchors = (sheet.wires.iter())
                .flat_map(|w| [pt_key(&w.start), pt_key(&w.end)])
                .chain(sheet.junctions.iter().map(|j| pt_key(&j.position)))
                .chain(sheet.labels.iter().map(|l| pt_key(&l.position)))
                .collect();
            (signex_net::logical_connectivity(sheet), anchors)
        })
    }

    /// Whether a pin end of `tips` other than the one at `k` is already on
    /// the logical net of `wire`: it is a wire end, dot or label point of it.
    fn net_holds_another(&mut self, wire: &Wire, k: Key, tips: &[Point]) -> bool {
        let (nets, anchors) = self.built();
        let net = nets.root_of(&wire.start);
        tips.iter().any(|other| {
            let at = pt_key(other);
            at != k && anchors.contains(&at) && nets.root_of(other) == net
        })
    }

    /// Whether wires `a` and `b` are on one net.
    fn same_net(&mut self, a: &Wire, b: &Wire) -> bool {
        let (nets, _) = self.built();
        nets.root_of(&a.start) == nets.root_of(&b.start)
    }

    /// Whether a dot at `at` would merge two nets: a wire through it, or a
    /// wire end, dot, label or power-port pin anchored there (a dotless T),
    /// is on another net than `wire`, which passes through it. `own` is the
    /// part whose pin end is at `at`: its own pin is not another net.
    fn merges_at(
        &mut self,
        at: &Point,
        wire: &Wire,
        through: &[&Wire],
        own: Option<uuid::Uuid>,
    ) -> bool {
        if through.iter().any(|other| !self.same_net(wire, other)) {
            return true;
        }
        let sheet = self.sheet;
        let k = pt_key(at);
        let (nets, anchors) = self.built();
        let root = nets.root_of(&wire.start);
        if anchors.contains(&k) && nets.root_of(at) != root {
            return true;
        }
        // A power port names its net across the sheet: its pin here joins it.
        let ports = (sheet.symbols.iter()).filter(|s| s.is_power && Some(s.uuid) != own);
        for port in ports {
            for tip in symbol_pin_tips(sheet, port) {
                if pt_key(&tip) == k && nets.root_of(&tip) != root {
                    return true;
                }
            }
        }
        false
    }
}

/// The pin ends inside a wire that may join it, and those refused because
/// a dot there would merge two nets (`Nets::merges_at`).
#[derive(Default)]
struct Taps {
    safe: Vec<Point>,
    crossed: Vec<Point>,
}

/// The pin ends of the symbols of `sheet` that `pick` keeps which lie strictly
/// inside a wire and may join it. A dot must never short a part: a pin end
/// may not join a wire that holds another pin end of the same part (a wire
/// running along a resistor through both its pins), nor a wire whose net
/// already has one, nor a net another pin end of the part joins here. Nor may
/// a dot merge two nets: a pin end where wires of different nets pass or
/// meet (overlapping, crossing, or a bare T whose end sits there) gets none
/// (`crossed`, maintainer decision 2026-11-19).
fn safe_taps(
    sheet: &SchematicSheet,
    lines: &WireLines<'_>,
    pick: impl Fn(&Symbol) -> bool,
) -> Taps {
    let mut nets = Nets { sheet, cache: None };
    let mut out = Taps::default();
    for symbol in sheet.symbols.iter().filter(|symbol| pick(symbol)) {
        let tips = symbol_pin_tips(sheet, symbol);
        // The nets this part's pin ends join here, by a wire of each.
        let mut joined: Vec<&Wire> = Vec::new();
        for tip in &tips {
            let k = pt_key(tip);
            let through = lines.through(k);
            let Some(&wire) = through.first() else {
                continue;
            };
            if nets.merges_at(tip, wire, &through[1..], Some(symbol.uuid)) {
                out.crossed.push(*tip);
                continue;
            }
            let shorts = through.iter().any(|w| holds_another(w, k, &tips))
                || nets.net_holds_another(wire, k, &tips)
                || joined.iter().any(|other| nets.same_net(other, wire));
            if !shorts {
                joined.push(wire);
                out.safe.push(*tip);
            }
        }
    }
    out
}

/// Where the dots of `sheet` are.
fn dot_keys(sheet: &SchematicSheet) -> HashSet<Key> {
    (sheet.junctions.iter())
        .map(|j| pt_key(&j.position))
        .collect()
}

/// Whether `k` lies on `wire` and is not one of its ends.
fn strictly_inside(k: Key, wire: &Wire) -> bool {
    let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
    k != a && k != b && point_on_segment(k, a, b)
}

/// The wires of a sheet by the row or column they run along, so "which wire
/// passes through this point" reads one line, not the sheet.
struct WireLines<'a> {
    rows: BTreeMap<i64, Vec<&'a Wire>>,
    columns: BTreeMap<i64, Vec<&'a Wire>>,
    slanted: Vec<&'a Wire>,
}

impl<'a> WireLines<'a> {
    fn of(wires: &'a [Wire]) -> Self {
        let mut lines = Self {
            rows: BTreeMap::new(),
            columns: BTreeMap::new(),
            slanted: Vec::new(),
        };
        for wire in wires {
            let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
            if a.1 == b.1 {
                lines.rows.entry(a.1).or_default().push(wire);
            } else if a.0 == b.0 {
                lines.columns.entry(a.0).or_default().push(wire);
            } else {
                lines.slanted.push(wire);
            }
        }
        lines
    }

    /// The wires that pass through `k` with neither end there.
    fn through(&self, k: Key) -> Vec<&'a Wire> {
        let row = self.rows.get(&k.1).into_iter().flatten();
        let column = self.columns.get(&k.0).into_iter().flatten();
        row.chain(column)
            .chain(self.slanted.iter())
            .filter(|wire| strictly_inside(k, wire))
            .copied()
            .collect()
    }
}

/// Where a pin tip sits strictly inside a wire with no dot there: a contact
/// the netlist does not count, left by an older Signex or another tool. A
/// touch a dot would turn into a short of its own part, or where wires of two
/// nets overlap, is not one.
#[must_use]
pub fn bare_pin_taps(sheet: &SchematicSheet) -> Vec<Point> {
    let dots = dot_keys(sheet);
    let lines = WireLines::of(&sheet.wires);
    let mut seen: HashSet<Key> = HashSet::new();
    safe_taps(sheet, &lines, |_| true)
        .safe
        .into_iter()
        .filter(|tip| {
            let k = pt_key(tip);
            !dots.contains(&k) && seen.insert(k)
        })
        .collect()
}

/// The repair of [`bare_pin_taps`]: one dot per bare contact, as commands to
/// run as one batch (one undo step). Empty when there is nothing to repair.
/// Each dot is an automatic one, drawn and saved like any junction and
/// checked again on every later edit; its id comes from `ids` (#2429).
#[must_use]
pub fn pin_junction_repairs(sheet: &SchematicSheet, ids: &mut dyn IdSource) -> Vec<Command> {
    (bare_pin_taps(sheet).into_iter())
        .map(|at| Command::PlaceJunction {
            junction: pin_dot(at, ids),
        })
        .collect()
}

/// An automatic pin dot at `position`, its id from the caller's `ids`: the
/// engine mints none (#2429).
fn pin_dot(position: Point, ids: &mut dyn IdSource) -> Junction {
    Junction {
        uuid: ids.next_id(),
        position,
        minted: true,
        ..Junction::default()
    }
}

/// Where a pin end touches a wire strictly inside it with no dot: the bare
/// contacts a sheet holds, read before an edit so the edit leaves them be.
#[must_use]
pub fn bare_tap_keys(sheet: &SchematicSheet) -> HashSet<Key> {
    let dots = dot_keys(sheet);
    let lines = WireLines::of(&sheet.wires);
    (sheet.symbols.iter())
        .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
        .map(|tip| pt_key(&tip))
        .filter(|k| !dots.contains(k) && !lines.through(*k).is_empty())
        .collect()
}

/// What a reconcile decides for the pin ends an edit put inside a wire.
#[derive(Default)]
pub(crate) struct PinDots {
    /// The dots to mint.
    pub(crate) dots: Vec<Junction>,
    /// Pin ends left bare because a dot there would merge two nets.
    pub(crate) crossed: Vec<Point>,
}

/// The dots an edit needs because a pin end now lies strictly inside a wire:
/// the pins of the `items` symbols on any wire, and any pin on the `items`
/// wires. Only places with no dot yet; called with the edit applied. A pin end
/// that touched a wire bare before the edit (`before`, see [`bare_tap_keys`])
/// stays bare: the edit did not make that contact, and only Repair Pin
/// Junctions turns an old bare touch into a connection. The dots' ids come
/// from `ids`.
pub(crate) fn pin_dots_for(
    sheet: &SchematicSheet,
    items: &[SelectedItem],
    before: Option<&HashSet<Key>>,
    ids: &mut dyn IdSource,
) -> PinDots {
    let kinds = |kind: SelectedKind| -> HashSet<uuid::Uuid> {
        (items.iter())
            .filter(|item| item.kind == kind)
            .map(|item| item.uuid)
            .collect()
    };
    let (symbols, wires) = (kinds(SelectedKind::Symbol), kinds(SelectedKind::Wire));
    if symbols.is_empty() && wires.is_empty() {
        return PinDots::default();
    }
    let lines = WireLines::of(&sheet.wires);
    let mut candidates = safe_taps(sheet, &lines, |symbol| symbols.contains(&symbol.uuid));
    if !wires.is_empty() {
        let touched: Vec<&Wire> = (sheet.wires.iter())
            .filter(|wire| wires.contains(&wire.uuid))
            .collect();
        let on_touched = |tip: &Point| {
            touched
                .iter()
                .any(|wire| strictly_inside(pt_key(tip), wire))
        };
        let all = safe_taps(sheet, &lines, |_| true);
        candidates
            .safe
            .extend(all.safe.into_iter().filter(on_touched));
        candidates
            .crossed
            .extend(all.crossed.into_iter().filter(on_touched));
    }
    let made_here = |tip: &Point| before.is_none_or(|old| !old.contains(&pt_key(tip)));
    let mut taken = dot_keys(sheet);
    let dots = (candidates.safe.into_iter())
        .filter(made_here)
        .filter(|tip| taken.insert(pt_key(tip)))
        .map(|tip| pin_dot(tip, ids))
        .collect();
    let mut seen: HashSet<Key> = HashSet::new();
    let crossed = (candidates.crossed.into_iter())
        .filter(made_here)
        .filter(|tip| !taken.contains(&pt_key(tip)) && seen.insert(pt_key(tip)))
        .collect();
    PinDots { dots, crossed }
}

/// The notice for the pin ends of `crossed` that `sheet` still leaves bare
/// where a dot would merge two nets: each pin with the two nets, as the
/// netlist names them. `None` when none is left.
pub(crate) fn crossed_taps_notice(
    sheet: &SchematicSheet,
    crossed: &[Point],
) -> Option<crate::EngineNotice> {
    if crossed.is_empty() {
        return None;
    }
    let dots = dot_keys(sheet);
    let lines = WireLines::of(&sheet.wires);
    let mut nets = Nets { sheet, cache: None };
    // Per bare place: the wire through it, and a wire of the other net there
    // (through it, or ending on it in a bare T), when one is.
    let mut pairs: Vec<(Key, uuid::Uuid, Option<uuid::Uuid>)> = Vec::new();
    let mut seen: HashSet<Key> = HashSet::new();
    for tip in crossed {
        let k = pt_key(tip);
        if !seen.insert(k) || dots.contains(&k) {
            continue;
        }
        let through = lines.through(k);
        let Some(&first) = through.first() else {
            continue;
        };
        if !nets.merges_at(tip, first, &through[1..], None) {
            continue;
        }
        let crossing = through[1..].iter().find(|w| !nets.same_net(first, w));
        let other = crossing.map(|w| w.uuid).or_else(|| {
            (sheet.wires.iter())
                .find(|w| pt_key(&w.start) == k || pt_key(&w.end) == k)
                .map(|w| w.uuid)
        });
        pairs.push((k, first.uuid, other));
    }
    if pairs.is_empty() {
        return None;
    }
    let netlist = signex_net::build_netlist(sheet);
    let net_of = |wire: uuid::Uuid| {
        (netlist.nets.iter())
            .find(|net| net.wires.contains(&wire))
            .map(|net| net.name.clone())
    };
    // A label alone names the other net when no wire of it is there.
    let label_at = |k: Key| {
        (sheet.labels.iter())
            .find(|label| pt_key(&label.position) == k)
            .map(|label| label.text.clone())
    };
    let mut pins: Vec<crate::RefusedPinDot> = Vec::new();
    for (k, a, b) in &pairs {
        let through = net_of(*a);
        let other = match b {
            Some(wire) => net_of(*wire),
            None => label_at(*k),
        };
        for pin in pin_labels_at(sheet, *k) {
            pins.push(crate::RefusedPinDot {
                pin,
                through: through.clone(),
                other: other.clone(),
            });
        }
    }
    (!pins.is_empty()).then_some(crate::EngineNotice::PinDotRefused { pins })
}

/// "R1 pin 2" for every pin end of `sheet` at `k`.
fn pin_labels_at(sheet: &SchematicSheet, k: Key) -> Vec<String> {
    let mut out = Vec::new();
    for symbol in &sheet.symbols {
        let Some(lib) = sheet.resolve_lib_symbol(symbol) else {
            continue;
        };
        let xform = SymbolTransform::from_symbol(symbol);
        for lib_pin in instance_pins(&lib, symbol) {
            if pt_key(&xform.apply(lib_pin.pin.position)) != k {
                continue;
            }
            let pin = if lib_pin.pin.number.is_empty() {
                &lib_pin.pin.name
            } else {
                &lib_pin.pin.number
            };
            out.push(format!("{} pin {pin}", symbol.reference));
        }
    }
    out
}

/// Where a wire end lies strictly inside another wire of `sheet` with no dot:
/// a dotless T, no connection (#107).
pub(crate) fn bare_t_keys(sheet: &SchematicSheet) -> HashSet<Key> {
    let dots = dot_keys(sheet);
    let lines = WireLines::of(&sheet.wires);
    (sheet.wires.iter())
        .flat_map(|wire| [pt_key(&wire.start), pt_key(&wire.end)])
        .filter(|k| !dots.contains(k) && !lines.through(*k).is_empty())
        .collect()
}

/// The pin dots of [`pin_dots_for`] when they were decided on another sheet:
/// a dot at each place of `frozen` where a wire still passes strictly through
/// and no dot stands yet. Geometry only; the no-short verdict is the one the
/// list was made with. The dots' ids come from `ids`.
pub(crate) fn frozen_pin_dots(
    sheet: &SchematicSheet,
    frozen: &[Point],
    ids: &mut dyn IdSource,
) -> Vec<Junction> {
    if frozen.is_empty() {
        return Vec::new();
    }
    let lines = WireLines::of(&sheet.wires);
    let mut taken = dot_keys(sheet);
    (frozen.iter())
        .filter(|at| !lines.through(pt_key(at)).is_empty())
        .filter(|at| taken.insert(pt_key(at)))
        .map(|at| pin_dot(*at, ids))
        .collect()
}

/// The pin dots `after` gained over `before`: new dots at a pin end that lies
/// strictly inside a wire. What a plain move decided, for its guarded pass to
/// keep (see [`frozen_pin_dots`]).
#[must_use]
pub(crate) fn new_pin_dots(before: &SchematicSheet, after: &SchematicSheet) -> Vec<Point> {
    let old: HashSet<uuid::Uuid> = before.junctions.iter().map(|j| j.uuid).collect();
    let fresh: Vec<Point> = (after.junctions.iter())
        .filter(|j| j.minted && !old.contains(&j.uuid))
        .map(|j| j.position)
        .collect();
    if fresh.is_empty() {
        return fresh;
    }
    let tips: HashSet<Key> = (after.symbols.iter())
        .flat_map(|symbol| symbol_pin_tips(after, symbol))
        .map(|tip| pt_key(&tip))
        .collect();
    let lines = WireLines::of(&after.wires);
    (fresh.into_iter())
        .filter(|at| tips.contains(&pt_key(at)) && !lines.through(pt_key(at)).is_empty())
        .collect()
}

/// The pin tips of the parts of `sheet` named in `ids`.
pub(crate) fn tip_keys_of(sheet: &SchematicSheet, ids: &HashSet<uuid::Uuid>) -> HashSet<Key> {
    (sheet.symbols.iter())
        .filter(|symbol| ids.contains(&symbol.uuid))
        .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
        .map(|tip| pt_key(&tip))
        .collect()
}

/// Every pin tip of `sheet` with the parts whose pin ends sit there, for
/// judging an automatic pin dot again: it stays while a pin end and a wire
/// still meet there (it goes when the pin or the wire leaves). The no-short
/// rule is the minting's ([`pin_dots_for`]), never a reason to drop a dot.
#[must_use]
pub(crate) fn pin_owners(sheet: &SchematicSheet) -> HashMap<Key, Vec<uuid::Uuid>> {
    let mut owners: HashMap<Key, Vec<uuid::Uuid>> = HashMap::new();
    for symbol in &sheet.symbols {
        for tip in symbol_pin_tips(sheet, symbol) {
            owners.entry(pt_key(&tip)).or_default().push(symbol.uuid);
        }
    }
    owners
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rubber_band_tests::{add_resistor, add_wire, lib_pin, lib_symbol, place};
    use crate::test_support::test_sheet;
    use signex_types::object_id::SequentialIds;

    fn sheet_with_tap() -> SchematicSheet {
        let mut sheet = test_sheet();
        sheet
            .lib_symbols
            .insert("TP".to_string(), lib_symbol("TP", vec![lib_pin("1", 0.0)]));
        place(&mut sheet, "TP1", "TP", Point::new(20.0, 10.0));
        place(&mut sheet, "TP2", "TP", Point::new(0.0, 10.0));
        add_wire(&mut sheet, (0.0, 10.0), (40.0, 10.0));
        sheet
    }

    #[test]
    fn a_pin_mid_wire_is_a_bare_tap_and_one_on_the_end_is_not() {
        let sheet = sheet_with_tap();
        assert_eq!(bare_pin_taps(&sheet), vec![Point::new(20.0, 10.0)]);
        let tap = pt_key(&Point::new(20.0, 10.0));
        assert!(pin_owners(&sheet).contains_key(&tap));
    }

    #[test]
    fn a_dot_on_the_tap_ends_the_report_and_the_repair_adds_one_dot() {
        let mut sheet = sheet_with_tap();
        let mut ids = SequentialIds::default();
        let repairs = pin_junction_repairs(&sheet, &mut ids);
        assert_eq!(repairs.len(), 1);
        let Command::PlaceJunction { junction } = &repairs[0] else {
            panic!("a repair places a junction");
        };
        assert!(junction.minted);
        assert_eq!(junction.uuid, uuid::Uuid::from_u128(1), "the caller's id");
        sheet.junctions.push(junction.clone());
        assert!(bare_pin_taps(&sheet).is_empty());
        assert!(pin_junction_repairs(&sheet, &mut ids).is_empty());
    }

    /// A wire along a part through both its pin ends: a dot on each would short
    /// the part, so neither gets one, none is reported and none repaired.
    #[test]
    fn a_wire_through_both_pins_of_a_part_gets_no_dot() {
        let mut sheet = test_sheet();
        let r1 = add_resistor(&mut sheet, "R1", Point::new(20.0, 10.0)); // pins 15, 25
        let wire = add_wire(&mut sheet, (0.0, 10.0), (40.0, 10.0));
        assert!(bare_pin_taps(&sheet).is_empty());
        let items = [
            SelectedItem::new(wire, SelectedKind::Wire),
            SelectedItem::new(r1, SelectedKind::Symbol),
        ];
        assert!(
            pin_dots_for(&sheet, &items, None, &mut SequentialIds::default())
                .dots
                .is_empty()
        );
    }
}
