//! [`Diagnostic`] is the internal output type for every rule. It carries a
//! [`RuleId`] in addition to the legacy [`RuleKind`] so that DSL rules
//! (Phase 2) can emit violations without needing a `RuleKind` variant.
//!
//! [`Violation`] is kept as the public API type; [`From<Diagnostic>`] converts
//! for backward compatibility with the rest of the app.

use signex_types::schematic::{Point, SelectedItem};

use crate::net_finding::NetFinding;
use crate::rule::RuleId;
use crate::{RuleKind, Severity, Violation};

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// Which coordinate space a [`Diagnostic::location`] lives in. Only
/// [`LocationSpace::ThisSheet`] locations can be matched against the sheet's
/// No-ERC directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationSpace {
    /// A real point on the sheet being checked.
    ThisSheet,
    /// A placeholder: the finding has no point (net-scoped DSL rules).
    Unplaced,
    /// A point in a child sheet's own coordinate space.
    ChildSheet,
    /// A net-level finding anchored at the net's representative pin. The
    /// point is real but belongs to the net, not to that pin, so a positional
    /// No-ERC directive never matches it.
    Net,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule_id: RuleId,
    pub rule_kind: RuleKind,
    pub severity: Severity,
    pub message: String,
    pub location: Point,
    /// Coordinate space of `location`; gates positional No-ERC suppression.
    pub location_space: LocationSpace,
    /// For [`LocationSpace::ChildSheet`]: the `ChildSheet.filename` (as the
    /// parent references it) of the sheet `location` belongs to.
    pub child_sheet: Option<String>,
    /// For [`LocationSpace::ChildSheet`]: the resolved identity of the sheet
    /// that owns `location`. Unlike `child_sheet` (a name relative to the
    /// reporting parent) this is the same for every ancestor that sees it.
    pub owner_sheet: Option<signex_net::SheetKey>,
    pub primary: Option<SelectedItem>,
    pub peer: Option<SelectedItem>,
    /// The net a net-level finding is about (see [`NetFinding`]).
    pub net: Option<NetFinding>,
    /// Number of the pin a pin-anchored finding sits on, so two findings on
    /// two pins of one part stay two findings to a waiver.
    pub anchor_pin: Option<String>,
}

impl Diagnostic {
    /// Shorthand for built-in rules: severity comes from the kind's default.
    pub fn new(kind: RuleKind, message: impl Into<String>, location: Point) -> Self {
        Diagnostic {
            rule_id: RuleId::builtin(kind),
            rule_kind: kind,
            severity: kind.default_severity(),
            message: message.into(),
            location,
            location_space: LocationSpace::ThisSheet,
            child_sheet: None,
            owner_sheet: None,
            primary: None,
            peer: None,
            net: None,
            anchor_pin: None,
        }
    }

    /// Make this a net-level finding: it carries the net and, when the net
    /// has a pin, sits at the net's representative pin (never at a fake
    /// origin point). No-ERC directives never suppress it.
    pub fn for_net(mut self, net: NetFinding) -> Self {
        match net.representative() {
            Some(pin) => {
                self.location = pin.position;
                self.location_space = LocationSpace::Net;
            }
            None => self.location_space = LocationSpace::Unplaced,
        }
        self.net = Some(net);
        self
    }

    /// Attach the net a pin- or label-anchored finding is about, keeping its
    /// own location (and so its No-ERC suppression). The app highlights the
    /// net on click, and a per-finding waiver keys on the net's pins.
    pub fn about_net(mut self, net: NetFinding) -> Self {
        self.net = Some(net);
        self
    }

    /// Record the number of the pin this finding is anchored at.
    pub fn at_pin_number(mut self, number: impl Into<String>) -> Self {
        self.anchor_pin = Some(number.into());
        self
    }

    /// Mark `location` as a placeholder or a foreign-sheet point so a No-ERC
    /// directive on this sheet never matches it.
    pub fn in_space(mut self, space: LocationSpace) -> Self {
        self.location_space = space;
        self
    }

    /// Mark `location` as a point inside the child sheet the parent
    /// references as `filename`, so the app files it under that sheet.
    pub fn in_child_sheet(mut self, filename: impl Into<String>) -> Self {
        self.location_space = LocationSpace::ChildSheet;
        self.child_sheet = Some(filename.into());
        self
    }

    /// Record the resolved identity of the sheet that owns `location`.
    pub fn owned_by(mut self, key: Option<signex_net::SheetKey>) -> Self {
        self.owner_sheet = key;
        self
    }

    /// Grade one finding below the kind's default. Only `BadHierSheetPin`
    /// does this; every other rule takes [`RuleKind::default_severity`].
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_primary(mut self, primary: SelectedItem) -> Self {
        self.primary = Some(primary);
        self
    }

    pub fn with_peer(mut self, peer: SelectedItem) -> Self {
        self.peer = Some(peer);
        self
    }
}

// ---------------------------------------------------------------------------
// Conversion to legacy Violation
// ---------------------------------------------------------------------------

impl From<Diagnostic> for Violation {
    fn from(d: Diagnostic) -> Self {
        Violation {
            rule_id: d.rule_id,
            rule: d.rule_kind,
            severity: d.severity,
            message: d.message,
            location: d.location,
            location_space: d.location_space,
            child_sheet: d.child_sheet,
            owner_sheet: d.owner_sheet,
            primary: d.primary,
            peer: d.peer,
            net: d.net,
            anchor_pin: d.anchor_pin,
            waiver_key: None,
        }
    }
}
