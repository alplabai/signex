//! The signal harness tools (#1280). Signal Harness draws harness wire
//! segments click by click like the bus tool; Harness Connector is two
//! clicks (the corner of the body, then its width; a second click left of the
//! first turns the tip left); Harness Entry is one click on a connector's
//! flat side. Each placement is one `Command`, so one undo step. What a new
//! connector or entry is called is decided in `signex-types`
//! (`new_connector_type`, `seeded_entries`, `next_entry_name`). Model and
//! rules: `docs/SIGNAL_HARNESSES.md`.

use signex_types::corner::CornerRule;
use signex_types::schematic::{
    DEFAULT_HARNESS_ENTRY_PITCH_MM, HarnessConnector, HarnessDefinition, HarnessEntry, HarnessWire,
    Point, new_connector_type, next_entry_name, seeded_entries,
};

use super::super::super::*;
use crate::canvas::ShapePreviewKind;

/// How far outside a connector's body a Harness Entry click still picks it
/// (mm).
const ENTRY_REACH_MM: f64 = 1.27;

/// The commands that place `segments` as harness wires: one per segment.
pub(crate) fn harness_wire_commands(segments: &[(Point, Point)]) -> Vec<signex_engine::Command> {
    segments
        .iter()
        .filter(|(a, b)| a != b)
        .map(|seg| signex_engine::Command::PlaceHarnessWire {
            wire: HarnessWire {
                uuid: uuid::Uuid::now_v7(),
                start: seg.0,
                end: seg.1,
                locked: false,
            },
        })
        .collect()
}

/// A connector of `harness_type` with its body's corner at `anchor` and its
/// width up to `corner`: a corner left of the anchor turns the tip left. The
/// width is at least one coarse grid step.
pub(crate) fn connector_from_clicks(
    anchor: Point,
    corner: Point,
    harness_type: String,
    entries: Vec<HarnessEntry>,
) -> HarnessConnector {
    let dx = corner.x - anchor.x;
    let width = dx.abs().max(signex_types::schematic::GRID_MM);
    HarnessConnector {
        uuid: uuid::Uuid::now_v7(),
        harness_type,
        position: Point::new(anchor.x.min(corner.x), anchor.y),
        width,
        entry_pitch: DEFAULT_HARNESS_ENTRY_PITCH_MM,
        rotation: if dx < 0.0 { 180.0 } else { 0.0 },
        entries,
        locked: false,
    }
}

impl Signex {
    /// The harness definitions of the project that owns the sheet in view;
    /// empty without one.
    pub(crate) fn harness_definitions_in_view(&self) -> Vec<HarnessDefinition> {
        self.project_in_view()
            .and_then(|id| self.document_state.project_by_id(id))
            .map(|p| p.data.schematic_design.harness_definitions.clone())
            .unwrap_or_default()
    }

    /// Place harness wire segments from `start` to `end`, bent as `rule`
    /// bends them. The point the last segment ends on and whether the
    /// document changed.
    pub(crate) fn place_harness_wire_at(
        &mut self,
        start: Point,
        end: Point,
        rule: CornerRule,
    ) -> (Point, bool) {
        let segments = rule.legs(start, end);
        let commands = harness_wire_commands(&segments);
        let changed = !commands.is_empty() && self.apply_engine_commands(commands, false, false);
        (segments.last().map(|s| s.1).unwrap_or(end), changed)
    }

    /// One snapped click of the Harness Connector tool: the first anchors
    /// the body's corner, the second places the connector (the type last
    /// used, else the project's first definition, its entries seeded from
    /// the type's definition) and leaves the tool armed.
    pub(crate) fn harness_connector_click(&mut self, at: Point) {
        let Some(anchor) = self.interaction_state.shape_anchor.take() else {
            self.set_shape_anchor(Some((at, ShapePreviewKind::Rect)));
            return;
        };
        self.set_shape_anchor(None);
        let definitions = self.harness_definitions_in_view();
        let harness_type = new_connector_type(
            self.interaction_state.last_harness_type.as_deref(),
            &definitions,
        );
        let entries = seeded_entries(&harness_type, &definitions, uuid::Uuid::now_v7);
        let connector = connector_from_clicks(anchor, at, harness_type.clone(), entries);
        if self.apply_engine_command(
            signex_engine::Command::PlaceHarnessConnector { connector },
            false,
            true,
        ) {
            self.interaction_state.last_harness_type = Some(harness_type);
        }
    }

    /// One snapped click of the Harness Entry tool: on (or next to) a
    /// connector's body, add an entry at the nearest slot, named from the
    /// next unused definition entry or `ENTRY_n`.
    pub(crate) fn harness_entry_click(&mut self, at: Point) {
        let definitions = self.harness_definitions_in_view();
        let Some((connector_id, entry, index)) = self.document_state.edit_engine().and_then(|e| {
            let c = e
                .document()
                .harness
                .connectors
                .iter()
                .find(|c| c.bounds().expand(ENTRY_REACH_MM).contains(at.x, at.y))?;
            let entry = HarnessEntry {
                uuid: uuid::Uuid::now_v7(),
                name: next_entry_name(c, &definitions),
            };
            Some((c.uuid, entry, c.slot_at(at)))
        }) else {
            crate::diagnostics::log_warning(
                "Harness entry not placed: click on a harness connector",
            );
            return;
        };
        self.apply_engine_command(
            signex_engine::Command::PlaceHarnessEntry {
                connector_id,
                entry,
                index,
            },
            false,
            true,
        );
    }

    /// Replace the connector `connector.uuid` (a Properties edit). Edits of
    /// one connector in a row are one undo step.
    pub(crate) fn update_harness_connector(&mut self, connector: HarnessConnector) -> bool {
        self.apply_engine_command(
            signex_engine::Command::UpdateHarnessConnector {
                uuid: connector.uuid,
                connector,
            },
            true,
            true,
        )
    }

    /// Make the sheet entry `pin` harness-typed (`Some`) or plain again.
    pub(crate) fn set_harness_entry_type(
        &mut self,
        pin: uuid::Uuid,
        harness_type: Option<String>,
    ) -> bool {
        self.apply_engine_command(
            signex_engine::Command::SetHarnessEntryType {
                sheet_pin_id: pin,
                harness_type,
            },
            true,
            true,
        )
    }

    /// Rename the entry `entry` of the connector `connector` to `name`, as
    /// one undo step of its own (the mismatch fix-it).
    pub(crate) fn rename_harness_entry(
        &mut self,
        connector: uuid::Uuid,
        entry: uuid::Uuid,
        name: &str,
    ) -> bool {
        let Some(mut next) = self
            .document_state
            .edit_engine()
            .and_then(|e| e.document().harness.connector(connector).cloned())
        else {
            return false;
        };
        let Some(slot) = next.entries.iter_mut().find(|e| e.uuid == entry) else {
            return false;
        };
        slot.name = name.to_string();
        self.end_directive_edit_run();
        let done = self.update_harness_connector(next);
        self.end_directive_edit_run();
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_click_right_of_the_first_points_the_tip_right() {
        let c = connector_from_clicks(
            Point::new(10.0, 10.0),
            Point::new(20.16, 30.0),
            "SPI_BUS".into(),
            Vec::new(),
        );
        assert_eq!(c.rotation, 0.0);
        assert!((c.width - 10.16).abs() < 1e-9);
        assert_eq!(c.position, Point::new(10.0, 10.0));
    }

    #[test]
    fn a_second_click_left_of_the_first_points_the_tip_left() {
        let c = connector_from_clicks(
            Point::new(30.0, 10.0),
            Point::new(20.0, 10.0),
            "SPI_BUS".into(),
            Vec::new(),
        );
        assert_eq!(c.rotation, 180.0);
        assert_eq!(c.position, Point::new(20.0, 10.0));
    }

    #[test]
    fn a_zero_length_harness_segment_places_nothing() {
        let p = Point::new(1.0, 1.0);
        assert!(harness_wire_commands(&[(p, p)]).is_empty());
        assert_eq!(harness_wire_commands(&[(p, Point::new(2.0, 1.0))]).len(), 1);
    }
}
