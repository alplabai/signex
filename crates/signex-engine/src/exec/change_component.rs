//! `Command::ChangeComponent` (#1260): swap a placed symbol for another
//! library part, and the preview that says what the swap does to wires. A
//! pin mapping given by hand is in `change_component_pins` (#2954).

use super::change_component_pins::{
    Carry, carry_connections, check_hand_map, unit_carries, unit_mapping,
};
use super::pin_changes::{has_connection, shown_pins, unit_missing};
use crate::*;
use signex_net::{connected_points, pt_key};
use signex_types::schematic::{
    LibPin, LibSymbol, PinMapping, Symbol, SymbolTransform, group_into_parts, pin_key,
};
use std::collections::HashMap;
use uuid::Uuid;

/// The part a placed symbol is swapped for: its drawing and the library
/// identity and value a fresh placement of it would carry.
#[derive(Debug, Clone)]
pub struct ComponentReplacement {
    pub lib_symbol: LibSymbol,
    pub library_id: Option<Uuid>,
    pub row_id: Option<Uuid>,
    pub library_version: String,
    /// The row values the new part starts in agreement with. The old part's
    /// stamp describes another row, so it is always replaced; a part with no
    /// library identity keeps none.
    pub stamp: Option<signex_types::schematic::RowStamp>,
    pub value: String,
    /// The part's own footprint, datasheet and identity fields. They
    /// replace the placed part's: the old part's footprint is not the new
    /// part's. An empty value clears the field.
    pub footprint: String,
    pub datasheet: String,
    pub fields: Vec<(String, String)>,
    /// The pins as mapped by hand, old pin to new pin (#2954); `None` maps
    /// by number, then by name ([`signex_types::schematic::map_pins`]). A
    /// hand mapping is followed: what ends on an old pin's tip moves to the
    /// tip of its new pin, and an old pin it leaves unmatched dangles.
    pub pin_map: Option<PinMapping>,
}

impl ComponentReplacement {
    /// The parameters of the part `symbol_id` belongs to that the new part
    /// does not supply: the user's own and override parameters, which the
    /// swap may keep. Everything the new part supplies is library-owned and
    /// always replaced (DECISIONS 2026-11-19). Sorted, each once.
    #[must_use]
    pub fn user_parameters(&self, doc: &SchematicSheet, symbol_id: Uuid) -> Vec<String> {
        let mut names: Vec<String> = placed_units(doc, symbol_id)
            .iter()
            .flat_map(|unit| unit.fields.keys())
            .filter(|key| !self.fields.iter().any(|(own, _)| own == *key))
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// This replacement keeping exactly the user parameters named in `kept`
    /// (Change Component's "Keep now" boxes): every other user parameter of
    /// the part is cleared by the swap. Library-owned data is replaced
    /// either way.
    #[must_use]
    pub fn keeping_parameters(
        mut self,
        doc: &SchematicSheet,
        symbol_id: Uuid,
        kept: &[String],
    ) -> Self {
        for name in self.user_parameters(doc, symbol_id) {
            if !kept.contains(&name) {
                self.fields.push((name, String::new()));
            }
        }
        self
    }
}

/// What swapping a placed symbol for another drawing does to its pins.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ComponentChangePreview {
    pub mapping: PinMapping,
    /// Connected old pins no new pin takes over.
    pub unmatched_connected: Vec<String>,
    /// Connected old pins whose new pin sits somewhere else, so the wire
    /// would no longer reach it.
    pub moved_connected: Vec<String>,
    /// Connected old pins whose connection a hand mapping carries to their
    /// new pin's tip, which sits somewhere else.
    pub carried: Vec<String>,
    /// Why the hand mapping cannot be followed (a carried wire would join
    /// another net, is locked, ...); the swap is refused while it is set.
    pub refused: Option<String>,
    /// New pins landing on a connection no old pin occupied.
    pub gained: Vec<String>,
    /// The placed unit, when the new drawing has nothing for it.
    pub missing_unit: Option<u32>,
    /// Part-owned data that changes ("Value", "Footprint", an identity field).
    pub replaced: Vec<String>,
    /// What stays: the designator and parameters the new part lacks.
    pub kept: Vec<String>,
    /// Non-empty part data that is cleared or overwritten, with its old
    /// value, over every placed unit.
    pub changed_data: Vec<DataChange>,
}

/// One part-data field whose current value is lost by the swap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataChange {
    pub label: String,
    pub old: String,
    /// Empty when the field is cleared.
    pub new: String,
}

impl ComponentChangePreview {
    /// Nothing is lost, moved or gained: the wires stay where they are.
    pub fn is_clean(&self) -> bool {
        self.unmatched_connected.is_empty()
            && self.moved_connected.is_empty()
            && self.gained.is_empty()
            && self.missing_unit.is_none()
    }
}

/// The placed units of the part `symbol_id` belongs to on this sheet, by the
/// one grouping rule every consumer shares
/// ([`group_into_parts`]): units of one designator and drawing with different
/// unit numbers. A duplicate designator with the same unit is a part of its
/// own.
///
/// An empty or unannotated (`U?`) designator is deliberately a part of its
/// own, which differs from [`group_into_parts`]: that pairs `U?` symbols of
/// one drawing in input order, a guess Annotate needs to number them but one
/// a swap must not make, since two unrelated `U?` parts would be swapped
/// together. Treating the picked symbol alone is the safe side: at worst a
/// multi-unit `U?` part is changed one unit at a time, and the user sees
/// each step, where the other way silently rewrites a part nobody picked.
pub(super) fn placed_units(doc: &SchematicSheet, symbol_id: Uuid) -> Vec<&Symbol> {
    let Some(at) = doc.symbols.iter().position(|s| s.uuid == symbol_id) else {
        return Vec::new();
    };
    let target = &doc.symbols[at];
    let reference = target.reference.trim();
    if reference.is_empty() || reference.ends_with('?') {
        return vec![target];
    }
    group_into_parts(doc.symbols.iter().map(Symbol::part_key))
        .into_iter()
        .find(|part| part.contains(&at))
        .map(|part| part.into_iter().map(|i| &doc.symbols[i]).collect())
        .unwrap_or_else(|| vec![target])
}

/// The uuids of [`placed_units`].
pub fn placed_unit_ids(doc: &SchematicSheet, symbol_id: Uuid) -> Vec<Uuid> {
    placed_units(doc, symbol_id)
        .iter()
        .map(|s| s.uuid)
        .collect()
}

fn push_new(list: &mut Vec<String>, key: String) {
    if !list.contains(&key) {
        list.push(key);
    }
}

/// The pins of one placed unit judged against the replacement drawing. With
/// a hand mapping a connected pin whose new pin sits elsewhere is carried
/// there, and a new pin on the tip a carry leaves gains nothing.
fn unit_preview(
    doc: &SchematicSheet,
    gate: &signex_net::ConnectedPoints,
    sym: &Symbol,
    new_drawing: &LibSymbol,
    hand: Option<&PinMapping>,
) -> ComponentChangePreview {
    let old_pins = shown_pins(doc.body_of(sym), sym);
    let new_pins = shown_pins(Some(new_drawing), sym);
    let mapping = unit_mapping(&old_pins, &new_pins, hand);
    let xform = SymbolTransform::from_symbol(sym);
    let tip = |lp: &LibPin| xform.apply(lp.pin.position);
    let new_pin = |key: &str| new_pins.iter().find(|np| pin_key(np) == key);

    let mut preview = ComponentChangePreview::default();
    let mut connected_old: Vec<&str> = Vec::new();
    // Old tips a carry empties.
    let mut left: Vec<signex_net::uf::Key> = Vec::new();
    for old in &old_pins {
        if !has_connection(doc, gate, &tip(old)) {
            continue;
        }
        let key = pin_key(old);
        connected_old.push(key);
        match mapping.pairs.iter().find(|p| p.old == key) {
            None => preview.unmatched_connected.push(key.to_string()),
            Some(pair) => {
                let stays =
                    new_pin(&pair.new).is_some_and(|np| pt_key(&tip(np)) == pt_key(&tip(old)));
                match (stays, hand.is_some()) {
                    (true, _) => {}
                    (false, true) => {
                        preview.carried.push(key.to_string());
                        left.push(pt_key(&tip(old)));
                    }
                    (false, false) => preview.moved_connected.push(key.to_string()),
                }
            }
        }
    }
    for np in &new_pins {
        let key = pin_key(np);
        let from_connected_old = mapping
            .pairs
            .iter()
            .any(|p| p.new == key && connected_old.contains(&p.old.as_str()));
        let on_a_left_tip = left.contains(&pt_key(&tip(np)));
        if !from_connected_old && !on_a_left_tip && has_connection(doc, gate, &tip(np)) {
            preview.gained.push(key.to_string());
        }
    }
    preview.missing_unit = unit_missing(sym, new_drawing).then_some(sym.unit);
    preview.mapping = mapping;
    preview
}

/// What replacing the part data of every unit with `replacement` changes
/// (labels), what stays, and which non-empty values are lost.
fn data_changes(
    units: &[&Symbol],
    replacement: &ComponentReplacement,
) -> (Vec<String>, Vec<String>, Vec<DataChange>) {
    let mut replaced = Vec::new();
    let mut changed = Vec::new();
    let mut kept = vec!["Designator".to_string()];
    for unit in units {
        let mut check = |label: &str, old: &str, new: &str| {
            if old == new {
                return;
            }
            push_new(&mut replaced, label.to_string());
            let lost = DataChange {
                label: label.to_string(),
                old: old.to_string(),
                new: new.to_string(),
            };
            if !old.is_empty() && !changed.contains(&lost) {
                changed.push(lost);
            }
        };
        check("Value", &unit.value, &replacement.value);
        check("Footprint", &unit.footprint, &replacement.footprint);
        check("Datasheet", &unit.datasheet, &replacement.datasheet);
        for (key, value) in &replacement.fields {
            check(key, unit.fields.get(key).map_or("", String::as_str), value);
        }
        let mut parameters: Vec<&String> = unit
            .fields
            .keys()
            .filter(|k| !replacement.fields.iter().any(|(key, _)| key == *k))
            .collect();
        parameters.sort();
        for key in parameters {
            push_new(&mut kept, key.clone());
        }
    }
    (replaced, kept, changed)
}

/// What swapping the part of placed symbol `symbol_id` for `replacement`
/// does to the pins of every placed unit of it, judged on the sheet's
/// existing copy of the drawing when it holds one (placement keeps it too),
/// and which data is replaced or kept. Empty when the symbol is unknown.
pub fn change_component_preview(
    doc: &SchematicSheet,
    symbol_id: Uuid,
    replacement: &ComponentReplacement,
) -> ComponentChangePreview {
    let units = placed_units(doc, symbol_id);
    if units.is_empty() {
        return ComponentChangePreview::default();
    }
    let new_drawing = &replacement.lib_symbol;
    let gate = connected_points(doc);
    let hand = replacement.pin_map.as_ref();
    let mut preview = ComponentChangePreview::default();
    for unit in &units {
        let one = unit_preview(doc, &gate, unit, new_drawing, hand);
        one.unmatched_connected
            .into_iter()
            .for_each(|k| push_new(&mut preview.unmatched_connected, k));
        one.moved_connected
            .into_iter()
            .for_each(|k| push_new(&mut preview.moved_connected, k));
        one.carried
            .into_iter()
            .for_each(|k| push_new(&mut preview.carried, k));
        one.gained
            .into_iter()
            .for_each(|k| push_new(&mut preview.gained, k));
        preview.missing_unit = preview.missing_unit.or(one.missing_unit);
        for pair in one.mapping.pairs {
            if !preview.mapping.pairs.contains(&pair) {
                preview.mapping.pairs.push(pair);
            }
        }
        for key in one.mapping.unmatched {
            push_new(&mut preview.mapping.unmatched, key);
        }
    }
    let paired: Vec<String> = preview
        .mapping
        .pairs
        .iter()
        .map(|p| p.old.clone())
        .collect();
    preview.mapping.unmatched.retain(|k| !paired.contains(k));
    (preview.replaced, preview.kept, preview.changed_data) = data_changes(&units, replacement);
    if hand.is_some()
        && let Err(EngineError::ChangeComponentPins { detail }) =
            swapped_sheet(doc, symbol_id, replacement)
    {
        preview.refused = Some(detail);
    }
    preview
}

/// `unit_missing` for every placed unit: `"<reference> unit <n>"` each.
fn missing_units(units: &[&Symbol], drawing: &LibSymbol) -> Vec<String> {
    units
        .iter()
        .filter(|s| unit_missing(s, drawing))
        .map(|s| format!("{} unit {}", s.reference, s.unit))
        .collect()
}

/// The pin uuids of one unit after the swap: a shown pin keeps its uuid
/// under the number it maps to, a shown pin with no match loses it, and a
/// pin the unit does not show is carried over as it was.
pub(super) fn remap_pin_uuids(
    sym: &Symbol,
    old_shown: &[&LibPin],
    mapping: &PinMapping,
) -> HashMap<String, Uuid> {
    let shown: Vec<&str> = old_shown.iter().map(|p| pin_key(p)).collect();
    let mut out: HashMap<String, Uuid> = sym
        .pin_uuids
        .iter()
        .filter(|(key, _)| !shown.contains(&key.as_str()))
        .map(|(key, uuid)| (key.clone(), *uuid))
        .collect();
    // A pin keeps the uuid of the pin it maps from. A fan-out (one old pin
    // mapped to several new ones) hands the first the uuid and mints one for
    // each other, so the saved file never repeats a pin uuid within a symbol.
    let mut taken: std::collections::HashSet<Uuid> = out.values().copied().collect();
    for pair in &mapping.pairs {
        if let Some(uuid) = sym.pin_uuids.get(&pair.old) {
            let own = if taken.insert(*uuid) {
                *uuid
            } else {
                let fresh = Uuid::now_v7();
                taken.insert(fresh);
                fresh
            };
            out.insert(pair.new.clone(), own);
        }
    }
    out
}

/// `sym` re-keyed to the new part: its drawing, library identity, value,
/// part data and the pin uuids `mapping` carries over.
fn swapped_symbol(
    sym: &Symbol,
    replacement: &ComponentReplacement,
    new_ref: &str,
    old_pins: &[&LibPin],
    mapping: &PinMapping,
) -> Symbol {
    let mut next = sym.clone();
    next.lib_id = replacement.lib_symbol.id.clone();
    next.body_ref = new_ref.to_string();
    next.library_id = replacement.library_id;
    next.row_id = replacement.row_id;
    next.library_version = replacement.library_version.clone();
    // Tagged with the version it travels with, whatever the caller set.
    next.library_stamp = replacement
        .library_id
        .and(replacement.stamp.clone())
        .map(|mut stamp| {
            stamp.version = Some(replacement.library_version.clone());
            stamp
        });
    next.value = replacement.value.clone();
    next.footprint = replacement.footprint.clone();
    next.datasheet = replacement.datasheet.clone();
    for (key, value) in &replacement.fields {
        if value.is_empty() {
            next.fields.remove(key);
        } else {
            next.fields.insert(key.clone(), value.clone());
        }
    }
    next.pin_uuids = remap_pin_uuids(sym, old_pins, mapping);
    next
}

/// The sheet with the part of placed symbol `symbol_id` swapped for
/// `replacement`, and what changed in it; `None` when nothing would. With a
/// hand mapping the connections it carries are moved too. Pure: the
/// command installs it, the preview asks it whether the carry is refused.
pub fn swapped_sheet(
    doc: &SchematicSheet,
    symbol_id: Uuid,
    replacement: &ComponentReplacement,
) -> Result<Option<(SchematicSheet, DocumentPatch)>, EngineError> {
    // Refuse a body-ref collision first, even when the body is already
    // embedded: a different body under the ref is never drawn silently.
    let new_ref = doc.check_body(&replacement.lib_symbol)?;
    let placed = placed_units(doc, symbol_id);
    if placed.is_empty() {
        return Ok(None);
    }
    let registered = doc.lib_symbols.contains_key(&new_ref);
    let drawing = &replacement.lib_symbol;
    let gone = missing_units(&placed, drawing);
    if !gone.is_empty() {
        return Err(EngineError::ChangeComponentUnitMissing { parts: gone });
    }
    let hand = replacement.pin_map.as_ref();
    if let Some(hand) = hand {
        check_hand_map(doc, &placed, replacement, hand)?;
    }
    let gate = hand.map(|_| connected_points(doc));
    let mut updated: Vec<Symbol> = Vec::new();
    let mut carries: Vec<Carry> = Vec::new();
    for sym in &placed {
        let old_pins = shown_pins(doc.body_of(sym), sym);
        let new_pins = shown_pins(Some(drawing), sym);
        let mapping = unit_mapping(&old_pins, &new_pins, hand);
        if let Some(gate) = &gate {
            carries.extend(unit_carries(
                doc,
                gate,
                sym,
                (&old_pins, &new_pins),
                &mapping,
            ));
        }
        let next = swapped_symbol(sym, replacement, &new_ref, &old_pins, &mapping);
        if next != **sym {
            updated.push(next);
        }
    }
    if registered && updated.is_empty() && carries.is_empty() {
        return Ok(None);
    }
    let mut out = doc.clone();
    if !registered {
        out.embed_body(replacement.lib_symbol.clone())?;
    }
    for next in updated {
        if let Some(slot) = out.symbols.iter_mut().find(|s| s.uuid == next.uuid) {
            *slot = next;
        }
    }
    let mut patch = DocumentPatch::LIB_SYMBOLS | DocumentPatch::SYMBOLS;
    if !carries.is_empty() {
        let part: Vec<Uuid> = placed.iter().map(|s| s.uuid).collect();
        let designator = placed[0].reference.clone();
        match carry_connections(&out, &part, &carries, &designator) {
            Ok(Some(carried)) => {
                out = carried;
                patch = patch
                    | DocumentPatch::WIRES
                    | DocumentPatch::LABELS
                    | DocumentPatch::JUNCTIONS
                    | DocumentPatch::NO_CONNECTS
                    | DocumentPatch::NO_ERC_DIRECTIVES;
            }
            Ok(None) => {}
            Err(detail) => return Err(EngineError::ChangeComponentPins { detail }),
        }
    }
    Ok(Some((out, patch)))
}

impl Engine {
    pub(super) fn change_component(
        &mut self,
        before: SchematicSheet,
        symbol_id: Uuid,
        replacement: ComponentReplacement,
    ) -> Result<CommandResult, EngineError> {
        let Some((next, document)) = swapped_sheet(&self.document, symbol_id, &replacement)? else {
            return Ok(CommandResult::unchanged());
        };
        self.document = next;
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SymbolFieldsUpdated,
            document,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
