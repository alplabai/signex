//! The track, via, pad, area and drawing panels' events (#2355, #2393,
//! #2424). A typed field keeps a
//! draft until Enter; Enter, a dropdown pick or a net picked on the board
//! is one board undo step through `execute_board_commands`. The commands
//! come from `signex_engine::pcb::primitive_edit`; a refusal changes
//! nothing and says why in the Messages panel. An edit of the object a
//! placement tool is about to place sets the tool's value instead (#2393,
//! `pcb_tool::form`).

mod facts;
mod free;
mod graphic;
mod links;
mod placement;
mod stack_facts;
mod tune;
mod via_type;
mod zone;

#[cfg(test)]
mod tune_tests;

pub(crate) use graphic::graphic_edit;
pub(crate) use zone::zone_edit;

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
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::StackMode;
use signex_widgets::property_illustration::IllustrationHover;

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
    /// The field or illustration callout under the pointer (#2523).
    pub hover: IllustrationHover,
    /// The "Not stored yet" group is open.
    pub unstored_open: bool,
    /// The vias Override was switched on for (mockup row 96): their
    /// Diameter and Drill take typed values.
    pub via_override: Option<PrimitiveTarget>,
    /// The copper layer the via illustration's top view shows.
    pub via_view_layer: Option<CopperLayer>,
    /// The pour Copper card's menu of board-wide commands is open (#2523).
    pub pour_menu_open: bool,
    /// The target the panel showed at the last sync: `hover`,
    /// `via_view_layer` and `pour_menu_open` belong to it alone.
    pub shown: Option<PrimitiveTarget>,
}

impl PcbPrimitivePanel {
    /// Forget drafts, a pick and Override that were not made for `shown` on
    /// `board`; a new `shown` also drops the hover, the via view layer and
    /// the pour menu.
    pub(crate) fn keep_only(&mut self, board: &Path, shown: Option<&PrimitiveTarget>) {
        let stale = |t: &PrimitiveTarget| t.board == board && Some(t) != shown;
        if self.draft_target.as_ref().is_some_and(stale) {
            self.draft_target = None;
            self.drafts.clear();
        }
        if self.net_pick.as_ref().is_some_and(stale) {
            self.net_pick = None;
        }
        if self.via_override.as_ref().is_some_and(stale) {
            self.via_override = None;
        }
        if self.shown.as_ref() != shown {
            self.shown = shown.cloned();
            self.hover = IllustrationHover::default();
            self.via_view_layer = None;
            self.pour_menu_open = false;
        }
    }

    /// Whether the per-target state (hover, via view layer, pour menu) is
    /// `target`'s: facts built before `keep_only` saw a new target must not
    /// show the old one's.
    pub(crate) fn is_showing(&self, target: &PrimitiveTarget) -> bool {
        self.shown.as_ref() == Some(target)
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
    /// A string's text (#2373), or a text frame's as the board holds it (#2424).
    Text(String),
    Layer(CopperLayer),
    /// An area's layer: copper, or a keep-out layer for a keepout.
    AreaLayer(PcbLayer),
    Net(u32),
    Shape(PadShape),
    /// A via's two end layers (#2407).
    Span(CopperLayer, CopperLayer),
    /// How vias or pads size their copper by layer (#2401).
    Mode(StackMode),
    /// A mask expansion field set to the rule or a manual value (#2378).
    Mask(PrimitiveField, MaskExpansion),
    Flag(PrimitiveFlag, bool),
    /// A pour's Remove Dead Copper (#2216).
    DeadIslands(bool),
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
                // Enter in the paused form goes on placing (#2393).
                if target.is_placement() && self.is_board_tool_paused() {
                    self.resume_board_tool();
                }
            }
            M::LayerPicked { target, layer } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Layer(*layer),
                    ids::PCB_PRIM_LAYER,
                );
            }
            M::AreaLayerPicked { target, layer } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::AreaLayer(*layer),
                    ids::PCB_PRIM_LAYER,
                );
            }
            M::NetPicked { target, net } => {
                self.apply_primitive_edit(target, PrimitiveEdit::Net(*net), ids::PCB_PRIM_NET);
            }
            M::DeadIslandsToggled { target, on } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::DeadIslands(*on),
                    ids::PCB_PRIM_REMOVE_DEAD_ISLANDS,
                );
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
                // Measure would take the pick's click (#2403).
                if panel.net_pick.is_some() {
                    self.interaction_state.pcb_query.measure = None;
                }
            }
            M::NetLinkPressed { board, net } => self.select_pcb_net(board, *net),
            M::ClassLinkPressed { board, class } => self.select_pcb_net_class(board, class),
            M::ShowMoreToggled => panel.show_more = !panel.show_more,
            M::SearchTyped(text) => panel.search = text.clone(),
            M::LockToggled { target, locked } => {
                let items = super::pcb_item_edits::lock_targets(&target.items);
                if let Err(why) = self.set_items_locked(&target.board, &items, *locked) {
                    self.refuse_primitive_edit(ids::PCB_PRIM_LOCK, why);
                }
            }
            M::SpanPicked {
                target,
                top,
                bottom,
            } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Span(*top, *bottom),
                    ids::PCB_PRIM_VIA_NAME,
                );
            }
            M::StackModePicked { target, mode } => {
                let what = match target.items.first() {
                    Some(ItemRef::Pad { .. }) => ids::PCB_PRIM_PAD_STACK,
                    _ => ids::PCB_PRIM_VIA_STACK,
                };
                self.apply_primitive_edit(target, PrimitiveEdit::Mode(*mode), what);
            }
            // Only the hover changes: patch the shown facts instead of
            // rebuilding the board summary on every pointer crossing.
            M::IllustrationHovered(hover) => {
                panel.hover.update(hover.clone());
                let hover = panel.hover.clone();
                let shown = self.document_state.panel_ctx.pcb_board.as_mut();
                if let Some(facts) = shown.and_then(|b| b.primitive.as_mut()) {
                    facts.common_mut().hover = hover;
                }
                return;
            }
            M::UnstoredToggled => panel.unstored_open = !panel.unstored_open,
            M::ValuePicked {
                target,
                field,
                value,
            } => {
                self.apply_primitive_edit(
                    target,
                    PrimitiveEdit::Number(*field, *value),
                    field_name(*field),
                );
            }
            M::ArcModePicked(mode) => self.pick_arc_mode(*mode),
            M::Via(m) => self.handle_via_msg(m),
            M::Pour(m) => self.handle_pour_msg(m),
        }
        self.sync_pcb_board_summary();
    }

    /// Enter in `field`: read the typed text and apply it.
    pub(crate) fn submit_primitive_text(
        &mut self,
        target: &PrimitiveTarget,
        field: PrimitiveField,
        text: &str,
    ) {
        let what = field_name(field);
        let edit = if field == PrimitiveField::Text {
            // A text frame reads a typed `\n` as a line break (#2424); a string
            // keeps the text as typed (#2373).
            let frame = match target.items.first() {
                Some(item) => matches!(item, ItemRef::Graphic { .. }),
                None => self
                    .interaction_state
                    .pcb_tool
                    .as_ref()
                    .is_some_and(|s| s.tool.graphic_kind().is_some()),
            };
            if frame {
                PrimitiveEdit::Text(signex_types::pcb_graphic::text_from_field(text))
            } else {
                PrimitiveEdit::Text(text.to_string())
            }
        } else if field.is_text() {
            PrimitiveEdit::Designator(text.to_string())
        } else {
            match parse_number(text) {
                Some(value) if field == PrimitiveField::TuneLength => {
                    self.tune_track_length(target, value);
                    return;
                }
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
        if target.is_placement() {
            self.edit_board_tool_value(&edit, what);
            return;
        }
        self.run_guarded_board_step(&target.board, what, |board| {
            Ok(primitive_command(board, &target.items, &edit)?
                .into_iter()
                .collect())
        });
    }

    /// Run the commands `build` makes from the board at `path` as one undo
    /// step labelled `what`, or say why not: the board is gone (logged), it
    /// is read-only, `build` refuses, or the board moved on meanwhile. No
    /// command is no step.
    pub(crate) fn run_guarded_board_step(
        &mut self,
        path: &Path,
        what: MessageId,
        build: impl FnOnce(&PcbBoard) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal>,
    ) {
        let Some(engine) = self.document_state.board_engine(path) else {
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
        let commands = match build(engine.board()) {
            Ok(commands) if commands.is_empty() => return,
            Ok(commands) => commands,
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
        if let Err(error) = self.execute_board_commands(path, commands, Some(step)) {
            self.refuse_primitive_edit(what, error.to_string());
        }
    }

    pub(crate) fn refuse_primitive_edit(&self, what: MessageId, why: String) {
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
        F::TuneLength => ids::PCB_PRIM_TUNE_LENGTH,
        F::Clearance => ids::PCB_PRIM_CLEARANCE,
        F::MinThickness => ids::PCB_PRIM_MIN_THICKNESS,
        F::LayerSize(_) | F::LayerSizeY(_) => ids::PCB_PRIM_COPPER,
        F::PasteMask => ids::PCB_PRIM_PASTE_MASK,
        F::SolderMask => ids::PCB_PRIM_SOLDER_MASK,
        F::SolderMaskTop => ids::PCB_PRIM_TOP,
        F::SolderMaskBottom => ids::PCB_PRIM_BOTTOM,
        F::Text => ids::PCB_PRIM_TEXT,
        F::TextHeight => ids::PCB_PRIM_TEXT_HEIGHT,
        F::Radius => ids::PCB_PRIM_RADIUS,
        F::StartAngle => ids::PCB_PRIM_START_ANGLE,
        F::EndAngle => ids::PCB_PRIM_END_ANGLE,
        F::Offset => ids::PCB_PRIM_OFFSET,
        F::LineWidth => ids::PCB_PRIM_LINE_WIDTH,
        F::Precision => ids::PCB_PRIM_PRECISION,
        F::Priority => ids::PCB_PRIM_PRIORITY,
        F::MinIslandArea => ids::PCB_PRIM_MIN_ISLAND_AREA,
        F::CenterX | F::CenterY => ids::PCB_PRIM_GRAPHIC_CENTER,
    }
}

/// `edit` with a typed X or Y, read from the board origin with Y up, in the
/// model (#2384).
fn in_board(board: &PcbBoard, edit: &PrimitiveEdit) -> PrimitiveEdit {
    use PrimitiveField as F;
    let origin = board.options.origin;
    match *edit {
        PrimitiveEdit::Number(field @ (F::X | F::StartX | F::EndX | F::CenterX), v) => {
            PrimitiveEdit::Number(field, origin.board_x(v))
        }
        PrimitiveEdit::Number(field @ (F::Y | F::StartY | F::EndY | F::CenterY), v) => {
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
        Some(ItemRef::Zone { .. }) => zone::zone_command(board, items, edit),
        Some(ItemRef::Arc(_) | ItemRef::Text(_) | ItemRef::FreePad(_) | ItemRef::Dimension(_)) => {
            free::free_command(board, items, edit)
        }
        Some(ItemRef::Graphic { .. }) => graphic::graphic_command(board, items, edit),
        // A room edits through Room Properties (#2457).
        Some(ItemRef::Footprint(_) | ItemRef::FootprintText { .. } | ItemRef::Room(_)) | None => {
            Ok(None)
        }
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
        PrimitiveEdit::Number(F::LayerSize(layer), v) => ViaEdit::LayerDiameter(layer, v),
        PrimitiveEdit::Net(net) => ViaEdit::Net(net),
        PrimitiveEdit::Span(top, bottom) => ViaEdit::Span(top, bottom),
        PrimitiveEdit::Mode(mode) => ViaEdit::Mode(mode),
        PrimitiveEdit::Mask(F::SolderMaskTop, m) => ViaEdit::SolderMaskTop(m),
        PrimitiveEdit::Mask(F::SolderMaskBottom, m) => ViaEdit::SolderMaskBottom(m),
        PrimitiveEdit::Flag(PrimitiveFlag::Locked, on) => ViaEdit::Locked(on),
        PrimitiveEdit::Flag(PrimitiveFlag::TentedTop, on) => ViaEdit::TentedTop(on),
        PrimitiveEdit::Flag(PrimitiveFlag::TentedBottom, on) => ViaEdit::TentedBottom(on),
        PrimitiveEdit::Flag(PrimitiveFlag::FromHoleEdge, on) => ViaEdit::FromHoleEdge(on),
        _ => return None,
    })
}

pub(super) fn pad_edit(edit: &PrimitiveEdit) -> Option<PadEdit> {
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
        PrimitiveEdit::Number(F::LayerSize(layer), v) => PadEdit::LayerSizeX(*layer, *v),
        PrimitiveEdit::Number(F::LayerSizeY(layer), v) => PadEdit::LayerSizeY(*layer, *v),
        PrimitiveEdit::Mode(mode) => PadEdit::Mode(*mode),
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
