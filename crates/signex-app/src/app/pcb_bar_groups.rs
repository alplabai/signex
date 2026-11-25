//! The tool groups of the PCB Active Bar (#2392). A group is one button: it
//! shows and runs the group's last-used row and its right-click opens a
//! dropdown of every row. A row is a board placement tool from
//! `pcb_tools.rs`, greyed with its reason while the board editor cannot run
//! it, or a catalog command. Which row each group last ran is kept in
//! `prefs.json` (`pcb_bar_last_tools`), so it outlives the session.

use std::collections::BTreeMap;

use signex_types::pcb_dimension_kind::DimensionKind;

use crate::app::pcb_tools::BoardPlaceTool;

/// A tool group of the PCB Active Bar, in the bar's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PcbBarGroup {
    /// Place a part, swap its footprint.
    Components,
    /// Route copper between pads: one net, a pair, straight, a bus.
    Route,
    /// Lengthen routed nets with meanders (#2389).
    Tune,
    /// Via and free pad.
    ViaPad,
    /// Copper areas: polygon pour and its cutout, slice and repour (#2216),
    /// solid region, fill, and the keepouts
    /// that hold copper out of an area; then rooms, the areas a sheet's
    /// parts are placed in (#2457).
    CopperAreas,
    /// Dimensions on the board, one row per kind (#2507).
    Dimensions,
    /// Board text.
    Text,
    /// Lines, arcs, circles, rectangles and text frames (#2424); a line or
    /// arc on copper is a track (#2869), so the bar has no copper drawing
    /// group: Track and the copper Arc stay in Place.
    Graphics,
}

/// A row of a group: a board placement tool, or a catalog command that is
/// not a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcbBarRow {
    Tool(BoardPlaceTool),
    Command(&'static str),
}

impl PcbBarRow {
    /// The command the row runs; `None` while its tool cannot run on a
    /// board yet.
    pub(crate) fn command(self) -> Option<&'static str> {
        match self {
            Self::Tool(tool) => tool.command(),
            Self::Command(command) => Some(command),
        }
    }
}

impl PcbBarGroup {
    /// The groups in the bar's order.
    pub(crate) const ALL: [Self; 8] = [
        Self::Components,
        Self::Route,
        Self::Tune,
        Self::ViaPad,
        Self::CopperAreas,
        Self::Dimensions,
        Self::Text,
        Self::Graphics,
    ];

    /// The group's rows, top to bottom. The first live row is what the
    /// button runs until another row is used.
    pub(crate) fn rows(self) -> &'static [PcbBarRow] {
        use BoardPlaceTool as T;
        use PcbBarRow::{Command, Tool};
        match self {
            Self::Components => &[Tool(T::Component), Command("change_footprint")],
            Self::Route => &[
                Tool(T::InteractiveRouting),
                Tool(T::DiffPairRouting),
                Tool(T::QuickRoute),
                Tool(T::MultiRoute),
            ],
            Self::Tune => &[Command("tune_net_length"), Command("tune_diff_pair")],
            Self::ViaPad => &[Tool(T::Via), Tool(T::Pad)],
            Self::CopperAreas => &[
                Tool(T::PolygonPour),
                Tool(T::PourCutout),
                Tool(T::SlicePour),
                Tool(T::SolidRegion),
                Tool(T::Fill),
                Tool(T::Keepout),
                Tool(T::Room),
                Command("repour_selected"),
                Command("repour_all"),
                Command("unfill_selected"),
            ],
            // #2507: one row per dimension kind.
            Self::Dimensions => &[
                Tool(T::Dimension(DimensionKind::Aligned)),
                Tool(T::Dimension(DimensionKind::Horizontal)),
                Tool(T::Dimension(DimensionKind::Vertical)),
                Tool(T::Dimension(DimensionKind::Radial)),
            ],
            Self::Text => &[Tool(T::TextString)],
            Self::Graphics => &[
                Tool(T::Line),
                Tool(T::GraphicArc),
                Tool(T::FullCircle),
                Tool(T::Rectangle),
                Tool(T::TextFrame),
            ],
        }
    }

    /// The group's key in `prefs.json`.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Components => "components",
            Self::Route => "route",
            Self::Tune => "tune",
            Self::ViaPad => "via_pad",
            Self::CopperAreas => "copper_areas",
            Self::Dimensions => "dimensions",
            Self::Text => "text",
            Self::Graphics => "graphics",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|group| group.key() == key)
    }

    /// The live row of the group that runs `command`.
    pub(crate) fn live_row(self, command: &str) -> Option<PcbBarRow> {
        self.rows()
            .iter()
            .copied()
            .find(|row| row.command() == Some(command))
    }

    /// The group whose rows include `tool`.
    pub(crate) fn of_tool(tool: BoardPlaceTool) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|group| group.rows().contains(&PcbBarRow::Tool(tool)))
    }
}

/// The command each group last ran. Only a live row is kept: a command
/// that is no longer a live row of its group reads as never used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PcbBarLastTools {
    by_group: BTreeMap<PcbBarGroup, &'static str>,
}

impl PcbBarLastTools {
    /// The row `group`'s button shows and runs: the last-used row while it
    /// is live, else the first live row, else the first row (greyed).
    pub(crate) fn current(&self, group: PcbBarGroup) -> PcbBarRow {
        let rows = group.rows();
        self.by_group
            .get(&group)
            .and_then(|command| group.live_row(command))
            .or_else(|| rows.iter().copied().find(|row| row.command().is_some()))
            .unwrap_or(rows[0])
    }

    /// Note that `group` ran `command`. Whether that changed what the group
    /// remembers; a command that is not a live row of `group` changes
    /// nothing.
    pub(crate) fn remember(&mut self, group: PcbBarGroup, command: &str) -> bool {
        let Some(command) = group.live_row(command).and_then(PcbBarRow::command) else {
            return false;
        };
        self.by_group.insert(group, command) != Some(command)
    }

    /// The `prefs.json` value: group key to command id.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let map = self
            .by_group
            .iter()
            .map(|(group, command)| (group.key().to_string(), serde_json::Value::from(*command)))
            .collect();
        serde_json::Value::Object(map)
    }

    /// Read the `prefs.json` value. An unknown group, a command that is not
    /// a live row of its group, or a value of the wrong shape is skipped.
    pub(crate) fn from_json(value: Option<&serde_json::Value>) -> Self {
        let mut last = Self::default();
        let Some(map) = value.and_then(serde_json::Value::as_object) else {
            return last;
        };
        for (key, command) in map {
            let (Some(group), Some(command)) = (PcbBarGroup::from_key(key), command.as_str())
            else {
                continue;
            };
            last.remember(group, command);
        }
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::pcb_tools::ToolState;

    /// Every board placement tool is a row of exactly one group, so the bar
    /// offers each Place row once; Track and the copper Arc are in Place
    /// only, as the Graphics Line and Arc draw them on copper (#2869).
    #[test]
    fn every_place_tool_is_in_one_group() {
        let tools = BoardPlaceTool::PLACE_MENU
            .into_iter()
            .chain(BoardPlaceTool::DIMENSION_MENU)
            .chain(BoardPlaceTool::GRAPHICS_MENU);
        for tool in tools {
            let groups = PcbBarGroup::ALL
                .into_iter()
                .filter(|group| group.rows().contains(&PcbBarRow::Tool(tool)))
                .count();
            let expected = match tool {
                BoardPlaceTool::Track | BoardPlaceTool::Arc => 0,
                _ => 1,
            };
            assert_eq!(groups, expected, "{tool:?}");
        }
        assert_eq!(PcbBarGroup::of_tool(BoardPlaceTool::Track), None);
        assert_eq!(PcbBarGroup::from_key("copper_drawing"), None, "#2869");
    }

    /// A group starts on its first live row, follows the row used last, and
    /// never shows a row that cannot run while a live one exists.
    #[test]
    fn a_group_shows_its_last_live_row() {
        let mut last = PcbBarLastTools::default();
        let areas = PcbBarGroup::CopperAreas;
        assert_eq!(
            last.current(areas),
            PcbBarRow::Tool(BoardPlaceTool::PolygonPour),
            "#2216: the first row is live"
        );
        let mut parts = PcbBarLastTools::default();
        assert!(matches!(
            BoardPlaceTool::Component.state(),
            ToolState::Unavailable(_)
        ));
        assert!(!parts.remember(PcbBarGroup::Components, "place_board_component"));
        assert_eq!(
            parts.current(PcbBarGroup::Components),
            PcbBarRow::Command("change_footprint"),
            "a greyed row is never shown while a live one exists"
        );
        // #2424: the Graphics group follows its last live row.
        let mut drawing = PcbBarLastTools::default();
        assert!(drawing.remember(PcbBarGroup::Graphics, "place_board_graphic_arc"));
        assert_eq!(
            drawing.current(PcbBarGroup::Graphics),
            PcbBarRow::Tool(BoardPlaceTool::GraphicArc)
        );
        assert!(last.remember(areas, "place_board_fill"));
        assert!(!last.remember(areas, "place_board_fill"), "unchanged");
        assert_eq!(last.current(areas), PcbBarRow::Tool(BoardPlaceTool::Fill));
        assert!(!last.remember(areas, "place_board_via"), "not this group's");
        assert_eq!(
            last.current(PcbBarGroup::Dimensions),
            PcbBarRow::Tool(BoardPlaceTool::Dimension(DimensionKind::Aligned)),
            "#2373: Dimension is live"
        );
        // #2507: the Dimensions group follows the kind used last.
        let mut kinds = PcbBarLastTools::default();
        assert!(kinds.remember(PcbBarGroup::Dimensions, "place_board_dimension_radial"));
        assert_eq!(
            kinds.current(PcbBarGroup::Dimensions),
            PcbBarRow::Tool(BoardPlaceTool::Dimension(DimensionKind::Radial))
        );
    }

    /// The `prefs.json` value round-trips and skips what it cannot use.
    #[test]
    fn last_tools_round_trip_through_json() {
        let mut last = PcbBarLastTools::default();
        last.remember(PcbBarGroup::Graphics, "place_board_line");
        last.remember(PcbBarGroup::CopperAreas, "place_board_keepout");
        let json = last.to_json();
        assert_eq!(PcbBarLastTools::from_json(Some(&json)), last);
        let odd = serde_json::json!({
            "copper_areas": "place_board_via",
            "no_such_group": "place_board_track",
            "via_pad": 7,
        });
        assert_eq!(
            PcbBarLastTools::from_json(Some(&odd)),
            PcbBarLastTools::default()
        );
        assert_eq!(PcbBarLastTools::from_json(None), PcbBarLastTools::default());
    }
}
