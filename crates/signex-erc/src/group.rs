//! The groups the rule editor lists the built-in rules under (#1289): one
//! group per kind of object a rule checks.

use crate::RuleKind;

/// What a rule checks, for grouping the rule list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleGroup {
    /// Pins and their placement on the connection grid.
    Pins,
    /// Nets: endpoints, names, drivers and power.
    Nets,
    /// Net labels and their anchoring.
    Labels,
    /// Buses, bus entries and bus names.
    Buses,
    /// Sheet symbols, sheet pins, hierarchical ports, and the entries of a
    /// repeated sheet or a harness.
    Hierarchy,
    /// Parts: designators, fields, units and placement on the sheet.
    Parts,
}

impl RuleGroup {
    /// Every group, in the order the rule editor lists them.
    pub const ALL: [RuleGroup; 6] = [
        RuleGroup::Pins,
        RuleGroup::Nets,
        RuleGroup::Labels,
        RuleGroup::Buses,
        RuleGroup::Hierarchy,
        RuleGroup::Parts,
    ];

    /// The built-in rules of this group, in [`RuleKind::ALL`] order.
    pub fn rules(self) -> impl Iterator<Item = RuleKind> {
        RuleKind::ALL
            .into_iter()
            .filter(move |rule| rule.group() == self)
    }
}

impl RuleKind {
    /// The group the rule editor lists this rule under.
    pub fn group(self) -> RuleGroup {
        match self {
            RuleKind::UnusedPin
            | RuleKind::PinTypeConflict
            | RuleKind::PinsTouchNotConnected
            | RuleKind::OffGrid => RuleGroup::Pins,
            RuleKind::DanglingWire
            | RuleKind::NetMultipleNames
            | RuleKind::SinglePinNet
            | RuleKind::NetWithNoDriver
            | RuleKind::PowerInputNotDriven
            | RuleKind::LabelsOnlyNet
            | RuleKind::MissingPowerFlag
            | RuleKind::PowerPortShort
            | RuleKind::NetClassConflict
            | RuleKind::DiffPairPartnerMissing => RuleGroup::Nets,
            RuleKind::NetLabelConflict
            | RuleKind::OrphanLabel
            | RuleKind::AmbiguousLabelAnchor
            | RuleKind::LocalLabelNameOnOtherSheets => RuleGroup::Labels,
            RuleKind::BusBitWidthMismatch
            | RuleKind::BusEntryUnlabeled
            | RuleKind::BusMemberNotInBus
            | RuleKind::BusUnnamed
            | RuleKind::BusNameConflict
            | RuleKind::BusLabelOnWire => RuleGroup::Buses,
            RuleKind::HierPortDisconnected
            | RuleKind::BadHierSheetPin
            | RuleKind::SheetPinUnconnected
            | RuleKind::SheetPinDirectionConflict
            // A repeated sheet's entries (#1279) and harness entries (#1280).
            | RuleKind::RepeatEntryNeedsBus
            | RuleKind::HarnessEntryMismatch
            | RuleKind::HarnessTypeConflict => RuleGroup::Hierarchy,
            RuleKind::DuplicateRefDesignator
            | RuleKind::UnannotatedDesignator
            | RuleKind::PartWithoutFootprint
            | RuleKind::PartWithoutValue
            | RuleKind::PartWithoutMpn
            | RuleKind::PartWithoutLibraryBody
            | RuleKind::UnitsNotPlaced
            | RuleKind::SymbolOutsideSheet => RuleGroup::Parts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_has_exactly_one_group() {
        let listed: usize = RuleGroup::ALL.iter().map(|g| g.rules().count()).sum();
        assert_eq!(listed, RuleKind::ALL.len());
        for rule in RuleKind::ALL {
            let holding: Vec<RuleGroup> = RuleGroup::ALL
                .into_iter()
                .filter(|g| g.rules().any(|r| r == rule))
                .collect();
            assert_eq!(holding, [rule.group()], "{rule:?}");
        }
    }

    #[test]
    fn every_group_holds_its_rules() {
        let sizes: Vec<usize> = RuleGroup::ALL.iter().map(|g| g.rules().count()).collect();
        assert_eq!(sizes, [4, 9, 4, 6, 7, 8]);
    }

    #[test]
    fn each_rule_sits_in_the_group_of_what_it_checks() {
        use RuleGroup::*;
        let expected = [
            (RuleKind::UnusedPin, Pins),
            (RuleKind::DuplicateRefDesignator, Parts),
            (RuleKind::HierPortDisconnected, Hierarchy),
            (RuleKind::DanglingWire, Nets),
            (RuleKind::NetLabelConflict, Labels),
            (RuleKind::OrphanLabel, Labels),
            (RuleKind::BusBitWidthMismatch, Buses),
            (RuleKind::BadHierSheetPin, Hierarchy),
            (RuleKind::MissingPowerFlag, Nets),
            (RuleKind::PowerPortShort, Nets),
            (RuleKind::SymbolOutsideSheet, Parts),
            (RuleKind::AmbiguousLabelAnchor, Labels),
            (RuleKind::PinTypeConflict, Pins),
            (RuleKind::PinsTouchNotConnected, Pins),
            (RuleKind::BusEntryUnlabeled, Buses),
            (RuleKind::BusMemberNotInBus, Buses),
            (RuleKind::BusUnnamed, Buses),
            (RuleKind::BusNameConflict, Buses),
            (RuleKind::BusLabelOnWire, Buses),
            (RuleKind::NetMultipleNames, Nets),
            (RuleKind::LocalLabelNameOnOtherSheets, Labels),
            (RuleKind::PartWithoutFootprint, Parts),
            (RuleKind::PartWithoutValue, Parts),
            (RuleKind::PartWithoutMpn, Parts),
            (RuleKind::PartWithoutLibraryBody, Parts),
            (RuleKind::UnannotatedDesignator, Parts),
            (RuleKind::SinglePinNet, Nets),
            (RuleKind::NetWithNoDriver, Nets),
            (RuleKind::PowerInputNotDriven, Nets),
            (RuleKind::UnitsNotPlaced, Parts),
            (RuleKind::OffGrid, Pins),
            (RuleKind::LabelsOnlyNet, Nets),
            (RuleKind::SheetPinUnconnected, Hierarchy),
            (RuleKind::SheetPinDirectionConflict, Hierarchy),
            (RuleKind::RepeatEntryNeedsBus, Hierarchy),
            (RuleKind::HarnessEntryMismatch, Hierarchy),
            (RuleKind::HarnessTypeConflict, Hierarchy),
            (RuleKind::NetClassConflict, Nets),
            (RuleKind::DiffPairPartnerMissing, Nets),
        ];
        assert_eq!(expected.len(), RuleKind::ALL.len());
        for (rule, group) in expected {
            assert_eq!(rule.group(), group, "{rule:?}");
        }
    }
}
