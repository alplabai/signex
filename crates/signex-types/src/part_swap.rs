//! Which pins and which units (gates) of a part may trade nets (#2410). The
//! library part states it in two parameters, carried onto every placed unit
//! like its other parameters: `Pin Swap Groups` lists groups of pin numbers,
//! `Gate Swap Groups` groups of unit numbers. Groups are separated by `;`,
//! members by `,`: `1,2;5,6,7` lets pins 1 and 2 trade, and any two of 5, 6
//! and 7. A part without the parameter allows no swap. See
//! `docs/PCB_TOOLS.md`, "Pin and gate swap".

use crate::schematic::Symbol;

/// The part parameter that lists the pin swap groups.
pub const PIN_SWAP_GROUPS_FIELD: &str = "Pin Swap Groups";

/// The part parameter that lists the gate (unit) swap groups.
pub const GATE_SWAP_GROUPS_FIELD: &str = "Gate Swap Groups";

/// Groups of members (pin numbers or unit numbers) that may trade places.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SwapGroups {
    groups: Vec<Vec<String>>,
}

impl SwapGroups {
    /// Read `text`: groups split by `;`, members by `,`, each trimmed. Empty
    /// members are dropped, and so is a group left with fewer than two.
    pub fn parse(text: &str) -> Self {
        let groups = text
            .split(';')
            .map(|group| {
                let mut members: Vec<String> = Vec::new();
                for member in group.split(',').map(str::trim).filter(|m| !m.is_empty()) {
                    if !members.iter().any(|m| m == member) {
                        members.push(member.to_string());
                    }
                }
                members
            })
            .filter(|members| members.len() >= 2)
            .collect();
        Self { groups }
    }

    /// The pin swap groups `symbol` carries; none when it has no parameter.
    pub fn pins_of(symbol: &Symbol) -> Self {
        Self::of_field(symbol, PIN_SWAP_GROUPS_FIELD)
    }

    /// The gate swap groups `symbol` carries; none when it has no parameter.
    pub fn gates_of(symbol: &Symbol) -> Self {
        Self::of_field(symbol, GATE_SWAP_GROUPS_FIELD)
    }

    fn of_field(symbol: &Symbol, key: &str) -> Self {
        symbol
            .fields
            .get(key)
            .map(|text| Self::parse(text))
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn groups(&self) -> &[Vec<String>] {
        &self.groups
    }

    /// The index of the first group that lists `member` (trimmed).
    pub fn group_of(&self, member: &str) -> Option<usize> {
        let member = member.trim();
        self.groups
            .iter()
            .position(|group| group.iter().any(|m| m == member))
    }

    /// Whether `a` and `b` are two different members of one group.
    pub fn same_group(&self, a: &str, b: &str) -> bool {
        a.trim() != b.trim() && self.group_of(a).is_some() && self.group_of(a) == self.group_of(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_split_on_semicolons_and_members_on_commas() {
        let groups = SwapGroups::parse(" 1, 2 ; 5,6,7 ");
        assert_eq!(
            groups.groups(),
            [
                vec!["1".to_string(), "2".to_string()],
                vec!["5".to_string(), "6".to_string(), "7".to_string()]
            ]
        );
        assert!(groups.same_group("1", "2"));
        assert!(groups.same_group("7", "5"));
        assert!(!groups.same_group("1", "5"), "two groups");
        assert!(!groups.same_group("1", "1"), "one pin is no swap");
        assert!(!groups.same_group("3", "4"), "unlisted pins");
    }

    #[test]
    fn a_group_of_one_and_empty_members_are_dropped() {
        let groups = SwapGroups::parse("1;;2,,3;4,4");
        assert_eq!(groups.groups(), [vec!["2".to_string(), "3".to_string()]]);
        assert!(SwapGroups::parse("").is_empty());
    }

    #[test]
    fn a_symbol_without_the_parameter_allows_no_swap() {
        let mut symbol = Symbol::default();
        assert!(SwapGroups::pins_of(&symbol).is_empty());
        symbol
            .fields
            .insert(GATE_SWAP_GROUPS_FIELD.to_string(), "1,2,3,4".to_string());
        assert!(SwapGroups::gates_of(&symbol).same_group("2", "4"));
        assert!(SwapGroups::pins_of(&symbol).is_empty());
    }
}
