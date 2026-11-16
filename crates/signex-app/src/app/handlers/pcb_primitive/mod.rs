//! The track, via and pad panels' events (#2355). A typed field keeps a
//! draft until Enter; Enter, a dropdown pick or a net picked on the board
//! is one board undo step through `execute_board_commands`. The commands
//! come from `signex_engine::pcb::primitive_edit`; a refusal changes
//! nothing and says why in the Messages panel.

mod facts;
mod links;

use std::collections::BTreeMap;
use std::path::Path;

use pcb_geom::ItemRef;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::primitive_edit::{
    PadEdit, PrimitiveEditRefusal, TrackEdit, ViaEdit, pad_edit_command, track_edit_command,
    via_edit_command,
};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::{MaskExpansion, PadShape, PcbBoard};
use signex_types::pcb_layer::CopperLayer;

use crate::app::Signex;
use crate::app::state::pcb_edit::CheckedStep;
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{
    PrimitiveField, PrimitiveFlag, PrimitiveTarget, parse_number,
};

/// The panel's own state: drafts, search, Show More and the net pick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PcbPrimitivePanel {
    /// The board and items the drafts were typed for.
    pub draft_target: Option<PrimitiveTarget>,
    pub drafts: BTreeMap<PrimitiveField, String>,
    pub show_more: bool,
    pub search: String,
    /// The pick button is waiting for a click on this board for these items.
    pub net_pick: Option<PrimitiveTarget>,
}

impl PcbPrimitivePanel {
    /// Forget drafts and a pick that were not made for `shown` on `board`.
    pub(crate) fn keep_only(&mut self, board: &Path, shown: Option<&PrimitiveTarget>) {
        let stale = |t: &PrimitiveTarget| t.board == board && Some(t) != shown;
        if self.draft_target.as_ref().is_some_and(stale) {
            self.draft_target = None;
            self.drafts.clear();
        }
        if self.net_pick.as_ref().is_some_and(stale) {
            self.net_pick = None;
        }
    }

    /// Forget the drafts typed on `board` (its undo or redo moved them).
    pub(crate) fn drop_drafts_of(&mut self, board: &Path) {
        if self.draft_target.as_ref().is_some_and(|t| t.board == board) {
            self.draft_target = None;
            self.drafts.clear();
        }
    }
}

/// One edit from the panel, before it is matched to the selection's kind.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PrimitiveEdit {
    Number(PrimitiveField, f64),
    Designator(String),
    Layer(CopperLayer),
    Net(u32),
    Shape(PadShape),
    /// A mask expansion field set to the rule or a manual value (#2378).
    Mask(PrimitiveField, MaskExpansion),
    Flag(PrimitiveFlag, bool),
}

impl Signex {
    pub(crate) fn handle_pcb_primitive(&mut self, msg: &PcbPrimitiveMsg) {
        use PcbPrimitiveMsg as M;
        let panel = &mut self.interaction_state.pcb_primitive;
        match msg {
            M::FieldTyped {
                target,
                field,
                text,
            } => {
                if panel.draft_target.as_ref() != Some(target) {
                    panel.draft_target = Some(target.clone());
                    panel.drafts.clear();
                }
                panel.drafts.insert(*field, text.clone());
            }
            M::FieldSubmitted { target, field } => {
                let typed = (panel.draft_target.as_ref() == Some(target))
                    .then(|| panel.drafts.remove(field))
                    .flatten();
                if panel.drafts.is_empty() {
                    panel.draft_target = None;
                }
                if let Some(text) = typed {
                    self.submit_primitive_text(target, *field, &text);
                }
            }
            M::LayerPicked { target, layer } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Layer(*layer),
                    ids::PCB_PRIM_LAYER,
                );
            }
            M::NetPicked { target, net } => {
                self.apply_primitive_edit(target, PrimitiveEdit::Net(*net), ids::PCB_PRIM_NET);
            }
            M::ShapePicked { target, shape } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Shape(*shape),
                    ids::PCB_PRIM_SHAPE,
                );
            }
            M::MaskModePicked {
                target,
                field,
                manual,
            } => {
                let mask = if *manual {
                    MaskExpansion::Manual { mm: 0.0 }
                } else {
                    MaskExpansion::Rule
                };
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Mask(*field, mask),
                    field_name(*field),
                );
            }
            M::FlagToggled { target, flag, on } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Flag(*flag, *on),
                    flag_name(*flag),
                );
            }
            M::NetPickStarted { target } => {
                panel.net_pick = if panel.net_pick.as_ref() == Some(target) {
                    None
                } else {
                    Some(target.clone())
                };
            }
            M::NetLinkPressed { board, net } => self.select_pcb_net(board, *net),
            M::ClassLinkPressed { board, class } => self.select_pcb_net_class(board, class),
            M::ShowMoreToggled => panel.show_more = !panel.show_more,
            M::SearchTyped(text) => panel.search = text.clone(),
        }
        self.sync_pcb_board_summary();
    }

    /// Enter in `field`: read the typed text and apply it.
    fn submit_primitive_text(
        &mut self,
        target: &PrimitiveTarget,
        field: PrimitiveField,
        text: &str,
    ) {
        let what = field_name(field);
        let edit = if field.is_text() {
            PrimitiveEdit::Designator(text.to_string())
        } else {
            match parse_number(text) {
                Some(value) if field.is_mask() => {
                    PrimitiveEdit::Mask(field, MaskExpansion::Manual { mm: value })
                }
                Some(value) => PrimitiveEdit::Number(field, value),
                None => {
                    let line = t!(self.ui_state.i18n, ids::PCB_PRIM_NOT_A_NUMBER, "text" => text);
                    self.refuse_primitive_edit(what, line.into_string());
                    return;
                }
            }
        };
        self.apply_primitive_edit(target, edit, what);
    }

    /// Run `edit` on the target's items as one labelled step, or say why not.
    pub(crate) fn apply_primitive_edit(
        &mut self,
        target: &PrimitiveTarget,
        edit: PrimitiveEdit,
        what: MessageId,
    ) {
        let Some(engine) = self.document_state.board_engine(&target.board) else {
            let name = t!(self.ui_state.i18n, what).into_string();
            let line = t!(self.ui_state.i18n, ids::PCB_PRIM_BOARD_GONE, "what" => name);
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        if engine.board().unknown.newer_minor_source.is_some() {
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string();
            self.refuse_primitive_edit(what, why);
            return;
        }
        let revision = engine.revision();
        let command = match primitive_command(engine.board(), &target.items, &edit) {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(refusal) => {
                self.refuse_primitive_edit(what, refusal.to_string());
                return;
            }
        };
        let label = t!(self.ui_state.i18n, what).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        if let Err(error) = self.execute_board_commands(&target.board, vec![command], Some(step)) {
            self.refuse_primitive_edit(what, error.to_string());
        }
    }

    fn refuse_primitive_edit(&self, what: MessageId, why: String) {
        let name = t!(self.ui_state.i18n, what).into_string();
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_PRIM_REFUSED,
            "what" => name,
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }
}

/// The catalog name of `field`, for undo labels and notices.
fn field_name(field: PrimitiveField) -> MessageId {
    use PrimitiveField as F;
    match field {
        F::X => ids::PCB_PRIM_X,
        F::Y => ids::PCB_PRIM_Y,
        F::Width => ids::PCB_PRIM_WIDTH,
        F::StartX | F::StartY => ids::PCB_PRIM_START,
        F::EndX | F::EndY => ids::PCB_PRIM_END,
        F::Diameter => ids::PCB_PRIM_DIAMETER,
        F::HoleSize => ids::PCB_PRIM_HOLE_SIZE,
        F::Designator => ids::PCB_PRIM_DESIGNATOR,
        F::Rotation => ids::PCB_PRIM_ROTATION,
        F::SizeX => ids::PCB_PRIM_X_SIZE,
        F::SizeY => ids::PCB_PRIM_Y_SIZE,
        F::PasteMask => ids::PCB_PRIM_PASTE_MASK,
        F::SolderMask => ids::PCB_PRIM_SOLDER_MASK,
        F::SolderMaskTop => ids::PCB_PRIM_TOP,
        F::SolderMaskBottom => ids::PCB_PRIM_BOTTOM,
    }
}

/// `edit` with a typed X or Y, read from the board origin with Y up, in the
/// model (#2384).
fn in_board(board: &PcbBoard, edit: &PrimitiveEdit) -> PrimitiveEdit {
    use PrimitiveField as F;
    let origin = board.options.origin;
    match *edit {
        PrimitiveEdit::Number(field @ (F::X | F::StartX | F::EndX), v) => {
            PrimitiveEdit::Number(field, origin.board_x(v))
        }
        PrimitiveEdit::Number(field @ (F::Y | F::StartY | F::EndY), v) => {
            PrimitiveEdit::Number(field, origin.board_y(v))
        }
        _ => edit.clone(),
    }
}

/// The command for `edit` on `items`, all of one kind. `None` when the
/// edit changes nothing or does not apply to the kind.
fn primitive_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edit: &PrimitiveEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let edit = &in_board(board, edit);
    match items.first() {
        Some(ItemRef::Track(_)) => {
            let ids: Vec<_> = items
                .iter()
                .filter_map(|i| match i {
                    ItemRef::Track(id) => Some(*id),
                    _ => None,
                })
                .collect();
            match track_edit(edit) {
                Some(e) => track_edit_command(board, &ids, e),
                None => Ok(None),
            }
        }
        Some(ItemRef::Via(_)) => {
            let ids: Vec<_> = items
                .iter()
                .filter_map(|i| match i {
                    ItemRef::Via(id) => Some(*id),
                    _ => None,
                })
                .collect();
            match via_edit(edit) {
                Some(e) => via_edit_command(board, &ids, e),
                None => Ok(None),
            }
        }
        Some(ItemRef::Pad { .. }) => {
            let pads: Vec<_> = items
                .iter()
                .filter_map(|i| match i {
                    ItemRef::Pad { footprint, pad } => Some((*footprint, *pad)),
                    _ => None,
                })
                .collect();
            match pad_edit(edit) {
                Some(e) => pad_edit_command(board, &pads, &e),
                None => Ok(None),
            }
        }
        Some(ItemRef::Footprint(_) | ItemRef::Zone { .. }) | None => Ok(None),
    }
}

fn track_edit(edit: &PrimitiveEdit) -> Option<TrackEdit> {
    use PrimitiveField as F;
    Some(match *edit {
        PrimitiveEdit::Number(F::X, v) => TrackEdit::LocationX(v),
        PrimitiveEdit::Number(F::Y, v) => TrackEdit::LocationY(v),
        PrimitiveEdit::Number(F::Width, v) => TrackEdit::Width(v),
        PrimitiveEdit::Number(F::StartX, v) => TrackEdit::StartX(v),
        PrimitiveEdit::Number(F::StartY, v) => TrackEdit::StartY(v),
        PrimitiveEdit::Number(F::EndX, v) => TrackEdit::EndX(v),
        PrimitiveEdit::Number(F::EndY, v) => TrackEdit::EndY(v),
        PrimitiveEdit::Layer(layer) => TrackEdit::Layer(layer),
        PrimitiveEdit::Net(net) => TrackEdit::Net(net),
        PrimitiveEdit::Mask(F::PasteMask, m) => TrackEdit::PasteMask(m),
        PrimitiveEdit::Mask(F::SolderMask, m) => TrackEdit::SolderMask(m),
        PrimitiveEdit::Flag(PrimitiveFlag::Locked, on) => TrackEdit::Locked(on),
        _ => return None,
    })
}

fn via_edit(edit: &PrimitiveEdit) -> Option<ViaEdit> {
    use PrimitiveField as F;
    Some(match *edit {
        PrimitiveEdit::Number(F::X, v) => ViaEdit::X(v),
        PrimitiveEdit::Number(F::Y, v) => ViaEdit::Y(v),
        PrimitiveEdit::Number(F::Diameter, v) => ViaEdit::Diameter(v),
        PrimitiveEdit::Number(F::HoleSize, v) => ViaEdit::HoleSize(v),
        PrimitiveEdit::Net(net) => ViaEdit::Net(net),
        PrimitiveEdit::Mask(F::SolderMaskTop, m) => ViaEdit::SolderMaskTop(m),
        PrimitiveEdit::Mask(F::SolderMaskBottom, m) => ViaEdit::SolderMaskBottom(m),
        PrimitiveEdit::Flag(PrimitiveFlag::Locked, on) => ViaEdit::Locked(on),
        PrimitiveEdit::Flag(PrimitiveFlag::TentedTop, on) => ViaEdit::TentedTop(on),
        PrimitiveEdit::Flag(PrimitiveFlag::TentedBottom, on) => ViaEdit::TentedBottom(on),
        PrimitiveEdit::Flag(PrimitiveFlag::FromHoleEdge, on) => ViaEdit::FromHoleEdge(on),
        _ => return None,
    })
}

fn pad_edit(edit: &PrimitiveEdit) -> Option<PadEdit> {
    use PrimitiveField as F;
    Some(match edit {
        PrimitiveEdit::Number(F::X, v) => PadEdit::X(*v),
        PrimitiveEdit::Number(F::Y, v) => PadEdit::Y(*v),
        PrimitiveEdit::Number(F::Rotation, v) => PadEdit::Rotation(*v),
        PrimitiveEdit::Number(F::SizeX, v) => PadEdit::SizeX(*v),
        PrimitiveEdit::Number(F::SizeY, v) => PadEdit::SizeY(*v),
        PrimitiveEdit::Number(F::HoleSize, v) => PadEdit::HoleSize(*v),
        PrimitiveEdit::Designator(text) => PadEdit::Designator(text.clone()),
        PrimitiveEdit::Shape(shape) => PadEdit::Shape(*shape),
        PrimitiveEdit::Net(net) => PadEdit::Net(*net),
        _ => return None,
    })
}

/// The catalog name of `flag`, for undo labels and notices.
fn flag_name(flag: PrimitiveFlag) -> MessageId {
    match flag {
        PrimitiveFlag::Locked => ids::PCB_PRIM_LOCK,
        PrimitiveFlag::TentedTop | PrimitiveFlag::TentedBottom => ids::PCB_PRIM_TENTED,
        PrimitiveFlag::FromHoleEdge => ids::PCB_PRIM_FROM_HOLE_EDGE,
    }
}
