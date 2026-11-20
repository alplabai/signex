//! The Custom Selection Filter dialog's draft: the footprint slice owns how a
//! draft maps onto its [`SelectionFilter`].

use super::selection_filter::{SelectionFilter, SelectionFilterKind};
use crate::app::SelectionFilterCustomState;

impl SelectionFilterCustomState {
    /// The kinds the dialog shows, in its order.
    pub const SHOWN: [SelectionFilterKind; 8] = [
        SelectionFilterKind::Pads,
        SelectionFilterKind::Tracks,
        SelectionFilterKind::Arcs,
        SelectionFilterKind::Pours,
        SelectionFilterKind::Bodies3d,
        SelectionFilterKind::Keepouts,
        SelectionFilterKind::Cutouts,
        SelectionFilterKind::Texts,
    ];

    /// A draft of `filter`, with no preset name typed.
    pub fn from_filter(f: SelectionFilter) -> Self {
        Self {
            pads: f.pads,
            tracks: f.tracks,
            arcs: f.arcs,
            pours: f.pours,
            bodies_3d: f.bodies_3d,
            keepouts: f.keepouts,
            cutouts: f.cutouts,
            texts: f.texts,
            vias: f.vias,
            regions: f.regions,
            fills: f.fills,
            other: f.other,
            preset_name: String::new(),
        }
    }

    /// The filter Apply writes to the editor.
    pub fn to_filter(&self) -> SelectionFilter {
        SelectionFilter {
            pads: self.pads,
            tracks: self.tracks,
            arcs: self.arcs,
            pours: self.pours,
            bodies_3d: self.bodies_3d,
            keepouts: self.keepouts,
            cutouts: self.cutouts,
            texts: self.texts,
            vias: self.vias,
            regions: self.regions,
            fills: self.fills,
            other: self.other,
        }
    }

    /// The flag of `kind`.
    fn flag_mut(&mut self, kind: SelectionFilterKind) -> &mut bool {
        use SelectionFilterKind as K;
        match kind {
            K::Pads => &mut self.pads,
            K::Tracks => &mut self.tracks,
            K::Arcs => &mut self.arcs,
            K::Pours => &mut self.pours,
            K::Bodies3d => &mut self.bodies_3d,
            K::Keepouts => &mut self.keepouts,
            K::Cutouts => &mut self.cutouts,
            K::Texts => &mut self.texts,
            K::Vias => &mut self.vias,
            K::Regions => &mut self.regions,
            K::Fills => &mut self.fills,
            K::Other => &mut self.other,
        }
    }

    /// Whether `kind` is selectable in the draft.
    pub fn is_on(&self, kind: SelectionFilterKind) -> bool {
        self.to_filter().get(kind)
    }

    /// Flip `kind`.
    pub fn toggle(&mut self, kind: SelectionFilterKind) {
        let flag = self.flag_mut(kind);
        *flag = !*flag;
    }

    /// All / None: every kind the dialog shows on or off.
    pub fn set_all_shown(&mut self, on: bool) {
        for kind in Self::SHOWN {
            *self.flag_mut(kind) = on;
        }
    }

    /// How many of the shown kinds are on, for the footer.
    pub fn shown_on_count(&self) -> usize {
        Self::SHOWN.iter().filter(|k| self.is_on(**k)).count()
    }

    /// The preset name: the typed one, trimmed, or "Filter N" after the
    /// `existing` presets when nothing is typed.
    pub fn preset_name_or_numbered(&self, existing: usize) -> String {
        match self.preset_name.trim() {
            "" => format!("Filter {}", existing + 1),
            typed => typed.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SelectionFilter, SelectionFilterCustomState as Draft, SelectionFilterKind};

    fn draft() -> Draft {
        Draft::from_filter(SelectionFilter::default())
    }

    /// None turns all eight shown kinds off, All turns them back on; the
    /// footer count follows.
    #[test]
    fn all_and_none_set_every_shown_kind() {
        let mut d = draft();
        d.set_all_shown(false);
        assert!(Draft::SHOWN.iter().all(|k| !d.is_on(*k)));
        assert_eq!(d.shown_on_count(), 0);
        d.set_all_shown(true);
        assert!(Draft::SHOWN.iter().all(|k| d.is_on(*k)));
        assert_eq!(d.shown_on_count(), 8);
    }

    /// The kinds the dialog does not show keep their flags through All / None.
    #[test]
    fn all_and_none_leave_hidden_kinds_alone() {
        let mut d = draft();
        d.set_all_shown(false);
        assert!(d.is_on(SelectionFilterKind::Vias));
        assert!(d.to_filter().vias);
    }

    #[test]
    fn toggle_flips_one_kind() {
        let mut d = draft();
        d.toggle(SelectionFilterKind::Tracks);
        assert!(!d.is_on(SelectionFilterKind::Tracks));
        assert_eq!(d.shown_on_count(), 7);
    }

    /// The typed name is the preset name, trimmed.
    #[test]
    fn the_typed_preset_name_is_used() {
        let mut d = draft();
        d.preset_name = "  Pads + text ".to_string();
        assert_eq!(d.preset_name_or_numbered(2), "Pads + text");
    }

    /// No name typed (or only blanks): "Filter N" after the existing presets.
    #[test]
    fn an_empty_preset_name_falls_back_to_filter_n() {
        let mut d = draft();
        assert_eq!(d.preset_name_or_numbered(0), "Filter 1");
        d.preset_name = "   ".to_string();
        assert_eq!(d.preset_name_or_numbered(2), "Filter 3");
    }

    /// The draft round-trips the editor's filter.
    #[test]
    fn the_draft_round_trips_the_filter() {
        let mut f = SelectionFilter::default();
        f.toggle(SelectionFilterKind::Pours);
        assert_eq!(Draft::from_filter(f).to_filter(), f);
    }
}
