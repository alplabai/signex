//! The Via panel's own events (mockup row 96): picking a via type, a reset
//! arrow and the shown layer. A pick or a reset is one board undo step built
//! from the board's derived via types (`signex_engine::pcb::via_types`); the
//! shown layer is panel state only.

use pcb_geom::ItemRef;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::primitive_edit::{PrimitiveEditRefusal, ViaEdit, via_edit_command};
use signex_engine::pcb::via_types::{ViaTypeDef, apply_type_commands, type_of, via_types};
use signex_i18n::{MessageId, ids};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::Signex;
use crate::panels::pcb_primitive_properties::{PrimitiveField, PrimitiveTarget, ViaMsg};

impl Signex {
    pub(super) fn handle_via_msg(&mut self, msg: &ViaMsg) {
        let panel = &mut self.interaction_state.pcb_primitive;
        match msg {
            ViaMsg::TypePicked { target, key } => {
                let key = *key;
                self.run_via_type_step(target, ids::PCB_PRIM_VIA_TYPE, |board, items| {
                    let types = via_types(board);
                    let Some(t) = types.iter().find(|t| t.key() == key) else {
                        return Ok(Vec::new());
                    };
                    apply_type_commands(board, items, t)
                });
            }
            ViaMsg::SizeReset { target, field } => {
                let field = *field;
                let what = if field == PrimitiveField::Diameter {
                    ids::PCB_PRIM_DIAMETER
                } else {
                    ids::PCB_PRIM_DRILL
                };
                self.run_via_type_step(target, what, |board, items| {
                    per_type(board, items, |board, items, t| {
                        let edit = if field == PrimitiveField::Diameter {
                            ViaEdit::Diameter(t.diameter)
                        } else {
                            ViaEdit::HoleSize(t.drill)
                        };
                        Ok(via_edit_command(board, &via_ids(items), edit)?
                            .into_iter()
                            .collect())
                    })
                });
            }
            ViaMsg::ViewLayerPicked(layer) => panel.via_view_layer = Some(*layer),
        }
    }

    /// Run the commands `build` makes for the target's vias as one labelled
    /// step, or say why not. Nothing to change is no step.
    fn run_via_type_step(
        &mut self,
        target: &PrimitiveTarget,
        what: MessageId,
        build: impl FnOnce(&PcbBoard, &[ItemRef]) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal>,
    ) {
        if target.is_placement() {
            return;
        }
        self.run_guarded_board_step(&target.board, what, |board| build(board, &target.items));
    }
}

/// `build` run once per via type among the vias of `items`, each on the
/// vias of that type; a via of no type is left as it is.
fn per_type(
    board: &PcbBoard,
    items: &[ItemRef],
    build: impl Fn(
        &PcbBoard,
        &[ItemRef],
        &ViaTypeDef,
    ) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal>,
) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal> {
    let types = via_types(board);
    let mut groups: Vec<(usize, Vec<ItemRef>)> = Vec::new();
    for id in via_ids(items) {
        let via = board
            .vias
            .iter()
            .find(|v| v.uuid == id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
        let Some(index) = type_of(&types, via) else {
            continue;
        };
        match groups.iter_mut().find(|(i, _)| *i == index) {
            Some((_, group)) => group.push(ItemRef::Via(id)),
            None => groups.push((index, vec![ItemRef::Via(id)])),
        }
    }
    let mut commands = Vec::new();
    for (index, group) in groups {
        commands.extend(build(board, &group, &types[index])?);
    }
    Ok(commands)
}

fn via_ids(items: &[ItemRef]) -> Vec<Uuid> {
    items
        .iter()
        .filter_map(|item| match item {
            ItemRef::Via(id) => Some(*id),
            _ => None,
        })
        .collect()
}
