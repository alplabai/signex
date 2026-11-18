//! "Why this value?" (#2398): the value that governs one net's side of a
//! limit, where it comes from, and how every written rule of that limit
//! fared, by the same resolution the check uses (`user_rules::pick`). See
//! `docs/DRC_RULES.md`, "Which rule applies".

use crate::user_rules::{self, CopperObject, LimitKind, ScopedRule};
use crate::{DrcRules, Violation};

/// How one written rule fared for one net, layer and object pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// It sets the value.
    Won,
    /// A more specific rule covers the same net here.
    LessSpecific,
    /// An equally specific rule covers it and is stricter, or is the same
    /// and written first.
    LessStrict,
    /// Its net, class or pattern does not take in this net.
    OtherNets,
    /// Its layer condition does not take in this layer.
    OtherLayers,
    /// It is for another pair of objects.
    OtherObjects,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Verdict::Won => "won",
            Verdict::LessSpecific => "less_specific",
            Verdict::LessStrict => "less_strict",
            Verdict::OtherNets => "other_nets",
            Verdict::OtherLayers => "other_layers",
            Verdict::OtherObjects => "other_objects",
        }
    }
}

/// One written rule that limits the same thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub value_nm: i64,
    /// `(scope, narrowing conditions)`; higher wins.
    pub specificity: (u8, u8),
    pub verdict: Verdict,
}

/// Where a governing value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A written rule, by name.
    Rule(String),
    /// The net's class clearance, larger than the board setup's.
    NetClass,
    /// The board setup clearance (or the default when it sets none).
    BoardSetup,
    /// Nothing limits it.
    Unlimited,
}

/// The value one net's side gets, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub kind: LimitKind,
    pub net: u32,
    /// The copper layer bit the value holds on: the strictest one.
    pub layer: Option<u32>,
    pub value_nm: Option<i64>,
    pub source: Source,
    /// What the net would get without written rules (clearance only).
    pub implicit_nm: Option<i64>,
    /// Every written rule of `kind`, on [`Self::layer`], in written order.
    pub chain: Vec<Candidate>,
}

/// Why a violation asks what it asks: each side's value; the pair needs
/// the stricter one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    pub kind: LimitKind,
    pub sides: Vec<Effective>,
    pub required_nm: i64,
}

impl DrcRules {
    /// The value of `kind` that governs `net` where it spans `layers`, with
    /// `objects` naming the two kinds of a clearance pair (`[None, None]`
    /// for a single object). Per copper layer the most specific written
    /// rule wins, else the implicit value; the strictest layer holds.
    pub fn effective(
        &self,
        kind: LimitKind,
        net: u32,
        layers: u32,
        objects: [Option<CopperObject>; 2],
    ) -> Effective {
        self.effective_between(kind, net, None, layers, objects)
    }

    /// [`Self::effective`] for one side of a clearance whose other side is
    /// net `other`: a class-pair rule (#2398) takes part when the two nets
    /// sit in its two classes.
    pub fn effective_between(
        &self,
        kind: LimitKind,
        net: u32,
        other: Option<u32>,
        layers: u32,
        objects: [Option<CopperObject>; 2],
    ) -> Effective {
        let layers = self.on_board(layers);
        let implicit = (kind == LimitKind::MinClearance).then(|| self.required(net, net));
        let mut held: Option<(u32, i64, Option<&ScopedRule>)> = None;
        for bit in (0..32).filter(|bit| layers & (1 << bit) != 0) {
            let picked = user_rules::pick_between(&self.user, kind, net, other, 1 << bit, objects);
            let side = match picked {
                Some((nm, scoped)) => Some((nm, Some(scoped))),
                None => implicit.map(|nm| (nm, None)),
            };
            let Some((nm, rule)) = side else {
                continue;
            };
            let replaces = held.is_none_or(|(_, h, h_rule)| {
                kind.stricter(nm, h) || (nm == h && h_rule.is_none() && rule.is_some())
            });
            if replaces {
                held = Some((bit, nm, rule));
            }
        }
        let layer = held
            .map(|(bit, _, _)| bit)
            .or_else(|| (0..32).find(|bit| layers & (1 << bit) != 0));
        let chain = layer
            .map(|bit| self.candidates(kind, net, other, 1 << bit, objects))
            .unwrap_or_default();
        let source = match held {
            Some((_, _, Some(scoped))) => Source::Rule(scoped.rule.name.clone()),
            Some((_, _, None)) if self.net_clearance.get(&net).copied() > Some(self.clearance) => {
                Source::NetClass
            }
            Some((_, _, None)) => Source::BoardSetup,
            None => Source::Unlimited,
        };
        Effective {
            kind,
            net,
            layer,
            value_nm: held.map(|(_, nm, _)| nm),
            source,
            implicit_nm: implicit,
            chain,
        }
    }

    /// Every written rule of `kind` and how it fared for `net` on the one
    /// layer `layer_mask`.
    fn candidates(
        &self,
        kind: LimitKind,
        net: u32,
        other: Option<u32>,
        layer_mask: u32,
        objects: [Option<CopperObject>; 2],
    ) -> Vec<Candidate> {
        let winner = user_rules::pick_between(&self.user, kind, net, other, layer_mask, objects)
            .map(|(_, s)| s);
        let mut out = Vec::new();
        for scoped in &self.user {
            let Some(value_nm) = scoped.rule.limit(kind) else {
                continue;
            };
            let verdict = if !scoped.covers_net(net) || !scoped.covers_partner(net, other) {
                Verdict::OtherNets
            } else if !scoped.covers(net, layer_mask) {
                Verdict::OtherLayers
            } else if !scoped.pairs_with(objects) {
                Verdict::OtherObjects
            } else {
                match winner {
                    Some(won) if std::ptr::eq(won, scoped) => Verdict::Won,
                    Some(won) if won.rule.rank() > scoped.rule.rank() => Verdict::LessSpecific,
                    _ => Verdict::LessStrict,
                }
            };
            out.push(Candidate {
                name: scoped.rule.name.clone(),
                value_nm,
                specificity: scoped.rule.rank(),
                verdict,
            });
        }
        out
    }

    /// Why `violation` asks its `required_nm`: one side per net for a
    /// clearance or short, the item's side for a width or via limit;
    /// `None` for an unconnected link or a keepout.
    pub fn explain(&self, violation: &Violation) -> Option<Explanation> {
        // A kind with a minimum and a maximum broke the maximum when the
        // value measured is over the limit.
        let kind = match violation.kind.limits() {
            [] => return None,
            [min, max] if violation.measured_nm > violation.required_nm => {
                debug_assert!(max.is_maximum() && !min.is_maximum());
                *max
            }
            [first, ..] => *first,
        };
        let sides = if kind == LimitKind::MinClearance {
            let objects = [
                violation.items.first().and_then(CopperObject::of),
                violation.items.get(1).and_then(CopperObject::of),
            ];
            let (a, b) = violation.nets;
            let layers = violation.layers;
            let mut sides = vec![self.effective_between(kind, a, Some(b), layers, objects)];
            if b != a {
                sides.push(self.effective_between(kind, b, Some(a), layers, objects));
            }
            sides
        } else {
            // A finding off copper (silkscreen, mask, courtyard) has no
            // layers; its board-wide rule is read on every layer.
            let layers = match violation.layers {
                0 => u32::MAX,
                layers => layers,
            };
            let (a, b) = violation.nets;
            let mut sides = vec![self.effective(kind, a, layers, [None, None])];
            if kind == LimitKind::MinHoleToHole && b != a {
                sides.push(self.effective(kind, b, layers, [None, None]));
            }
            sides
        };
        Some(Explanation {
            kind,
            sides,
            required_nm: violation.required_nm,
        })
    }
}
