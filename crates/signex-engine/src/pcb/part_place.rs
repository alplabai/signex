//! A library part placed on a board from the Components panel (#2364): its
//! top-side footprint on the cursor, turned and flipped, and the one command
//! a click places it with.
//!
//! Pure: nothing here writes; the caller runs the command as one undo step.

use signex_types::designator::next_reference;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::pcb_layer::BoardSide;
use uuid::Uuid;

use super::command::BoardCommand;
use super::eco::stable_uuid;
use super::error::BoardCommandError;
use super::footprint_content::sync_text_properties;
use super::items::{self, BoardItem};

/// A footprint on its way onto a board: the library footprint in its
/// top-side form, how far it is turned and which side it goes on.
#[derive(Debug, Clone, PartialEq)]
pub struct PartToPlace {
    template: Footprint,
    prefix: String,
    value: String,
    /// Quarter turns, counter-clockwise in the model's axes, in `0..4`.
    quarter_turns: i32,
    bottom: bool,
}

impl PartToPlace {
    /// The part `template` names, numbered with `prefix` and carrying
    /// `value`. Refused for a template not on a readable top-side layer or
    /// with a pad that is not at a finite place.
    pub fn new(
        template: Footprint,
        prefix: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, BoardCommandError> {
        let side = parse_pcb_layer(&template.layer).and_then(|layer| layer.side());
        if side != Some(BoardSide::Top) {
            return Err(BoardCommandError::TemplateNotTopSide(template.uuid));
        }
        items::check_pads_finite(&template.pads)?;
        Ok(Self {
            template,
            prefix: prefix.into(),
            value: value.into(),
            quarter_turns: 0,
            bottom: false,
        })
    }

    /// Turn by `quarter_turns` x 90°, counter-clockwise in the model's axes
    /// (on screen that is clockwise, as the board's y axis points down).
    pub fn rotate(&mut self, quarter_turns: i32) {
        self.quarter_turns = (self.quarter_turns + quarter_turns).rem_euclid(4);
    }

    /// Move it to the other side of the board.
    pub fn flip(&mut self) {
        self.bottom = !self.bottom;
    }

    pub fn is_bottom(&self) -> bool {
        self.bottom
    }

    /// The library footprint's name.
    pub fn footprint_name(&self) -> &str {
        &self.template.footprint_id
    }

    /// The designator the next click gives on `board`: one more than the
    /// highest number of the prefix there, as on a sheet.
    pub fn next_reference(&self, board: &PcbBoard) -> String {
        next_reference(
            &self.prefix,
            board.footprints.iter().map(|fp| fp.reference.as_str()),
        )
    }

    /// The footprint as it lands with its origin at `at`, as `uuid`, named
    /// `reference`. Its pads get uuids that follow from `uuid` and carry no
    /// net.
    pub fn footprint_at(&self, uuid: Uuid, at: Point, reference: &str) -> Footprint {
        let mut fp = self.template.clone();
        fp.uuid = uuid;
        fp.position = at;
        fp.rotation = f64::from(self.quarter_turns) * 90.0;
        fp.locked = false;
        fp.schematic_link = None;
        reference.clone_into(&mut fp.reference);
        fp.value.clone_from(&self.value);
        sync_text_properties(&mut fp);
        for (i, pad) in fp.pads.iter_mut().enumerate() {
            pad.uuid = stable_uuid(&[b"placed-pad", uuid.as_bytes(), &(i as u64).to_le_bytes()]);
            pad.net = None;
            pad.net_locked = false;
        }
        if self.bottom {
            pcb_geom::flip_footprint(&mut fp);
        }
        fp
    }

    /// The command that places the part on `board` with its origin at `at`,
    /// as `uuid`, with the next designator.
    pub fn place_command(&self, board: &PcbBoard, uuid: Uuid, at: Point) -> BoardCommand {
        let reference = self.next_reference(board);
        BoardCommand::AddItems {
            items: vec![BoardItem::Footprint(
                self.footprint_at(uuid, at, &reference),
            )],
        }
    }

    /// `board` holding only this part at `at`, as the next click would place
    /// it: what the cursor shows. Keeps the board's layers and setup, so the
    /// part is drawn as the board draws it.
    pub fn preview_board(&self, board: &PcbBoard, at: Point) -> PcbBoard {
        let reference = self.next_reference(board);
        PcbBoard {
            uuid: board.uuid,
            version: board.version,
            generator: String::new(),
            thickness: board.thickness,
            outline: Vec::new(),
            layers: board.layers.clone(),
            setup: board.setup.clone(),
            nets: Vec::new(),
            footprints: vec![self.footprint_at(Uuid::nil(), at, &reference)],
            segments: Vec::new(),
            vias: Vec::new(),
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            sketch: None,
            unknown: Default::default(),
        }
    }
}

#[cfg(test)]
#[path = "part_place_tests.rs"]
mod tests;
