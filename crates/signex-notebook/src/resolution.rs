//! Resolving a notebook's references against the design, and telling the
//! stale ones apart (mockup row 196): a part renamed (its stored id still
//! finds it), a part or net no longer in the design, and a pin the part no
//! longer has. A rename made outside Annotate leaves the text stale until
//! it is updated; see `docs/NOTEBOOK_ANNOTATIONS.md`, "References".

use uuid::Uuid;

use crate::design::{DesignIndex, DesignPart};
use crate::header::Header;
use crate::reference::Reference;
use crate::syntax::ReferenceAt;

/// Where one reference stands against the design.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReferenceState {
    /// The design has what the reference names.
    Resolved,
    /// The stored id finds the part under another designator: `now` is the
    /// reference as it should read (`R17`, or `R17.3` for a pin).
    Renamed { now: Reference },
    /// No part or net of that name, and no stored id that finds one.
    Deleted,
    /// The part is there but has no such pin; `pins` are the pins it has.
    MissingPin { pins: Vec<String> },
}

/// One distinct reference of a notebook file and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedReference {
    pub reference: Reference,
    #[serde(flatten)]
    pub state: ReferenceState,
    /// The id of the part (for a component or a pin) it resolved to, else
    /// the stored id.
    pub id: Option<Uuid>,
    /// The 1-based lines of each use, in source order, repeats kept.
    pub lines: Vec<u32>,
}

impl ResolvedReference {
    /// True when the reference no longer reads as the design does.
    pub fn is_stale(&self) -> bool {
        self.state != ReferenceState::Resolved
    }

    /// How many times the file uses it.
    pub fn uses(&self) -> usize {
        self.lines.len()
    }
}

/// Every distinct reference of a file, first use first.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReferenceReport {
    pub references: Vec<ResolvedReference>,
}

impl ReferenceReport {
    /// Every use of every reference: the "31 references" of the status bar.
    pub fn total_uses(&self) -> usize {
        self.references.iter().map(ResolvedReference::uses).sum()
    }

    /// How many distinct references are stale: the "3 stale".
    pub fn stale_count(&self) -> usize {
        self.stale().count()
    }

    /// The stale references, first use first.
    pub fn stale(&self) -> impl Iterator<Item = &ResolvedReference> {
        self.references.iter().filter(|r| r.is_stale())
    }

    /// The entry of `reference`.
    pub fn get(&self, reference: &Reference) -> Option<&ResolvedReference> {
        self.references.iter().find(|r| &r.reference == reference)
    }

    /// Store in `header` the id of every reference that found its part, so
    /// a later rename is still found, and drop the stored references the
    /// file no longer makes. True when the header changed.
    pub fn store_ids(&self, header: &mut Header) -> bool {
        let before = header.clone();
        let used: Vec<Reference> = self
            .references
            .iter()
            .map(|r| r.reference.clone())
            .collect();
        header.retain_used(&used);
        for entry in &self.references {
            let keeps = matches!(
                entry.state,
                ReferenceState::Resolved | ReferenceState::Renamed { .. }
            );
            if keeps && entry.id.is_some() {
                header.set_id(&entry.reference, entry.id);
            }
        }
        *header != before
    }
}

/// Resolve the references `found` in one file (`crate::references`)
/// against `design`, with the ids `header` stored for them.
pub fn resolve_references(
    found: &[ReferenceAt],
    header: &Header,
    design: &DesignIndex,
) -> ReferenceReport {
    let mut report = ReferenceReport::default();
    for at in found {
        if let Some(entry) = report
            .references
            .iter_mut()
            .find(|r| r.reference == at.reference)
        {
            entry.lines.push(at.line);
            continue;
        }
        let stored = header.id_of(&at.reference);
        let (state, id) = resolve_one(&at.reference, stored, design);
        report.references.push(ResolvedReference {
            reference: at.reference.clone(),
            state,
            id,
            lines: vec![at.line],
        });
    }
    report
}

/// Where `reference` stands, with `stored`, the id kept in the header.
pub fn resolve_one(
    reference: &Reference,
    stored: Option<Uuid>,
    design: &DesignIndex,
) -> (ReferenceState, Option<Uuid>) {
    match reference {
        Reference::Net(name) => match design.net(name) {
            Some(_) => (ReferenceState::Resolved, None),
            None => (ReferenceState::Deleted, None),
        },
        Reference::Component(name) => match find_part(name, stored, design) {
            Found::Named(part) => (ReferenceState::Resolved, Some(part.id)),
            Found::Renamed(part) => (
                ReferenceState::Renamed {
                    now: Reference::Component(part.designator.clone()),
                },
                Some(part.id),
            ),
            Found::None => (ReferenceState::Deleted, stored),
        },
        Reference::Pin { component, pin } => {
            let (part, renamed) = match find_part(component, stored, design) {
                Found::Named(part) => (part, false),
                Found::Renamed(part) => (part, true),
                Found::None => return (ReferenceState::Deleted, stored),
            };
            // A pin the part no longer has is not fixed by a rename.
            if part.pin(pin).is_none() {
                let pins = part.pin_numbers();
                return (ReferenceState::MissingPin { pins }, Some(part.id));
            }
            let state = if renamed {
                ReferenceState::Renamed {
                    now: Reference::Pin {
                        component: part.designator.clone(),
                        pin: pin.clone(),
                    },
                }
            } else {
                ReferenceState::Resolved
            };
            (state, Some(part.id))
        }
    }
}

enum Found<'a> {
    /// The part under the name the text uses.
    Named(&'a DesignPart),
    /// The part the stored id finds, now under another designator.
    Renamed(&'a DesignPart),
    None,
}

/// The stored id decides first: a part carrying it under `name` is named,
/// under another designator of the same channel renamed. Every placement
/// of a reused sheet carries the same id, so a rename resolves only within
/// the channel the name is in (`R1_B` to `R2_B`, never to `R1_A`); with no
/// one carrier left in that channel the part is gone (#3021). With no id,
/// or one no part carries, the name decides.
fn find_part<'a>(name: &str, stored: Option<Uuid>, design: &'a DesignIndex) -> Found<'a> {
    if let Some(id) = stored {
        let carrying: Vec<&DesignPart> = design.parts_with_id(id).collect();
        if !carrying.is_empty() {
            if let Some(named) = carrying.iter().copied().find(|p| p.designator == name) {
                return Found::Named(named);
            }
            let channel = channel_of(name);
            let mut same = carrying
                .iter()
                .copied()
                .filter(|p| channel_of(&p.designator) == channel);
            return match (same.next(), same.next()) {
                (Some(part), None) => Found::Renamed(part),
                _ => Found::None,
            };
        }
    }
    match design.part(name) {
        Some(part) => Found::Named(part),
        None => Found::None,
    }
}

/// The channel suffix of a designator: what follows its prefix letters and
/// number (`_B` of `R1_B`; empty for `R7`).
fn channel_of(designator: &str) -> &str {
    let rest = designator.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    rest.trim_start_matches(|c: char| c.is_ascii_digit())
}
