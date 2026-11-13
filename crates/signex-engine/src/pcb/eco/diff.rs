//! The footprint rows of the change list: which parts are added, removed,
//! re-footprinted, renamed, revalued or linked.

use std::collections::{HashMap, HashSet};

use pcb_geom::{PointNm, RectNm, footprint_extent, nm_to_mm, stage};
use signex_types::part::{FootprintSourceRef, ResolvedFootprint};
use signex_types::pcb::{Footprint, PcbBoard, Point, SymbolLink};
use signex_types::schematic::InstancePath;

use super::change::{
    ChangeReason, EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoNote, EcoRow,
};
use super::{EcoInput, EcoPart, is_under, stable_uuid};
use crate::pcb::footprint_content::carry_would_lose;

const REFERENCE_KEY: &str = "Reference";
const VALUE_KEY: &str = "Value";

/// The change list that brings `input.board` in line with the design: the
/// footprint rows, then the net rows computed on the board as it would be
/// with the ticked footprint rows applied.
pub fn diff_board(input: &EcoInput) -> EcoChangeList {
    let board = input.board;
    let mut list = EcoChangeList {
        revision: input.revision,
        rows: Vec::new(),
        notes: input
            .unreadable
            .iter()
            .cloned()
            .map(EcoNote::UnreadableSheet)
            .collect(),
    };
    let duplicates = duplicate_designators(input.parts);
    let matched = match_parts(board, input.parts, input.unreadable, &duplicates);

    let mut adds: Vec<(usize, EcoRow)> = Vec::new();
    for (pi, part) in input.parts.iter().enumerate() {
        match matched.footprint_of[pi] {
            Some(fi) => footprint_rows(
                &mut list,
                &board.footprints[fi],
                part,
                matched.by_designator.contains(&fi),
                &duplicates,
            ),
            None if part.on_board => adds.push((pi, add_row(part, &duplicates))),
            None => {}
        }
    }
    stage_adds(board, input.staging, &mut adds);
    list.rows.extend(adds.into_iter().map(|(_, row)| row));

    for (fi, fp) in board.footprints.iter().enumerate() {
        if matched.part_of.contains_key(&fi) {
            continue;
        }
        match &fp.schematic_link {
            Some(link) if input.unreadable.iter().any(|u| is_under(&link.sheet, u)) => {}
            Some(link) => match input.parts.iter().enumerate().find(|(pi, p)| {
                p.on_board
                    && matched.footprint_of[*pi].is_some()
                    && link.units.iter().any(|u| p.part.units.contains(u))
            }) {
                // Another footprint holds the part the link names or shares a
                // unit with, and it is on the board: listed, never ticked.
                Some((_, part)) => {
                    let mut row = remove_row(fp);
                    row.default_on = false;
                    row.on = false;
                    list.rows.push(row);
                    list.notes.push(EcoNote::ExtraLink {
                        footprint: fp.uuid,
                        reference: fp.reference.clone(),
                        designator: part.part.designator.clone(),
                    });
                }
                _ => list.rows.push(remove_row(fp)),
            },
            None => list.notes.push(EcoNote::KeptUnlinked {
                footprint: fp.uuid,
                reference: fp.reference.clone(),
            }),
        }
    }
    list.rows.sort_by_key(|r| r.group);
    super::nets::refresh_nets(&mut list, input);
    list
}

/// Designators carried by more than one annotated part.
fn duplicate_designators(parts: &[EcoPart]) -> HashSet<String> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    for p in parts.iter().filter(|p| !p.part.needs_annotation) {
        *count.entry(p.part.designator.as_str()).or_default() += 1;
    }
    count
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(d, _)| d.to_string())
        .collect()
}

struct Matched {
    /// Part index -> footprint index.
    footprint_of: Vec<Option<usize>>,
    /// Footprint index -> part index.
    part_of: HashMap<usize, usize>,
    /// Footprints matched by designator (no stored link yet).
    by_designator: HashSet<usize>,
}

/// Whether `link` names some part of the design.
fn names_any_part(link: &SymbolLink, parts: &[EcoPart]) -> bool {
    parts
        .iter()
        .any(|p| link.names_part(&p.part.sheet, &p.part.units))
}

/// The unique unmatched, on-board part sharing a unit uuid with `link`, the
/// sheet path ignored: where a footprint whose link names no part went.
fn part_by_shared_unit(link: &SymbolLink, parts: &[EcoPart], m: &Matched) -> Option<usize> {
    let mut hits = parts.iter().enumerate().filter(|(pi, p)| {
        m.footprint_of[*pi].is_none()
            && p.on_board
            && link.units.iter().any(|u| p.part.units.contains(u))
    });
    match (hits.next(), hits.next()) {
        (Some((pi, _)), None) => Some(pi),
        _ => None,
    }
}

/// Match parts to footprints: by stored link first (same sheet, a shared
/// unit), then an unlinked footprint to the one part its designator names,
/// when no other unlinked footprint or annotated part shares that designator,
/// then a footprint whose link names no part of the design (its sheet path
/// changed) to the one part that shares a unit uuid with the link. A link
/// under an unreadable sheet is left alone.
fn match_parts(
    board: &PcbBoard,
    parts: &[EcoPart],
    unreadable: &[InstancePath],
    duplicates: &HashSet<String>,
) -> Matched {
    let mut m = Matched {
        footprint_of: vec![None; parts.len()],
        part_of: HashMap::new(),
        by_designator: HashSet::new(),
    };
    for (pi, part) in parts.iter().enumerate() {
        let hit = board.footprints.iter().enumerate().position(|(fi, fp)| {
            !m.part_of.contains_key(&fi)
                && fp
                    .schematic_link
                    .as_ref()
                    .is_some_and(|l| l.names_part(&part.part.sheet, &part.part.units))
        });
        if let Some(fi) = hit {
            m.footprint_of[pi] = Some(fi);
            m.part_of.insert(fi, pi);
        }
    }
    let mut unlinked: HashMap<&str, Vec<usize>> = HashMap::new();
    for (fi, fp) in board.footprints.iter().enumerate() {
        if fp.schematic_link.is_none() {
            unlinked.entry(fp.reference.trim()).or_default().push(fi);
        }
    }
    for (pi, part) in parts.iter().enumerate() {
        let d = part.part.designator.as_str();
        if m.footprint_of[pi].is_some() || part.part.needs_annotation || duplicates.contains(d) {
            continue;
        }
        if let Some([fi]) = unlinked.get(d).map(Vec::as_slice) {
            m.footprint_of[pi] = Some(*fi);
            m.part_of.insert(*fi, pi);
            m.by_designator.insert(*fi);
        }
    }
    for (fi, fp) in board.footprints.iter().enumerate() {
        let Some(link) = &fp.schematic_link else {
            continue;
        };
        let dangling =
            !names_any_part(link, parts) && !unreadable.iter().any(|u| is_under(&link.sheet, u));
        // The row's link differs from the part's, so a Link row follows.
        if dangling && let Some(pi) = part_by_shared_unit(link, parts, &m) {
            m.footprint_of[pi] = Some(fi);
            m.part_of.insert(fi, pi);
        }
    }
    m
}

fn designator_block(part: &EcoPart, duplicates: &HashSet<String>) -> Option<EcoBlock> {
    if part.part.needs_annotation {
        Some(EcoBlock::NeedsAnnotation)
    } else if duplicates.contains(&part.part.designator) {
        Some(EcoBlock::DuplicateDesignator(part.part.designator.clone()))
    } else {
        None
    }
}

fn part_link(part: &EcoPart) -> SymbolLink {
    SymbolLink::new(part.part.sheet.clone(), part.part.units.iter().copied())
}

/// The rows for a part that has a footprint.
fn footprint_rows(
    list: &mut EcoChangeList,
    fp: &Footprint,
    part: &EcoPart,
    linked_by_designator: bool,
    duplicates: &HashSet<String>,
) {
    let id = fp.uuid;
    if !part.read {
        // Nothing about the part is known: leave its footprint alone.
        list.notes.push(EcoNote::PartNotRead {
            designator: part.part.designator.clone(),
        });
        return;
    }
    if let Err(error) = &part.footprint
        && !part.pin_map_known(Some(fp))
    {
        list.notes.push(EcoNote::FootprintUnresolved {
            footprint: id,
            reference: fp.reference.clone(),
            error: error.clone(),
        });
    }
    if !part.on_board {
        list.rows.push(remove_row(fp));
        return;
    }
    let link = part_link(part);
    if linked_by_designator || fp.schematic_link.as_ref() != Some(&link) {
        list.rows.push(EcoRow::new(
            EcoKey::Link(id),
            EcoGroup::Links,
            EcoChange::Link {
                footprint: id,
                reference: fp.reference.clone(),
                link,
            },
            true,
        ));
    }
    if fp.reference != part.part.designator {
        let row = EcoRow::new(
            EcoKey::SetDesignator(id),
            EcoGroup::Designators,
            EcoChange::SetDesignator {
                footprint: id,
                from: fp.reference.clone(),
                to: part.part.designator.clone(),
            },
            true,
        );
        list.rows.push(match designator_block(part, duplicates) {
            Some(why) => row.blocked(why),
            None => row,
        });
    }
    if fp.value != part.value {
        list.rows.push(EcoRow::new(
            EcoKey::SetValue(id),
            EcoGroup::Values,
            EcoChange::SetValue {
                footprint: id,
                reference: fp.reference.clone(),
                from: fp.value.clone(),
                to: part.value.clone(),
            },
            true,
        ));
    }
    if let Ok(resolved) = &part.footprint
        && let Some(reason) = change_reason(fp, resolved)
    {
        let row = change_row(list, fp, resolved, reason);
        list.rows.push(row);
    }
}

/// Why `fp` should be replaced by `resolved`, if it should. A footprint with
/// no library identity (an older board) is compared by name.
pub(super) fn change_reason(fp: &Footprint, resolved: &ResolvedFootprint) -> Option<ChangeReason> {
    let to = &resolved.source;
    match &fp.library_ref {
        Some(from) if from.library == to.library && from.footprint == to.footprint => {
            (from.version != to.version).then_some(ChangeReason::OtherVersion)
        }
        Some(_) => Some(ChangeReason::Footprint),
        None => {
            (fp.footprint_id != resolved.footprint.footprint_id).then_some(ChangeReason::Footprint)
        }
    }
}

fn change_row(
    list: &mut EcoChangeList,
    fp: &Footprint,
    resolved: &ResolvedFootprint,
    reason: ChangeReason,
) -> EcoRow {
    let mut with = resolved.footprint.clone();
    with.library_ref = Some(resolved.source.clone());
    remint_pads(&mut with, fp.uuid, &resolved.source);
    let new_numbers: HashSet<&str> = with.pads.iter().map(|p| p.number.as_str()).collect();
    for pad in &fp.pads {
        if new_numbers.contains(pad.number.as_str()) {
            continue;
        }
        if pad.net_locked {
            list.notes.push(EcoNote::PadLosesLock {
                footprint: fp.uuid,
                reference: fp.reference.clone(),
                pad_number: pad.number.clone(),
            });
        }
        if let Some(net) = &pad.net {
            list.notes.push(EcoNote::PadLosesNet {
                footprint: fp.uuid,
                reference: fp.reference.clone(),
                pad_number: pad.number.clone(),
                net: net.name.clone(),
            });
        }
    }
    let default_on = reason == ChangeReason::Footprint && !fp.locked;
    let blocked = carry_would_lose(fp, &with).map(|e| match e {
        crate::pcb::BoardCommandError::PadCountMismatch { number, .. } => {
            EcoBlock::PadsCannotCarry { number }
        }
        other => EcoBlock::Unplaceable(other.to_string()),
    });
    let row = EcoRow::new(
        EcoKey::ChangeFootprint(fp.uuid),
        EcoGroup::ChangeFootprints,
        EcoChange::ChangeFootprint {
            footprint: fp.uuid,
            reference: fp.reference.clone(),
            from: fp
                .library_ref
                .as_ref()
                .map_or_else(|| fp.footprint_id.clone(), ToString::to_string),
            to: resolved.source.clone(),
            reason,
            with: Box::new(with),
        },
        default_on,
    );
    match blocked {
        Some(why) => row.blocked(why),
        None => row,
    }
}

fn remove_row(fp: &Footprint) -> EcoRow {
    EcoRow::new(
        EcoKey::RemoveFootprint(fp.uuid),
        EcoGroup::RemoveFootprints,
        EcoChange::RemoveFootprint {
            footprint: fp.uuid,
            reference: fp.reference.clone(),
        },
        !fp.locked,
    )
}

fn add_key(part: &EcoPart) -> EcoKey {
    EcoKey::AddFootprint {
        sheet: part.part.sheet.clone(),
        part: part.part.units.iter().min().copied().unwrap_or_default(),
    }
}

/// The add row for a part with no footprint; positioned later by
/// [`stage_adds`].
fn add_row(part: &EcoPart, duplicates: &HashSet<String>) -> EcoRow {
    let key = add_key(part);
    let resolved = match &part.footprint {
        Ok(r) => r,
        Err(why) => {
            let placeholder = Footprint {
                uuid: new_footprint_uuid(&key),
                reference: part.part.designator.clone(),
                value: part.value.clone(),
                ..empty_footprint()
            };
            let block = if part.read {
                EcoBlock::MissingFootprint(why.clone())
            } else {
                EcoBlock::PartNotRead
            };
            return EcoRow::new(
                key,
                EcoGroup::AddFootprints,
                EcoChange::AddFootprint {
                    footprint: Box::new(placeholder),
                },
                true,
            )
            .blocked(block);
        }
    };
    let mut fp = resolved.footprint.clone();
    fp.uuid = new_footprint_uuid(&key);
    fp.reference = part.part.designator.clone();
    fp.value = part.value.clone();
    for property in &mut fp.properties {
        if property.key == REFERENCE_KEY {
            property.value.clone_from(&fp.reference);
        } else if property.key == VALUE_KEY {
            property.value.clone_from(&fp.value);
        }
    }
    fp.schematic_link = Some(part_link(part));
    fp.library_ref = Some(resolved.source.clone());
    fp.position = Point { x: 0.0, y: 0.0 };
    fp.rotation = 0.0;
    let owner = fp.uuid;
    remint_pads(&mut fp, owner, &resolved.source);
    let row = EcoRow::new(
        key,
        EcoGroup::AddFootprints,
        EcoChange::AddFootprint {
            footprint: Box::new(fp),
        },
        true,
    );
    match designator_block(part, duplicates) {
        Some(why) => row.blocked(why),
        None => row,
    }
}

/// Place every add row's footprint to the right of the board, in part
/// order. Blocked rows are staged too, so ticking or fixing one never moves
/// the others.
fn stage_adds(board: &PcbBoard, staging: pcb_geom::Staging, adds: &mut [(usize, EcoRow)]) {
    let mut extents = Vec::new();
    let mut staged = Vec::new();
    for (i, (_, row)) in adds.iter_mut().enumerate() {
        let EcoChange::AddFootprint { footprint } = &row.change else {
            continue;
        };
        match footprint_extent(footprint) {
            Ok(extent) => {
                extents.push(extent);
                staged.push(i);
            }
            Err(e) if row.blocked.is_none() => {
                *row = row.clone().blocked(EcoBlock::Unplaceable(e.to_string()));
            }
            Err(_) => {}
        }
    }
    let origins = stage(occupied(board), &extents, staging);
    for (i, origin) in staged.into_iter().zip(origins) {
        if let EcoChange::AddFootprint { footprint } = &mut adds[i].1.change {
            footprint.position = Point {
                x: nm_to_mm(origin.x),
                y: nm_to_mm(origin.y),
            };
        }
    }
}

/// What staging must stay clear of: the outline and every footprint already
/// on the board (one staged by an earlier update included).
fn occupied(board: &PcbBoard) -> Option<RectNm> {
    let outline = board
        .outline
        .iter()
        .filter_map(|p| PointNm::from_mm(*p, "outline").ok());
    let mut bounds = RectNm::bounding(outline);
    for fp in &board.footprints {
        let (Ok(extent), Ok(origin)) = (
            footprint_extent(fp),
            PointNm::from_mm(fp.position, "footprint.position"),
        ) else {
            continue;
        };
        // Rotation is ignored: the extent's farthest corner bounds any turn.
        let reach = extent
            .corners()
            .iter()
            .map(|c| c.x.abs().max(c.y.abs()))
            .max()
            .unwrap_or(0);
        let placed = RectNm::around(origin, reach, reach);
        bounds = Some(bounds.map_or(placed, |b| b.union(&placed)));
    }
    bounds
}

fn new_footprint_uuid(key: &EcoKey) -> uuid::Uuid {
    match key {
        EcoKey::AddFootprint { sheet, part } => {
            let chain: Vec<u8> = sheet.chain().iter().flat_map(|u| *u.as_bytes()).collect();
            stable_uuid(&[b"eco-footprint", &chain, part.as_bytes()])
        }
        _ => uuid::Uuid::nil(),
    }
}

/// Give `fp`'s pads uuids that depend only on the board footprint they are
/// for, the library footprint they come from and their place in it.
fn remint_pads(fp: &mut Footprint, owner: uuid::Uuid, source: &FootprintSourceRef) {
    let source = source.to_string();
    for (i, pad) in fp.pads.iter_mut().enumerate() {
        pad.uuid = stable_uuid(&[
            b"eco-pad",
            owner.as_bytes(),
            source.as_bytes(),
            &(i as u64).to_le_bytes(),
        ]);
        pad.net = None;
        pad.net_locked = false;
    }
}

fn empty_footprint() -> Footprint {
    Footprint {
        uuid: uuid::Uuid::nil(),
        reference: String::new(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}
