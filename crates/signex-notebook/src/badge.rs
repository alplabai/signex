//! Notebook badges (mockup row 197): a badge on each part or net the
//! notebook writes about, with how many blocks name it, coloured by the
//! worst status among them. The same blocks are the hover card of a badge
//! and Properties ▸ Design notes.

use crate::annotation::{Annotation, AnnotationKind, Status};
use crate::date::IsoDate;
use crate::reference::Reference;

/// A badge's colour key, mildest first; a badge takes the worst of its
/// blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeTone {
    /// Notes and calculations only (blue).
    Notes,
    /// Every measurement passes (green).
    Pass,
    /// An issue is open (amber).
    OpenIssue,
    /// A measurement fails (red).
    Failing,
}

/// What a badge sits on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum BadgeTarget {
    /// A part, by designator: its blocks name it or one of its pins.
    Part(String),
    /// A net, by name.
    Net(String),
}

impl BadgeTarget {
    /// The targets a block naming `reference` puts a badge on.
    pub fn of(reference: &Reference) -> Self {
        match reference {
            Reference::Net(name) => BadgeTarget::Net(name.clone()),
            Reference::Component(name)
            | Reference::Pin {
                component: name, ..
            } => BadgeTarget::Part(name.clone()),
        }
    }

    /// The designator or net name.
    pub fn name(&self) -> &str {
        match self {
            BadgeTarget::Part(name) | BadgeTarget::Net(name) => name,
        }
    }
}

/// One block a badge, a hover card or Design notes lists.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BlockMention {
    pub kind: AnnotationKind,
    pub title: String,
    pub status: Option<Status>,
    pub date: Option<IsoDate>,
    /// The notebook file, as a path inside `notebook/`.
    pub file: String,
    /// The 1-based line the block starts on.
    pub line: u32,
    /// True for a measurement whose result fails. Blocks are read without
    /// a result today, so this is set only by a reader that has one.
    pub failing: bool,
}

impl BlockMention {
    /// The mention of `annotation` in `file`.
    pub fn of(annotation: &Annotation, file: &str) -> Self {
        Self {
            kind: annotation.kind,
            title: annotation.title.clone(),
            status: annotation.status,
            date: annotation.date,
            file: file.to_string(),
            line: annotation.line,
            failing: false,
        }
    }

    /// The tone this block alone gives.
    pub fn tone(&self) -> BadgeTone {
        match (self.kind, self.status) {
            (AnnotationKind::Measurement, _) if self.failing => BadgeTone::Failing,
            (AnnotationKind::Issue, Some(Status::Open)) => BadgeTone::OpenIssue,
            (AnnotationKind::Measurement, Some(Status::Done)) => BadgeTone::Pass,
            _ => BadgeTone::Notes,
        }
    }

    fn is_measurement(&self) -> bool {
        self.kind == AnnotationKind::Measurement
    }
}

/// The badge of one part or net.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Badge {
    pub target: BadgeTarget,
    pub tone: BadgeTone,
    /// The blocks naming it, in notebook order; the click opens the first.
    pub blocks: Vec<BlockMention>,
}

impl Badge {
    /// The number on the badge.
    pub fn count(&self) -> usize {
        self.blocks.len()
    }
}

/// The tone of `blocks`: red for a failing measurement, else amber for an
/// open issue, else green when there are measurements and all pass, else
/// blue. A measurement still planned keeps a badge blue.
pub fn worst_tone(blocks: &[BlockMention]) -> BadgeTone {
    let tones: Vec<BadgeTone> = blocks.iter().map(BlockMention::tone).collect();
    if tones.contains(&BadgeTone::Failing) {
        return BadgeTone::Failing;
    }
    if tones.contains(&BadgeTone::OpenIssue) {
        return BadgeTone::OpenIssue;
    }
    let mut measurements = blocks.iter().filter(|b| b.is_measurement()).peekable();
    let any = measurements.peek().is_some();
    if any && measurements.all(|b| b.tone() == BadgeTone::Pass) {
        BadgeTone::Pass
    } else {
        BadgeTone::Notes
    }
}

/// The badges of the notebook's blocks, each `(file, annotations)` in
/// notebook order. A block naming a target twice (`#component("U1")` and
/// `#pin("U1", "3")`) counts once. Sorted by target.
pub fn badges<'a>(files: impl IntoIterator<Item = (&'a str, &'a [Annotation])>) -> Vec<Badge> {
    let mut found: Vec<(BadgeTarget, Vec<BlockMention>)> = Vec::new();
    for (file, annotations) in files {
        for annotation in annotations {
            let mut targets: Vec<BadgeTarget> = Vec::new();
            for target in annotation.references.iter().map(BadgeTarget::of) {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
            for target in targets {
                let mention = BlockMention::of(annotation, file);
                match found.iter_mut().find(|(t, _)| *t == target) {
                    Some((_, blocks)) => blocks.push(mention),
                    None => found.push((target, vec![mention])),
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
        .into_iter()
        .map(|(target, blocks)| Badge {
            tone: worst_tone(&blocks),
            target,
            blocks,
        })
        .collect()
}

/// The badge of `target` among `badges`.
pub fn badge_of<'a>(badges: &'a [Badge], target: &BadgeTarget) -> Option<&'a Badge> {
    badges.iter().find(|badge| &badge.target == target)
}
