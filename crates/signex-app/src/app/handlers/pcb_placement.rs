//! Placement edits on the board in view (#2240): rotate, flip, delete and
//! move the selected footprints, by key, menu, Active Bar action, palette,
//! drag or Edit ▸ Move. Each edit is one `execute_board_commands` step; the
//! commands are built by `signex_engine::pcb::placement`, which also refuses
//! a locked footprint. A refusal changes nothing and says why in the
//! Messages panel. A selected pad stands for its footprint.

use std::collections::BTreeSet;

use iced::Task;
use pcb_geom::{ItemRef, PickKind};
use signex_engine::pcb::placement::{self, PlacementRefusal, PlacementTransform};
use signex_engine::pcb::{BoardCommand, BoardItemId};
use signex_renderer::pcb::{BOARD_OUTLINE, BoardObject, board_layers};
use signex_types::pcb::Point;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_options::GridUse;
use uuid::Uuid;

use crate::app::command::{CommandOutcome, OperandValue, translator};
use crate::app::{Message, Signex};
use crate::pcb_canvas::{CarryKind, PcbCarry, SnapView, carry_delta};

/// The View Options rows that hide something object snapping targets.
const SNAP_OBJECT_KINDS: [(BoardObject, PickKind); 9] = [
    (BoardObject::Pads, PickKind::Pads),
    (BoardObject::Vias, PickKind::Vias),
    (BoardObject::Arcs, PickKind::Arcs),
    (BoardObject::Tracks, PickKind::Tracks),
    (BoardObject::Polygons, PickKind::Polygons),
    (BoardObject::Regions, PickKind::Regions),
    (BoardObject::Fills, PickKind::Fills),
    (BoardObject::Keepouts, PickKind::Keepouts),
    (BoardObject::Texts, PickKind::Texts),
];

impl Signex {
    /// Rotate or mirror the board selection when the active tab is a PCB.
    /// Whether it was one (the schematic path is then skipped).
    pub(crate) fn transform_pcb_selection(&mut self, transform: PlacementTransform) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let footprints = self.pcb_selected_footprints();
        let Some(board) = self.active_pcb() else {
            return false;
        };
        let on_grid = self.ui_state.snap_enabled;
        let built = placement::transform_commands_with(board, &footprints, transform, on_grid);
        let what = match transform {
            PlacementTransform::Rotate { .. } => "Rotate",
            PlacementTransform::FlipLeftRight | PlacementTransform::FlipTopBottom => "Flip",
        };
        self.run_placement(what, built);
        true
    }

    /// Delete the board selection when the active tab is a PCB: its
    /// footprints (a selected pad's included), tracks, vias and drawn areas,
    /// through the command's operand handler (`command::translator`). An
    /// empty selection runs too, so its refusal is said as before. `None`
    /// when the tab is no PCB.
    pub(crate) fn delete_pcb_selection(&mut self) -> Option<Task<Message>> {
        if !self.has_active_pcb() {
            return None;
        }
        let items = self
            .with_pcb_selection(|state, _| deletable_items(state.selection()))
            .unwrap_or_default();
        self.active_pcb()?;
        let Some(path) = self.active_tab_path() else {
            return Some(Task::none());
        };
        let ids = items.into_iter().map(BoardItemId::uuid).collect();
        let operands = vec![("ids", OperandValue::Targets(ids))];
        let (outcome, task) = translator::run(self, "delete_selection", &path, operands);
        match outcome {
            CommandOutcome::Answered(answer) if answer["changed"].as_bool() == Some(true) => {
                self.interaction_state.pcb_selection.clear_selection();
                self.refresh_pcb_highlight();
            }
            CommandOutcome::Disabled { reason } => {
                refuse("Delete", &reason.render(&self.ui_state.i18n));
            }
            _ => {}
        }
        Some(task)
    }

    /// Edit ▸ Move on a board: the selected footprints follow the pointer
    /// from the first click to the second. With nothing selected and
    /// `pick_first`, the first click picks the footprint under it.
    pub(crate) fn arm_pcb_move(&mut self, pick_first: bool) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let footprints = self.pcb_selected_footprints();
        let (Some(board), Some(path)) = (self.active_pcb(), self.active_tab_path()) else {
            return false;
        };
        if footprints.is_empty() && !pick_first {
            crate::diagnostics::log_warning("Move: select a footprint to move first");
            return true;
        }
        if !footprints.is_empty()
            && let Err(refusal) = placement::editable_footprints(board, &footprints)
        {
            refuse("Move", &refusal);
            return true;
        }
        crate::diagnostics::log_info(if footprints.is_empty() {
            "Move: click a footprint, then click where it should go (Esc to cancel)"
        } else {
            "Move: click a reference point, then click where it should go (Esc to cancel)"
        });
        let window = self.interaction_state.acting_window;
        let mut carry = PcbCarry::new(CarryKind::Move, path, window, footprints);
        carry.stretch_corner = pcb_geom::stretch_rule(self.interaction_state.corner_rule);
        self.release_board_tool();
        // Measure would take the Move's clicks (#2403).
        self.interaction_state.pcb_query.measure = None;
        self.interaction_state.pcb_carry = Some(carry);
        // While a Move waits for its clicks, a press never starts a drag.
        self.refresh_pcb_highlight();
        true
    }

    /// A click while Edit ▸ Move is armed: pick and anchor, or drop.
    /// Whether the click belonged to the move.
    pub(crate) fn pcb_move_clicked(&mut self, at: Point, tolerance_mm: f64) -> bool {
        let Some(carry) = self.live_carry(CarryKind::Move) else {
            return false;
        };
        let Some(anchor) = carry.anchor else {
            let mut footprints = carry.footprints;
            if footprints.is_empty() {
                // The click picks, and selects, the part it lands on; a
                // designator or comment stands for nothing here (#2368).
                self.with_pcb_selection(|state, board| {
                    state.click_part(board, at, tolerance_mm, pcb_geom::ClickMode::Replace)
                });
                footprints = self.pcb_selected_footprints();
                if footprints.is_empty() {
                    // Nothing under the pointer: keep waiting for a footprint.
                    self.refresh_pcb_highlight();
                    return true;
                }
            }
            let refused = self
                .active_pcb()
                .map(|board| placement::editable_footprints(board, &footprints).err());
            if let Some(Some(refusal)) = refused {
                refuse("Move", &refusal);
                self.end_pcb_carry();
                return true;
            }
            if let Some(carry) = self.interaction_state.pcb_carry.as_mut() {
                carry.footprints = footprints;
                carry.anchor = Some(at);
            }
            self.refresh_pcb_highlight();
            return true;
        };
        self.drop_pcb_carry(anchor, at);
        true
    }

    /// The pointer moved while a Move is anchored: follow it.
    pub(crate) fn pcb_move_followed(&mut self, at: Point) {
        if let Some(anchor) = self.live_carry(CarryKind::Move).and_then(|c| c.anchor) {
            self.carry_to(anchor, at);
        }
    }

    /// A drag pressed at `from` reached `to`: preview it. A press on the
    /// selection carries it; a press on an unselected footprint selects and
    /// carries that one (#2358). Once `leaves_tracks` (Ctrl down) arrives,
    /// the tracks on them stay where they are until the carry ends.
    pub(crate) fn pcb_footprints_dragged(
        &mut self,
        from: Point,
        to: Point,
        tolerance_mm: f64,
        leaves_tracks: bool,
    ) {
        self.follow_pcb_drag(from, to, Some(tolerance_mm));
        let latched = leaves_tracks
            && self
                .live_carry(CarryKind::Drag)
                .is_some_and(|carry| !carry.leaves_tracks);
        if let Some(carry) = self
            .interaction_state
            .pcb_carry
            .as_mut()
            .filter(|_| latched)
        {
            carry.leaves_tracks = true;
            // A via's snap counts its tracks only while they stay.
            self.carry_to(from, to);
            self.refresh_pcb_highlight();
        }
    }

    /// Start the drag pressed at `from`, unless it is under way, and follow
    /// it to `to`. With `pick` (the press's reach) the press picks what it
    /// carries; without, as for a drop whose drag never arrived, it carries
    /// the selection only, so a release left over from an ended drag never
    /// picks anything up.
    fn follow_pcb_drag(&mut self, from: Point, to: Point, pick: Option<f64>) {
        // A drag is one press: a carry from another press (a release that
        // never arrived) is replaced, never reused for this selection.
        let same_press = self
            .live_carry(CarryKind::Drag)
            .is_some_and(|carry| carry.anchor == Some(from));
        if !same_press {
            let Some(path) = self.active_tab_path() else {
                return;
            };
            if let Some(reach) = pick
                && self.start_text_drag(&path, from, reach)
            {
                self.carry_to(from, to);
                return;
            }
            // A via or any other object a click there picks (#2870); a press
            // on a member of the selection carries all of it.
            if let Some(reach) = pick
                && (self.start_via_drag(&path, from, reach)
                    || self.start_item_drag(&path, from, reach))
            {
                self.widen_drag_to_selection();
                self.carry_to(from, to);
                return;
            }
            let mut footprints = match pick {
                Some(reach) => self
                    .with_pcb_selection(|state, board| state.grab_for_drag(board, from, reach))
                    .unwrap_or_default(),
                None => self.pcb_selected_footprints(),
            };
            // A locked footprint refuses when the drag starts. The press
            // then carries nothing, so its later moves neither repeat the
            // notice nor paint a ghost, and its release moves nothing.
            let refused = self
                .active_pcb()
                .and_then(|board| placement::editable_footprints(board, &footprints).err());
            if let Some(refusal) = refused {
                refuse("Move", &refusal);
                footprints.clear();
            }
            let window = self.interaction_state.acting_window;
            let mut carry = PcbCarry::new(CarryKind::Drag, path, window, footprints);
            carry.anchor = Some(from);
            carry.stretch_corner = pcb_geom::stretch_rule(self.interaction_state.corner_rule);
            self.interaction_state.pcb_carry = Some(carry);
            if pick.is_some() {
                self.widen_drag_to_selection();
            }
            self.refresh_pcb_highlight();
        }
        self.carry_to(from, to);
    }

    /// What the status bar says while this window drags footprints, or vias
    /// with tracks on them: whether the tracks follow, and the key that
    /// leaves them.
    pub(crate) fn board_drag_status(&self) -> Option<String> {
        let carry = self.live_carry(CarryKind::Drag)?;
        if (carry.footprints.is_empty() && !carry.via_tracks) || carry.anchor.is_none() {
            return None;
        }
        let id = if carry.leaves_tracks {
            signex_i18n::ids::BOARD_STATUS_DRAG_WITHOUT_TRACKS
        } else {
            signex_i18n::ids::BOARD_STATUS_DRAG_WITH_TRACKS
        };
        let key = crate::pcb_canvas::LEAVE_TRACKS_KEY;
        Some(self.ui_state.i18n.message_plain(id, &[("key", key.into())]))
    }

    /// The drag was released at `to`: move as one step.
    pub(crate) fn pcb_footprints_dropped(&mut self, from: Point, to: Point) {
        self.follow_pcb_drag(from, to, None);
        self.drop_pcb_carry(from, to);
    }

    /// Drop this window's drag or Move without moving anything (Esc, the
    /// pointer gone, a right-click). Another window's carry is left alone.
    pub(crate) fn end_pcb_carry(&mut self) {
        self.interaction_state.pcb_canvas.cancel_marquee();
        if self.has_own_pcb_carry() {
            self.interaction_state.pcb_carry = None;
            self.refresh_pcb_highlight();
        }
    }

    /// Whether the carry belongs to the board this window shows.
    pub(crate) fn has_own_pcb_carry(&self) -> bool {
        let path = self.active_tab_path();
        let window = self.interaction_state.acting_window;
        self.interaction_state
            .pcb_carry
            .as_ref()
            .is_some_and(|carry| carry.is_for(path.as_deref(), window))
    }

    /// Forget a carry left on a board this window no longer shows (a tab
    /// switch mid-Move). Whether one was dropped.
    pub(crate) fn drop_stale_pcb_carry(&mut self) -> bool {
        let stale = self
            .interaction_state
            .pcb_carry
            .as_ref()
            .is_some_and(|carry| {
                carry.window == self.interaction_state.acting_window && !self.has_own_pcb_carry()
            });
        if stale {
            self.interaction_state.pcb_carry = None;
        }
        stale
    }

    /// `window` lost focus: its release may never arrive, so a drag
    /// started there ends with nothing moved. An armed Move waits for its
    /// clicks across the focus change.
    pub(crate) fn end_pcb_drag_on_focus_loss(&mut self, window: iced::window::Id) -> Task<Message> {
        let owner = self.carry_window(window);
        let is_drag_of = |app: &Self| {
            app.interaction_state
                .pcb_carry
                .as_ref()
                .is_some_and(|carry| carry.kind == CarryKind::Drag && carry.window == owner)
        };
        if !is_drag_of(self) {
            return Task::none();
        }
        let task = match owner {
            None => {
                self.end_pcb_carry();
                Task::none()
            }
            Some(id) => self.run_in_window(id, false, |app| {
                app.end_pcb_carry();
                Task::none()
            }),
        };
        // A drag left on a board the window no longer shows goes too.
        if is_drag_of(self) {
            self.interaction_state.pcb_carry = None;
        }
        task
    }

    /// The undocked `window` closed: a drag or Move started there goes
    /// with it.
    pub(crate) fn forget_pcb_carry_of_window(&mut self, window: iced::window::Id) {
        let owned = self
            .interaction_state
            .pcb_carry
            .as_ref()
            .is_some_and(|carry| carry.window == Some(window));
        if owned {
            self.interaction_state.pcb_carry = None;
        }
    }

    /// How a carry names `window`: `None` for the main one.
    fn carry_window(&self, window: iced::window::Id) -> Option<iced::window::Id> {
        (self.ui_state.main_window_id != Some(window)).then_some(window)
    }

    /// The carry of `kind`, if it belongs to the board in view in this
    /// window.
    pub(crate) fn live_carry(&self, kind: CarryKind) -> Option<PcbCarry> {
        let carry = self.interaction_state.pcb_carry.as_ref()?;
        (carry.kind == kind && self.has_own_pcb_carry()).then(|| carry.clone())
    }

    /// `at` on the grid for `what`, or as it is with the status bar's Snap
    /// off: for a point that takes the grid alone (an origin, a guide).
    pub(crate) fn board_grid_point(
        &self,
        board: &signex_types::pcb::PcbBoard,
        what: GridUse,
        at: Point,
    ) -> Point {
        if self.ui_state.snap_enabled {
            placement::grid_for(board, what).snap(at)
        } else {
            at
        }
    }

    /// Which grid applies on the board at `path` in the window `owner`
    /// (`None`: the main one): what its carry or armed tool snaps to, by the
    /// same use `grid_for` is asked with. With nothing armed, Components:
    /// the Select tool's drag of a part is what the grid then acts on.
    pub(crate) fn pcb_grid_use(
        &self,
        path: &std::path::Path,
        owner: Option<iced::window::Id>,
    ) -> GridUse {
        let state = &self.interaction_state;
        if let Some(carry) = state
            .pcb_carry
            .as_ref()
            .filter(|carry| carry.is_for(Some(path), owner))
        {
            return match &carry.paste {
                Some(load) => load.copy.grid_use(),
                None if !carry.vias.is_empty() => GridUse::NonComponents,
                None => GridUse::Components,
            };
        }
        let tool = state
            .pcb_tool
            .as_ref()
            .is_some_and(|session| session.is_for(Some(path), owner));
        let measure = state
            .pcb_query
            .measure
            .as_ref()
            .is_some_and(|measure| measure.is_for(Some(path), owner));
        let guide = state
            .pcb_guide_place
            .as_ref()
            .is_some_and(|(armed, _)| armed == path);
        let origin = state.pcb_origin_place.as_deref() == Some(path);
        let arrange = state
            .pcb_arrange_box
            .as_ref()
            .is_some_and(|armed| armed.path == path);
        if tool || measure || guide || origin || arrange {
            GridUse::NonComponents
        } else {
            GridUse::Components
        }
    }

    /// What a snap reads for the canvas in view: the panel's options, the
    /// status bar's Snap, the zoom and the layers shown.
    pub(crate) fn pcb_snap_view(&self) -> SnapView<'_> {
        let state = &self.interaction_state;
        let tile = || {
            let path = self.active_tab_path()?;
            state.tile_panes.board.get(&path)
        };
        let canvas = match state.acting_window {
            Some(window) => state.pcb_canvas_for_window(window),
            None => tile().unwrap_or(&state.pcb_canvas),
        };
        let active = self
            .active_tab_path()
            .map(|path| self.pcb_layer_view(&path).active())
            .unwrap_or(PcbLayer::Copper(CopperLayer::Top));
        let mut view = SnapView::new(
            &self.ui_state.pcb_snap,
            active,
            f64::from(canvas.camera_ref().scale()),
        );
        view.snap_on = self.ui_state.snap_enabled;
        if let Some((path, board)) = self.active_tab_path().zip(self.active_pcb()) {
            let shown = self.pcb_layer_view(&path);
            view.through_hidden = !shown.multi_layer_visible();
            // Start from all shown, so a layer the stack does not list
            // stays shown, and clear the hidden ones.
            let mut any_copper_shown = false;
            for layer in board_layers(&board.layers) {
                match layer {
                    PcbLayer::Copper(_) if shown.is_visible(layer) => any_copper_shown = true,
                    PcbLayer::Copper(copper) => {
                        view.shown_copper &= !(1 << pcb_geom::layer_bit(copper));
                    }
                    _ if !shown.is_visible(layer) => view.hidden.push(layer),
                    _ => {}
                }
            }
            // A through pad reaches every bit: with no copper shown, none.
            if !any_copper_shown {
                view.shown_copper = 0;
            }
            if !shown.is_visible(BOARD_OUTLINE) && !view.hidden.contains(&BOARD_OUTLINE) {
                view.hidden.push(BOARD_OUTLINE);
            }
            view.hidden_kinds = SNAP_OBJECT_KINDS
                .into_iter()
                .filter(|(object, _)| !shown.shows_object(*object))
                .map(|(_, kind)| kind)
                .collect();
        }
        view
    }

    fn carry_to(&mut self, from: Point, to: Point) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let Some(carry) = self
            .interaction_state
            .pcb_carry
            .as_ref()
            .filter(|_| self.has_own_pcb_carry())
        else {
            return;
        };
        let delta = if carry.text.is_some() {
            // A text follows the pointer exactly (#2368).
            Point::new(to.x - from.x, to.y - from.y)
        } else if carry.is_mixed() {
            let Some(delta) = self.mixed_drag_delta(carry, from, to) else {
                return;
            };
            delta
        } else if !carry.vias.is_empty() {
            let Some(delta) = self.via_preview_delta(carry, from, to) else {
                return;
            };
            delta
        } else if !carry.items.is_empty() || !carry.rooms.is_empty() {
            let Some(delta) = self.item_drag_delta(carry, from, to) else {
                return;
            };
            delta
        } else {
            let skip = carry.snap_skip(board);
            carry_delta(
                board,
                &carry.footprints,
                &skip,
                from,
                to,
                self.pcb_snap_view(),
            )
        };
        if let Some(carry) = self.interaction_state.pcb_carry.as_mut()
            && carry.delta != delta
        {
            carry.delta = delta;
            self.refresh_pcb_highlight();
        }
    }

    fn drop_pcb_carry(&mut self, from: Point, to: Point) {
        let Some(carry) = self
            .interaction_state
            .pcb_carry
            .clone()
            .filter(|_| self.has_own_pcb_carry())
        else {
            return;
        };
        if carry.text.is_some() {
            self.end_pcb_carry();
            self.drop_text_carry(&carry);
            return;
        }
        if carry.is_mixed() {
            self.drop_mixed_carry(&carry, from, to);
            return;
        }
        if !carry.vias.is_empty() {
            self.drop_via_carry(&carry, from, to);
            return;
        }
        if !carry.items.is_empty() || !carry.rooms.is_empty() {
            self.drop_item_carry(&carry, from, to);
            return;
        }
        if carry.footprints.is_empty() {
            // A drag refused at its start: the notice is already out.
            self.end_pcb_carry();
            return;
        }
        let built = self.active_pcb().map(|board| {
            let skip = carry.snap_skip(board);
            let view = self.pcb_snap_view();
            let delta = carry_delta(board, &carry.footprints, &skip, from, to, view);
            let commands = if carry.leaves_tracks {
                placement::move_footprints_only(board, &carry.footprints, delta)
            } else {
                placement::move_commands(board, &carry.footprints, delta, carry.stretch_corner)
            };
            (delta, commands)
        });
        self.end_pcb_carry();
        if let Some((delta, built)) = built {
            // The Command Log keeps the offset and the corner rule, not the
            // absolute positions, so a replay bends the tracks the same way.
            self.ui_state.command_replay.note_board_drop(
                carry.footprints.clone(),
                delta,
                carry.leaves_tracks,
                carry.stretch_corner,
            );
            self.run_placement("Move", built);
            self.ui_state.command_replay.forget_board_marks();
        }
    }

    /// Run a built placement edit on the board in view as one step, or say
    /// why not. Whether the board changed.
    pub(crate) fn run_placement(
        &mut self,
        what: &str,
        built: Result<Vec<BoardCommand>, PlacementRefusal>,
    ) -> bool {
        let commands = match built {
            Ok(commands) if commands.is_empty() => return false,
            Ok(commands) => commands,
            Err(refusal) => {
                refuse(what, &refusal);
                return false;
            }
        };
        let Some(path) = self.active_tab_path() else {
            return false;
        };
        match self.execute_board_commands(&path, commands, None) {
            Ok(patches) => {
                self.refresh_pcb_highlight();
                !patches.is_empty()
            }
            Err(error) => {
                crate::diagnostics::log_warning(format!("{what}: {error}; nothing changed"));
                false
            }
        }
    }

    /// The footprints a placement command would act on in the board in view.
    pub(crate) fn pcb_selected_footprints(&mut self) -> BTreeSet<Uuid> {
        self.with_pcb_selection(|state, _| state.selection().footprints())
            .unwrap_or_default()
    }
}

pub(crate) use signex_commands::PcbTargets;

/// The footprint and the two pads of a selection that is exactly two pads
/// of one footprint (#2410).
pub(crate) fn selected_pad_pair(selection: &pcb_geom::Selection) -> Option<(Uuid, Uuid, Uuid)> {
    let mut pads = selection.items().filter_map(|item| match item {
        ItemRef::Pad { footprint, pad } => Some((footprint, pad)),
        _ => None,
    });
    let (Some((footprint, a)), Some((other, b)), None) = (pads.next(), pads.next(), pads.next())
    else {
        return None;
    };
    (selection.len() == 2 && footprint == other).then_some((footprint, a, b))
}

impl Signex {
    /// [`PcbTargets`] for a command from `window` (`None`: the main one).
    pub(crate) fn pcb_targets(&self, window: Option<iced::window::Id>) -> PcbTargets {
        use crate::app::state::WindowKind;
        let undocked = window.filter(|id| self.ui_state.main_window_id != Some(*id));
        let (board, path, state) = match undocked {
            Some(id) => {
                let Some(WindowKind::UndockedTab { path, .. }) = self.ui_state.windows.get(&id)
                else {
                    // A detached panel or modal acts on the main window's board.
                    return self.pcb_targets(None);
                };
                (
                    self.pcb_for_window(id),
                    Some(path.clone()),
                    self.interaction_state.pcb_selections.get(&id),
                )
            }
            None => (
                self.active_pcb(),
                self.active_tab_path(),
                Some(&self.interaction_state.pcb_selection),
            ),
        };
        let Some(board) = board else {
            return PcbTargets::default();
        };
        let part_in_hand = self
            .interaction_state
            .pcb_carry
            .as_ref()
            .is_some_and(|c| c.kind == CarryKind::Place && c.is_for(path.as_deref(), undocked));
        let routing = self
            .interaction_state
            .pcb_tool
            .as_ref()
            .is_some_and(|s| s.is_routing() && s.is_for(path.as_deref(), undocked));
        let aligned = state.filter(|s| s.document().is_some() && s.document() == path.as_deref());
        let selection = aligned.map(|s| s.selection());
        // What the pointer is over: the board menu offers the object rows.
        let hover = aligned.and_then(|s| s.hover());
        let hovered = hover.is_some();
        let has_net = |item| pcb_geom::item_net(board, item).is_some();
        // A read-only board takes no edit: nothing on it is placeable,
        // deletable or a paste target (#2405).
        let read_only = board.unknown.newer_minor_source.is_some();
        let pasteable = self.interaction_state.pcb_clipboard.is_some() && !read_only;
        let Some(selection) = selection.filter(|s| !s.is_empty()) else {
            return PcbTargets {
                shown: true,
                pasteable,
                part_in_hand,
                routing,
                read_only,
                hovered,
                net: hover.is_some_and(has_net),
                ..PcbTargets::default()
            };
        };
        let footprints = selection.footprints();
        let unlocked = !read_only && placement::editable_footprints(board, &footprints).is_ok();
        let copyable = !deletable_items(selection).is_empty();
        let locks: Vec<bool> = super::pcb_item_edits::lockable_items(selection)
            .into_iter()
            .map(|id| signex_engine::pcb::lock::is_locked(board, id))
            .collect();
        let any_locked = locks.contains(&true);
        PcbTargets {
            shown: true,
            any: true,
            placeable: unlocked,
            deletable: !read_only && (footprints.is_empty() || unlocked) && copyable && !any_locked,
            copyable,
            pasteable,
            part_in_hand,
            routing,
            lockable: !read_only && locks.contains(&false),
            unlockable: !read_only && any_locked,
            read_only,
            footprints: footprints.len(),
            pad_pair: unlocked && selected_pad_pair(selection).is_some(),
            one_footprint: unlocked && footprints.len() == 1,
            routes: selection
                .items()
                .any(|item| matches!(item, ItemRef::Track(_) | ItemRef::Via(_))),
            hovered,
            net: selection.items().chain(hover).any(has_net),
        }
    }
}

/// What Delete removes for `selection`: its footprints (a selected pad's
/// included), tracks, vias, drawn areas, strings, free arcs, free pads,
/// dimensions and drawings (#2424).
pub(crate) fn deletable_items(selection: &pcb_geom::Selection) -> Vec<BoardItemId> {
    let mut items: Vec<BoardItemId> = selection
        .footprints()
        .into_iter()
        .map(BoardItemId::Footprint)
        .collect();
    items.extend(selection.items().filter_map(|item| match item {
        ItemRef::Track(id) => Some(BoardItemId::Segment(id)),
        ItemRef::Via(id) => Some(BoardItemId::Via(id)),
        ItemRef::Zone { zone, .. } => Some(BoardItemId::Zone(zone)),
        ItemRef::Text(id) => Some(BoardItemId::Text(id)),
        ItemRef::Arc(id) => Some(BoardItemId::Arc(id)),
        ItemRef::FreePad(id) => Some(BoardItemId::FreePad(id)),
        ItemRef::Dimension(id) => Some(BoardItemId::Dimension(id)),
        ItemRef::Graphic { graphic, .. } => Some(BoardItemId::Graphic(graphic)),
        // A designator or comment is hidden with its eye, never deleted.
        ItemRef::Pad { .. } | ItemRef::Footprint(_) | ItemRef::FootprintText { .. } => None,
        // A room is not a board item; it has its own commands (#2457).
        ItemRef::Room(_) => None,
    }));
    items
}

pub(super) fn refuse(what: &str, refusal: &dyn std::fmt::Display) {
    crate::diagnostics::log_warning(format!("{what}: {refusal}; nothing changed"));
}
