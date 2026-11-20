//! The linetype new sketch entities get: the Active Bar's three-way
//! segment (Normal / Construction / Centerline). Construction and
//! centerline entities never bake.

use crate::library::editor::footprint::state::FootprintEditorState;

/// One segment of the Linetype control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchLinetype {
    Normal,
    Construction,
    Centerline,
}

impl SketchLinetype {
    /// The segments, left to right.
    pub const ALL: [Self; 3] = [Self::Normal, Self::Construction, Self::Centerline];

    /// The linetype the editor draws with now.
    pub fn of(state: &FootprintEditorState) -> Self {
        if state.centerline_mode {
            Self::Centerline
        } else if state.construction_mode {
            Self::Construction
        } else {
            Self::Normal
        }
    }

    /// Make this the linetype new entities get. The two flags stay
    /// mutually exclusive.
    pub fn apply_to(self, state: &mut FootprintEditorState) {
        state.construction_mode = self == Self::Construction;
        state.centerline_mode = self == Self::Centerline;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_segment_sets_exactly_its_flag() {
        let mut state =
            FootprintEditorState::from_footprint(&signex_library::Footprint::empty("t"));
        for linetype in SketchLinetype::ALL {
            linetype.apply_to(&mut state);
            assert_eq!(SketchLinetype::of(&state), linetype);
            assert!(!(state.construction_mode && state.centerline_mode));
        }
    }
}
