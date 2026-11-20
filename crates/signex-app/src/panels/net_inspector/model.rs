//! What the Net Inspector shows: its own state (tab, filter, expanded rows)
//! and the rows the panel context copies from the project net index. Plain
//! data; the rows are built in `handlers/net_nav/inspector.rs` and
//! `handlers/net_nav/component_rows.rs`.

use std::collections::BTreeSet;

use signex_net::NetAnchor;

/// How many members an expanded net (or pins an expanded part) lists before
/// a "+ N more" row; a click on that row lists the rest.
pub const MEMBER_PREVIEW: usize = 6;

/// Which list the panel shows for a schematic project.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NetInspectorTab {
    /// Every net with its members.
    #[default]
    ByNet,
    /// Every part with its pins and the net of each (#1274).
    ByComponent,
}

/// The panel's own state, kept in `UiState`; the panel context carries a
/// copy. Rows are remembered by net name and by part key
/// ([`ComponentRow::key`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NetInspectorState {
    pub tab: NetInspectorTab,
    pub filter: String,
    /// Nets shown expanded.
    pub expanded: BTreeSet<String>,
    /// Parts shown expanded.
    pub expanded_parts: BTreeSet<String>,
    /// Expanded nets listing every member, past [`MEMBER_PREVIEW`].
    pub full_nets: BTreeSet<String>,
    /// Expanded parts listing every pin, past [`MEMBER_PREVIEW`].
    pub full_parts: BTreeSet<String>,
}

impl NetInspectorState {
    /// Expand or fold row `name` of `tab`; a folded row also stops listing in full.
    pub(crate) fn toggle(&mut self, tab: NetInspectorTab, name: String) {
        let (expanded, full) = match tab {
            NetInspectorTab::ByNet => (&mut self.expanded, &mut self.full_nets),
            NetInspectorTab::ByComponent => (&mut self.expanded_parts, &mut self.full_parts),
        };
        if expanded.remove(&name) {
            full.remove(&name);
        } else {
            expanded.insert(name);
        }
    }

    /// List every member of `name` in the current tab.
    pub(crate) fn show_all(&mut self, name: String) {
        match self.tab {
            NetInspectorTab::ByNet => self.full_nets.insert(name),
            NetInspectorTab::ByComponent => self.full_parts.insert(name),
        };
    }
}

/// One net of the board in view (#2403), from `pcb_geom::net_reports`.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardNetRow {
    pub name: String,
    pub routed_length_mm: f64,
    pub vias: usize,
    pub pads: usize,
    /// Connections still to route; 0 when the net is routed.
    pub unrouted: usize,
}

/// What the panel lists with a board in view: its nets the filter keeps.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardNetView {
    pub nets: Vec<BoardNetRow>,
    /// Every net of the board.
    pub total: usize,
}

/// Where the list comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetListStatus {
    /// The active document is not a schematic of a project (or none is open).
    #[default]
    NoProject,
    /// The project's net index is being built.
    Reading,
    /// The last build failed (the reason is in Messages).
    Unavailable,
    Ready,
}

/// What a member of a net is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetMemberRowKind {
    Pin,
    Label,
    PowerPort,
    SheetEntry,
}

/// One member of an expanded net.
#[derive(Debug, Clone, PartialEq)]
pub struct NetMemberRow {
    pub kind: NetMemberRowKind,
    /// `R1.2` for a pin, else the label, port or entry text.
    pub text: String,
    /// A pin's name from its library symbol; empty for other members and
    /// for a pin without a library name.
    pub pin_name: String,
    /// The sheet it sits on, as the project names it.
    pub sheet: String,
    pub anchor: NetAnchor,
}

/// One net row.
#[derive(Debug, Clone, PartialEq)]
pub struct NetRow {
    pub name: String,
    /// The first net class that contains the net.
    pub class: Option<String>,
    /// Component pins on the net.
    pub pins: usize,
    pub expanded: bool,
    /// Filled only while expanded: the first [`MEMBER_PREVIEW`] unless the
    /// net is listed in full.
    pub members: Vec<NetMemberRow>,
    /// Members of an expanded net not listed in `members`.
    pub more: usize,
}

/// One pin of an expanded part.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPinRow {
    /// The pin number (else its name), as By Net writes it after the dot.
    pub pin: String,
    /// The pin's name; empty when the library gives none.
    pub name: String,
    /// The net the pin is on; `None` for an unconnected pin.
    pub net: Option<String>,
    /// The sheet the pin's unit sits on, as the project names it.
    pub sheet: String,
    /// Where a click goes: the pin on its sheet.
    pub anchor: NetAnchor,
}

/// One part row.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRow {
    /// What the expanded set knows the part by.
    pub key: String,
    pub designator: String,
    pub value: String,
    pub footprint: String,
    /// Every pin of the part, wired or not.
    pub pins: usize,
    pub expanded: bool,
    /// Filled only while expanded, in pin-number order.
    pub pin_rows: Vec<ComponentPinRow>,
    /// Pins of an expanded part not listed in `pin_rows`.
    pub more: usize,
}

/// What the panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetInspectorView {
    pub tab: NetInspectorTab,
    pub filter: String,
    pub status: NetListStatus,
    /// The nets the filter keeps, named nets first, then by name. Filled
    /// only on the By Net tab.
    pub nets: Vec<NetRow>,
    /// Every net of the project.
    pub total: usize,
    /// The parts the filter keeps, by designator. Filled only on the By
    /// Component tab.
    pub components: Vec<ComponentRow>,
    /// Every part of the project, counted on both tabs for the tab label.
    pub component_total: usize,
    /// With a board in view, its nets instead of the project's (#2403).
    pub board: Option<BoardNetView>,
}

/// The first [`MEMBER_PREVIEW`] of `items` unless `full`, and how many were
/// left out.
pub(crate) fn preview<T>(mut items: Vec<T>, full: bool) -> (Vec<T>, usize) {
    if full || items.len() <= MEMBER_PREVIEW {
        return (items, 0);
    }
    let more = items.len() - MEMBER_PREVIEW;
    items.truncate(MEMBER_PREVIEW);
    (items, more)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_list_shows_six_and_counts_the_rest() {
        let (shown, more) = preview((0..9).collect::<Vec<_>>(), false);
        assert_eq!(shown, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(more, 3);
        assert_eq!(preview((0..9).collect::<Vec<_>>(), true).1, 0);
        assert_eq!(
            preview((0..6).collect::<Vec<_>>(), false),
            ((0..6).collect(), 0)
        );
    }

    #[test]
    fn folding_a_row_forgets_that_it_was_listed_in_full() {
        let mut state = NetInspectorState::default();
        state.toggle(NetInspectorTab::ByNet, "GND".into());
        state.show_all("GND".into());
        assert!(state.full_nets.contains("GND"));
        state.toggle(NetInspectorTab::ByNet, "GND".into());
        assert!(!state.expanded.contains("GND") && state.full_nets.is_empty());

        state.tab = NetInspectorTab::ByComponent;
        state.toggle(NetInspectorTab::ByComponent, "U1".into());
        state.show_all("U1".into());
        assert!(state.expanded_parts.contains("U1") && state.full_parts.contains("U1"));
        assert!(state.expanded.is_empty(), "the tabs keep their own rows");
    }
}
