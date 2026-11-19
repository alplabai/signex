//! The guarded keep-wires pass of a move, rotate or mirror: one plain
//! reference, one list of contacts, every round checked as it is committed.
//!
//! `P` is what the plain move commits (the move, its junctions reconciled,
//! its wires folded). It is made once and never edited, and the guard's
//! intent is read from it and the pre-move nets alone. The contacts are the
//! moved pins and the points the dragged wires carry (`exec::ownership` says
//! which pass has which point). Each round starts from the moved sheet,
//! carries the wires, follows the pins, reconciles with the pin dots `P`
//! decided, folds, and is checked against the intent; a contact the guard
//! blames goes down the ladder (follow, forced new path, plain) and the round
//! runs again. A contact that was connected and ends plain is named in a
//! notice. Decision: docs/DECISIONS.md, 2026-11-19.

use std::collections::HashSet;

use signex_types::schematic::{Point, SchematicSheet, SelectedItem};
use uuid::Uuid;

use super::ownership::Ownership;
use crate::{DocumentPatch, Engine};

/// What a guarded pass keeps joined.
#[derive(Default)]
pub(super) struct Contacts {
    /// Moved pins, for the pin pass.
    pub(super) tips: Vec<crate::PinTip>,
    /// Points the dragged wires carry, for the carry pass.
    pub(super) wires: Vec<crate::WirePoint>,
    /// Points of a dragged wire that cannot be carried whole: plain from the
    /// start, and named when they held something.
    pub(super) bailed: Vec<crate::WirePoint>,
}

impl Contacts {
    fn is_empty(&self) -> bool {
        self.tips.is_empty() && self.wires.is_empty() && self.bailed.is_empty()
    }
}

/// Folds the wires an edit lined up end to end: the fold a plain commit runs,
/// run inside every round so the sheet checked is the sheet committed.
pub(super) type Fold<'f> = &'f dyn Fn(&mut Engine) -> DocumentPatch;

/// The fixed side of the rounds: the moved sheet every round starts from,
/// the plain move `P`, and the pin dots `P` made.
struct Reference<'r> {
    moved: &'r SchematicSheet,
    plain: &'r SchematicSheet,
    plain_dots: &'r [Point],
}

#[cfg(test)]
thread_local! {
    /// Test switch: every contact starts plain (invariant I1).
    static ALL_PLAIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The last guarded pass on this thread, for the tests' own check.
    static LAST: std::cell::RefCell<Option<Record>> = const { std::cell::RefCell::new(None) };
}

/// What the last guarded pass decided, for the tests' own check.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct Record {
    /// Old keys of the contacts the rounds left plain (the guard's policy).
    pub(crate) plain: HashSet<signex_net::uf::Key>,
    /// Old keys of the points of a wire that could not be carried whole: plain
    /// from the start, apart from the policy, so a pin that rode on such a
    /// wire and sits on one of these keys is judged on its own.
    pub(crate) bailed: HashSet<signex_net::uf::Key>,
    /// Every carried point, `(old, new)`, bailed ones included.
    pub(crate) wires: Vec<(Point, Point)>,
    /// The moved pins the pin pass kept (not those riding on a carried wire).
    pub(crate) tips: Vec<crate::PinIdent>,
}

/// Turns [`ALL_PLAIN`] on for this thread until dropped.
#[cfg(test)]
pub(crate) struct AllPlain(bool);

#[cfg(test)]
impl Drop for AllPlain {
    fn drop(&mut self) {
        ALL_PLAIN.with(|on| on.set(self.0));
    }
}

/// Every contact of every guarded pass on this thread starts plain.
#[cfg(test)]
pub(crate) fn all_plain() -> AllPlain {
    AllPlain(ALL_PLAIN.with(|on| on.replace(true)))
}

/// The record of the last guarded pass on this thread, taken.
#[cfg(test)]
pub(crate) fn take_record() -> Option<Record> {
    LAST.with(|last| last.borrow_mut().take())
}

/// One notice for `names`: `one` of the name when there is one, else `many` of
/// the count, then the first few names.
fn notice(
    names: &[String],
    one: impl Fn(&str) -> String,
    many: impl Fn(usize) -> String,
) -> String {
    const NAMED: usize = 3;
    let shown = names
        .iter()
        .take(NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match names.len() {
        1 => one(&shown),
        n if n <= NAMED => format!("{}: {shown}", many(n)),
        n => format!("{}: {shown} and {} more", many(n), n - NAMED),
    }
}

impl Engine {
    /// The contacts of a move with "keep wires", read with the move applied:
    /// the points the dragged wires carry (a wire that cannot be carried
    /// whole bails at every point), and the moved pins that do not ride on a
    /// carried wire.
    pub(super) fn contacts_of(
        &self,
        owners: Ownership,
        wires: &[crate::WireContact],
        items: &[SelectedItem],
    ) -> Contacts {
        let points = crate::carried_points(&self.document, wires, items, &owners.pin_owned());
        let bailing = crate::bailing_wires(&self.document, &points, items);
        let (bailed, live): (Vec<_>, Vec<_>) =
            (points.into_iter()).partition(|point| bailing.contains(&point.wire()));
        Contacts {
            tips: owners.pin_tips(&bailing),
            wires: live,
            bailed,
        }
    }

    /// Commit the edit already applied to `self.document` with `contacts`
    /// kept: the plain commit when there are none, else the guarded rounds.
    pub(super) fn keep_contacts(
        &mut self,
        before: &SchematicSheet,
        contacts: &Contacts,
        items: &[SelectedItem],
        followed: &[SelectedItem],
        fold: Fold<'_>,
    ) -> DocumentPatch {
        let plain_items = [items, followed].concat();
        if contacts.is_empty() {
            let mut patch = self.reconciled_patch(&plain_items);
            patch |= fold(self);
            return patch;
        }
        // The sheet as the edit left it: every round starts here.
        let moved = self.document.clone();
        // P, the one reference: exactly what the plain edit commits.
        self.reconciled_patch(&plain_items);
        fold(self);
        let plain = std::mem::take(&mut self.document);
        let plain_dots = crate::transform::new_pin_dots(&moved, &plain);
        let reference = Reference {
            moved: &moved,
            plain: &plain,
            plain_dots: &plain_dots,
        };
        let contacts = self.needed_contacts(contacts, &moved, &plain);
        self.guarded_rounds(before, &contacts, items, followed, &reference, fold)
    }

    /// `contacts` less the carried points the plain move `plain` already keeps:
    /// the old place still on the wire's net through what was there before the
    /// move (no dot the move made), and more than a lone wire end left there.
    /// A lone end is the carry's to fold away; anything else stays as the
    /// plain move leaves it, so a drag that lines a wire up with its neighbour
    /// folds as a plain move does.
    fn needed_contacts(
        &self,
        contacts: &Contacts,
        moved: &SchematicSheet,
        plain: &SchematicSheet,
    ) -> Contacts {
        let mut needed = Contacts {
            tips: contacts.tips.clone(),
            wires: Vec::new(),
            bailed: contacts.bailed.clone(),
        };
        if contacts.wires.is_empty() {
            return needed;
        }
        let had: HashSet<Uuid> = moved.junctions.iter().map(|j| j.uuid).collect();
        let mut bare = plain.clone();
        bare.junctions.retain(|j| had.contains(&j.uuid));
        let keys: Vec<(signex_net::uf::Key, bool)> = (contacts.wires.iter())
            .flat_map(|w| [(w.old_key(), false), (w.new_key(), true)])
            .collect();
        let found = match self.preset_base.as_deref() {
            Some(base) => base.derive_at(&bare, &keys).1,
            None => signex_net::net_signatures_at(&bare, &keys).1,
        };
        for (point, at) in contacts.wires.iter().zip(found.chunks(2)) {
            let kept = at.len() == 2 && at[0].0 == at[1].0;
            if !kept || lone_wire_end(&bare, point.old_key()) {
                needed.wires.push(*point);
            }
        }
        needed
    }

    /// The rounds of [`Self::keep_contacts`]: each one follows, reconciles
    /// with `P`'s pin dots, folds and is checked; a blamed contact goes one
    /// step down the ladder until the round passes.
    fn guarded_rounds(
        &mut self,
        before: &SchematicSheet,
        contacts: &Contacts,
        items: &[SelectedItem],
        followed: &[SelectedItem],
        at: &Reference<'_>,
        fold: Fold<'_>,
    ) -> DocumentPatch {
        const MAX_ROUNDS: usize = 6;
        #[cfg(test)]
        let _scan = crate::transform::rubber_band::scan_all_on(self.full_scan_follow);
        let guard = crate::Guard::for_move(
            before,
            at.plain,
            &contacts.tips,
            &contacts.wires,
            self.preset_before.clone(),
            self.preset_base.clone(),
        );
        #[cfg(test)]
        let guard = if self.full_scan_guard {
            guard.without_local()
        } else {
            guard
        };
        let mut policy = crate::Policy::default();
        #[cfg(test)]
        if ALL_PLAIN.with(std::cell::Cell::get) {
            policy
                .plain
                .extend(contacts.tips.iter().map(crate::PinTip::old_key));
            policy
                .plain
                .extend(contacts.wires.iter().map(crate::WirePoint::old_key));
        }
        let tips_now: Vec<Option<Point>> = (contacts.tips.iter())
            .map(|tip| tip.now(at.moved))
            .collect();
        for round in 0.. {
            self.document = at.moved.clone();
            let carried = crate::carry_points(&mut self.document, &contacts.wires, items, &policy);
            let pins = crate::follow_moved_pins(&mut self.document, &contacts.tips, items, &policy);
            let mut touched = [items, followed].concat();
            touched.extend(carried.touched);
            touched.extend(pins.touched);
            let held: Vec<Point> = (guard.held_tips(&contacts.tips, &policy).into_iter())
                .filter_map(|i| tips_now[i])
                .collect();
            let dots = crate::pin_dots_to_keep(at.plain_dots, &held);
            let mut patch = DocumentPatch::from_selected_items(&touched);
            if self.reconcile_wire_junctions_frozen(&touched, Some(dots.as_slice())) {
                patch |= DocumentPatch::JUNCTIONS;
            }
            patch |= fold(self);
            let mut edited = carried.edited_tips;
            edited.extend(pins.edited_tips);
            let Some(blamed) = guard.check(&self.document, &contacts.tips, &policy, &edited) else {
                self.warn_plain_pins(&guard, &contacts.tips, &policy);
                self.warn_plain_wire_points(before, contacts, &policy, items);
                self.warn_unjoined_pins(contacts, at, &held, &dots, &tips_now);
                #[cfg(test)]
                LAST.with(|last| {
                    let plain = policy.plain.clone();
                    let bailed = (contacts.bailed.iter())
                        .map(crate::WirePoint::old_key)
                        .collect();
                    let wires = (contacts.wires.iter().chain(&contacts.bailed))
                        .map(|w| (w.old(), w.new_point()))
                        .collect();
                    let tips = contacts.tips.iter().map(|t| t.ident().clone()).collect();
                    *last.borrow_mut() = Some(Record {
                        plain,
                        bailed,
                        wires,
                        tips,
                    });
                });
                return patch;
            };
            // Every blamed contact is still live, so each round takes at least
            // one a step down: at most two rounds per contact, and after
            // MAX_ROUNDS the blamed go straight to a plain move.
            for key in blamed {
                if round >= MAX_ROUNDS {
                    policy.plain.insert(key);
                } else {
                    policy.escalate(key);
                }
            }
        }
        unreachable!("the escalation loop returns from inside")
    }

    /// One notice for the points of a dragged wire that held something
    /// before the move and ended plain: what they held is no longer on the
    /// wire.
    fn warn_plain_wire_points(
        &mut self,
        before: &SchematicSheet,
        contacts: &Contacts,
        policy: &crate::Policy,
        items: &[SelectedItem],
    ) {
        let moving: HashSet<Uuid> = items.iter().map(|item| item.uuid).collect();
        let live_plain = (contacts.wires.iter()).filter(|w| policy.plain.contains(&w.old_key()));
        let points: Vec<&crate::WirePoint> = live_plain.chain(&contacts.bailed).collect();
        if points.is_empty() {
            return;
        }
        // A point is parted only when the committed sheet has its old place
        // and its wire on two nets: a pin or another path may still keep it.
        let keys: Vec<(signex_net::uf::Key, bool)> = (points.iter())
            .flat_map(|w| [(w.old_key(), false), (w.new_key(), true)])
            .collect();
        let (_, found) = signex_net::net_signatures_at(&self.document, &keys);
        let mut names: Vec<String> = Vec::new();
        for (point, at) in points.iter().zip(found.chunks(2)) {
            if at.len() == 2 && at[0].0 == at[1].0 {
                continue;
            }
            if let Some(name) = crate::held_at(before, point.old(), &moving)
                && !names.contains(&name)
            {
                names.push(name);
            }
        }
        if names.is_empty() {
            return;
        }
        self.warnings.push(notice(
            &names,
            |name| format!("could not keep {name} on the dragged wire"),
            |n| format!("could not keep {n} connections on the dragged wire"),
        ));
    }

    /// One notice for the pins held on their old net whose new tip lies
    /// inside a wire the plain move would have joined them to (spec gap (a),
    /// `transform::rubber_band::intent`).
    fn warn_unjoined_pins(
        &mut self,
        contacts: &Contacts,
        at: &Reference<'_>,
        held: &[Point],
        kept: &[Point],
        tips_now: &[Option<Point>],
    ) {
        let key = signex_net::pt_key;
        let left_out = |p: &Point| {
            at.plain_dots.iter().any(|d| key(d) == key(p)) && !kept.iter().any(|d| key(d) == key(p))
        };
        let unjoined: Vec<Point> = held.iter().copied().filter(left_out).collect();
        if unjoined.is_empty() {
            return;
        }
        let names: Vec<String> = (contacts.tips.iter().zip(tips_now))
            .filter(|(_, now)| now.is_some_and(|p| unjoined.iter().any(|u| key(u) == key(&p))))
            .map(|(tip, _)| tip.label().to_string())
            .collect();
        self.warnings.push(notice(
            &names,
            |name| format!("{name} kept its net and was not joined to the wire it now ends on"),
            |n| format!("{n} pins kept their nets and were not joined to the wires they end on"),
        ));
    }
}

/// Whether only one wire end lies at `k` on `sheet`: no other wire end or
/// span, label, dot, pin or child-sheet pin.
fn lone_wire_end(sheet: &SchematicSheet, k: signex_net::uf::Key) -> bool {
    use signex_net::{point_on_segment, pt_key};
    let ends = (sheet.wires.iter())
        .filter(|w| pt_key(&w.start) == k || pt_key(&w.end) == k)
        .count();
    if ends != 1 {
        return false;
    }
    let through = (sheet.wires.iter()).any(|w| {
        let (a, b) = (pt_key(&w.start), pt_key(&w.end));
        a != k && b != k && point_on_segment(k, a, b)
    });
    let marked = sheet.labels.iter().any(|l| pt_key(&l.position) == k)
        || sheet.junctions.iter().any(|j| pt_key(&j.position) == k)
        || (sheet.child_sheets.iter())
            .flat_map(|c| c.pins.iter())
            .any(|p| pt_key(&p.position) == k);
    !through && !marked && !crate::transform::pin_owners(sheet).contains_key(&k)
}
