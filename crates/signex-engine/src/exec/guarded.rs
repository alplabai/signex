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
    /// Dotless T points of the moved sheet the plain move leaves bare: an edit
    /// of a wire there must not dot them (#107: no connection).
    bare: &'r HashSet<signex_net::uf::Key>,
    /// Every pin tip, where the move put it (the rounds do not move parts).
    pin_tips: &'r HashSet<signex_net::uf::Key>,
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
    /// The wires those points are on: they moved plainly, carrying nothing.
    pub(crate) bailed_wires: Vec<Uuid>,
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
pub(crate) fn notice(
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
        unfollowed: Option<SchematicSheet>,
    ) -> DocumentPatch {
        let plain_items = [items, followed].concat();
        if contacts.is_empty() && unfollowed.is_none() {
            let mut patch = self.reconciled_patch(&plain_items);
            patch |= fold(self);
            return patch;
        }
        #[cfg(test)]
        if !contacts.bailed.is_empty() {
            crate::transform::rubber_band::reasons::note("a dragged wire cannot be carried whole");
        }
        // The sheet as the edit left it: every round starts here.
        let moved = self.document.clone();
        // P, the one reference: exactly what the plain edit commits, so with
        // the wire ends past a dragged bus's entries where they were
        // (`unfollowed`): the rounds' shifts of them are checked too.
        if let Some(unfollowed) = unfollowed {
            self.document = unfollowed;
        }
        self.reconciled_patch(&plain_items);
        fold(self);
        let plain = std::mem::take(&mut self.document);
        let plain_dots = crate::transform::new_pin_dots(&moved, &plain);
        let dotted: HashSet<signex_net::uf::Key> = (plain.junctions.iter())
            .map(|j| signex_net::pt_key(&j.position))
            .collect();
        let mut bare = crate::transform::bare_t_keys(&moved);
        bare.retain(|k| !dotted.contains(k));
        let pin_tips: HashSet<signex_net::uf::Key> =
            crate::transform::pin_owners(&moved).into_keys().collect();
        let reference = Reference {
            moved: &moved,
            plain: &plain,
            plain_dots: &plain_dots,
            bare: &bare,
            pin_tips: &pin_tips,
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
        // The pin ends, read once for every point that asks.
        let owners = std::cell::OnceCell::new();
        for (point, at) in contacts.wires.iter().zip(found.chunks(2)) {
            let kept = at.len() == 2 && at[0].0 == at[1].0;
            let pins = || owners.get_or_init(|| crate::transform::pin_owners(&bare));
            if !kept || lone_wire_end(&bare, point.old_key(), pins) {
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
            self.document.clone_from(at.moved);
            let carried = crate::carry_points(&mut self.document, &contacts.wires, items, &policy);
            let pins = crate::follow_moved_pins(&mut self.document, &contacts.tips, items, &policy);
            let mut touched = [items, followed].concat();
            touched.extend(carried.touched);
            touched.extend(pins.touched);
            let held_tips = guard.held_tips(&contacts.tips, &policy);
            let held: Vec<Point> = held_tips.iter().filter_map(|&i| tips_now[i]).collect();
            let dots = crate::pin_dots_to_keep(at.plain_dots, &held);
            let mut patch = DocumentPatch::from_selected_items(&touched);
            let frozen = crate::transform::Frozen {
                pin_dots: &dots,
                bare: at.bare,
            };
            if self.reconcile_wire_junctions_frozen(&touched, Some(&frozen)) {
                patch |= DocumentPatch::JUNCTIONS;
            }
            patch |= fold(self);
            let mut edited = carried.edited_tips;
            edited.extend(pins.edited_tips);
            let (equal, mut verdict) =
                guard.judge(&self.document, &contacts.tips, &policy, &edited);
            // Spec gap (a), which the net comparison cannot see when the wire is
            // a bare run: a held pin still touching the wire it landed in.
            let touching = self.held_tips_touching(contacts, at, &held_tips, &dots, &tips_now);
            if !touching.is_empty() {
                #[cfg(test)]
                crate::transform::rubber_band::reasons::note("(a): a held pin touches its wire");
                let blamed = verdict.get_or_insert_with(Vec::new);
                blamed.extend(touching);
                blamed.sort_unstable();
                blamed.dedup();
            }
            // A wire the round laid over a pin end that the plain move leaves
            // free: a bare touch with no dot (no connection, #107) and no
            // notice. The contacts that edited take it down the ladder.
            if verdict.is_none() && self.makes_bare_taps(&touched, at) {
                #[cfg(test)]
                crate::transform::rubber_band::reasons::note("a new bare touch of a pin end");
                let live = live_keys(contacts, &policy, &edited);
                if !live.is_empty() {
                    verdict = Some(live);
                }
            }
            let Some(blamed) = verdict else {
                if !equal {
                    // Every contact is plain and the nets still differ from the
                    // intent: nothing is left to step down, so the plain move
                    // itself is what is committed.
                    self.document.clone_from(at.plain);
                    patch = DocumentPatch::from_selected_items(&[items, followed].concat())
                        | DocumentPatch::WIRES
                        | DocumentPatch::JUNCTIONS;
                }
                // A drag preview's notices are never shown, so it lists the pins
                // it left plain (cheap) and skips the rest, which reads nets.
                let preview = self.preset_sheet.is_some();
                self.warn_plain_pins(&guard, &contacts.tips, &policy, !preview);
                if !preview {
                    self.warn_plain_wire_points(before, contacts, &policy, items);
                    self.warn_unjoined_pins(contacts, at, &held, &dots, &tips_now);
                }
                #[cfg(test)]
                LAST.with(|last| {
                    let plain = policy.plain.clone();
                    let bailed = (contacts.bailed.iter())
                        .map(crate::WirePoint::old_key)
                        .collect();
                    let mut bailed_wires: Vec<Uuid> =
                        contacts.bailed.iter().map(crate::WirePoint::wire).collect();
                    bailed_wires.sort_unstable();
                    bailed_wires.dedup();
                    let wires = (contacts.wires.iter().chain(&contacts.bailed))
                        .map(|w| (w.old(), w.new_point()))
                        .collect();
                    let tips = contacts.tips.iter().map(|t| t.ident().clone()).collect();
                    *last.borrow_mut() = Some(Record {
                        plain,
                        bailed,
                        bailed_wires,
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
                    #[cfg(test)]
                    crate::transform::rubber_band::reasons::note("the ladder ran out of rounds");
                    policy.plain.insert(key);
                } else {
                    policy.escalate(key);
                }
            }
        }
        unreachable!("the escalation loop returns from inside")
    }

    /// Whether the round laid a wire it touched over a pin end with no dot
    /// there, where the plain move has none: neither the same bare touch nor
    /// a pin dot it made (a pin dot spec gap (a) left out is the held pin's,
    /// told in its own notice).
    fn makes_bare_taps(&self, touched: &[SelectedItem], at: &Reference<'_>) -> bool {
        use signex_net::{point_on_segment, pt_key};
        let ids: HashSet<Uuid> = (touched.iter())
            .filter(|item| item.kind == signex_types::schematic::SelectedKind::Wire)
            .map(|item| item.uuid)
            .collect();
        let inside = |k: signex_net::uf::Key, w: &signex_types::schematic::Wire| {
            let (a, b) = (pt_key(&w.start), pt_key(&w.end));
            a != k && b != k && point_on_segment(k, a, b)
        };
        let dot_at =
            |sheet: &SchematicSheet, k| (sheet.junctions.iter()).any(|j| pt_key(&j.position) == k);
        (self.document.wires.iter())
            .filter(|w| ids.contains(&w.uuid))
            .flat_map(|w| at.pin_tips.iter().copied().filter(move |&k| inside(k, w)))
            .any(|k| {
                !dot_at(&self.document, k)
                    && !at.plain_dots.iter().any(|d| pt_key(d) == k)
                    && !(at.plain.wires.iter().any(|w| inside(k, w)) && !dot_at(at.plain, k))
            })
    }

    /// Spec gap (a): the old keys of the held pins (`held`, indices into the
    /// tips) whose plain pin dot the round left out yet which the candidate
    /// still joins to the wire through their new tip, through a dot the
    /// path to the pin made there. The net comparison cannot see it when that
    /// wire is a bare run, so these go down the ladder like a blamed tip.
    fn held_tips_touching(
        &self,
        contacts: &Contacts,
        at: &Reference<'_>,
        held: &[usize],
        kept: &[Point],
        tips_now: &[Option<Point>],
    ) -> Vec<signex_net::uf::Key> {
        use signex_net::{point_on_segment, pt_key};
        let left_out = |k: signex_net::uf::Key| {
            at.plain_dots.iter().any(|d| pt_key(d) == k) && !kept.iter().any(|d| pt_key(d) == k)
        };
        let mut queries: Vec<(usize, signex_net::uf::Key, signex_net::uf::Key)> = Vec::new();
        for &i in held {
            let Some(k) = tips_now.get(i).copied().flatten().map(|p| pt_key(&p)) else {
                continue;
            };
            if !left_out(k) {
                continue;
            }
            let through = self.document.wires.iter().find(|w| {
                let (a, b) = (pt_key(&w.start), pt_key(&w.end));
                a != k && b != k && point_on_segment(k, a, b)
            });
            if let Some(wire) = through {
                queries.push((i, k, pt_key(&wire.start)));
            }
        }
        if queries.is_empty() {
            return Vec::new();
        }
        let keys: Vec<(signex_net::uf::Key, bool)> = (queries.iter())
            .flat_map(|(_, k, wire)| [(*k, false), (*wire, false)])
            .collect();
        // From the pre-move connectivity when a drag preview has it, as the
        // guard derives: the reached nets, not the whole sheet.
        let (_, found) = match self.preset_base.as_deref() {
            Some(base) => base.derive_at(&self.document, &keys),
            None => signex_net::net_signatures_at(&self.document, &keys),
        };
        (queries.iter().zip(found.chunks(2)))
            .filter(|(_, roots)| roots.len() == 2 && roots[0].0 == roots[1].0)
            .map(|((i, _, _), _)| contacts.tips[*i].old_key())
            .collect()
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
fn lone_wire_end<'p>(
    sheet: &SchematicSheet,
    k: signex_net::uf::Key,
    pins: impl FnOnce() -> &'p crate::PinOwners,
) -> bool {
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
    !through && !marked && !pins().contains_key(&k)
}

/// The live contacts' old keys to take down the ladder for a difference the
/// net comparison does not name: those whose handling edited something, or
/// every live one when none did.
fn live_keys(
    contacts: &Contacts,
    policy: &crate::Policy,
    edited: &[signex_net::uf::Key],
) -> Vec<signex_net::uf::Key> {
    let live: Vec<signex_net::uf::Key> = (contacts.tips.iter())
        .map(crate::PinTip::old_key)
        .chain(contacts.wires.iter().map(crate::WirePoint::old_key))
        .filter(|k| !policy.plain.contains(k))
        .collect();
    let mut keys: Vec<_> = live
        .iter()
        .copied()
        .filter(|k| edited.contains(k))
        .collect();
    if keys.is_empty() {
        keys = live;
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}
