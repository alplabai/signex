//! The placement session: a library part attached to the cursor.
//!
//! Armed by `handle_library_component_resolved` (library_place.rs), turned
//! by Space / X / Y, edited from the Tab form, committed by a click and
//! dropped by Esc, right-click, a tool change, a sheet switch or a document
//! close. Everything here mutates app state only through `update`; the
//! commit is ONE engine batch, so a click is one undo step.
//!
//! The designator is recomputed from the engine's document at every click,
//! not carried forward, so the clicks give R1, R2, R3 and an undone part
//! frees its number. A designator typed in the Tab form is used by the next
//! click, then stepped with `increment_designator`.

use std::path::Path;

use iced::Task;
use signex_engine::{MirrorAxis, PlacementPayload};
use signex_types::schematic::Point;

use crate::app::state::{PlacementKind, PlacementSession};
use crate::app::{Message, Signex, Tool};
use crate::canvas::PlacementGhost;

/// What the Tab form shows for the part on the cursor.
pub(crate) struct PlacementForm {
    pub value: String,
    pub designator: String,
    pub rotation: f64,
}

impl Signex {
    /// Arm a session for `payload` on `target_sheet`. Cancels every other
    /// armed tool first, so a leftover ghost or half-drawn wire cannot
    /// survive into the session.
    pub(crate) fn arm_placement_session(
        &mut self,
        payload: PlacementPayload,
        target_sheet: &Path,
        kind: PlacementKind,
    ) {
        // Arming is not a cancel: reads still in flight keep the token they
        // were issued, so the latest pick can still arm (see
        // `InteractionState::placement_epoch`).
        let token = self.interaction_state.placement_epoch;
        self.clear_transient_schematic_tool_state();
        self.interaction_state.placement_epoch = token;
        self.clear_ghost_previews();
        self.interaction_state.current_tool = Tool::Component;
        self.interaction_state.placement = Some(PlacementSession::new(
            payload,
            target_sheet.to_path_buf(),
            kind,
        ));
    }

    /// Drop the session, if any, and invalidate every library read still in
    /// flight. Returns whether a session was armed.
    pub(crate) fn drop_placement_session(&mut self) -> bool {
        self.interaction_state.placement_epoch += 1;
        self.interaction_state.placement.take().is_some()
    }

    /// [`Self::drop_placement_session`] for the places that also own the
    /// tool: a session that ends leaves no placement tool behind.
    pub(crate) fn cancel_placement_session(&mut self) {
        if self.drop_placement_session() {
            self.interaction_state.current_tool = Tool::Select;
            self.interaction_state.active_canvas_mut().tool_preview = None;
        }
    }

    /// End the session when the sheet it was armed for no longer has an
    /// engine (its tab closed clean, its project closed): there is nothing
    /// left to place into.
    pub(crate) fn cancel_placement_session_if_sheet_gone(&mut self) {
        let gone = self
            .interaction_state
            .placement
            .as_ref()
            .is_some_and(|s| !self.document_state.engines.contains_key(&s.target_sheet));
        if gone {
            self.cancel_placement_session();
        }
    }

    /// The ghost the window showing `window_id` draws: the session's, when
    /// that window shows the session's target sheet.
    pub fn placement_ghost_for_window(
        &self,
        window_id: iced::window::Id,
    ) -> Option<&PlacementGhost> {
        use crate::app::state::WindowKind;
        let session = self.interaction_state.placement.as_ref()?;
        let shown = if self.ui_state.main_window_id == Some(window_id) {
            self.document_state.active_path.as_deref()?
        } else {
            match self.ui_state.windows.get(&window_id)? {
                WindowKind::UndockedTab { path, .. } => path.as_path(),
                _ => return None,
            }
        };
        (shown == session.target_sheet).then_some(&session.ghost)
    }

    /// What the Tab form starts from: the part's value, the designator the
    /// next click would use, and its rotation.
    pub(crate) fn placement_form(&self) -> Option<PlacementForm> {
        let session = self.interaction_state.placement.as_ref()?;
        let symbol = session.payload.lone_symbol()?;
        Some(PlacementForm {
            value: symbol.value.clone(),
            designator: self.next_placement_reference(session)?,
            rotation: symbol.rotation,
        })
    }

    /// The designator the next click gives the part; `None` for pasted
    /// content, which keeps its own.
    fn next_placement_reference(&self, session: &PlacementSession) -> Option<String> {
        let prefix = session.kind.prefix()?;
        Some(match session.reference_override.as_deref() {
            Some(typed) => self.free_designator_from(&session.target_sheet, typed),
            None => self.next_designator_in_sheet(&session.target_sheet, prefix),
        })
    }

    /// `typed`, or the first designator stepped from it that no symbol in
    /// `sheet`'s project uses, so a typed `R1` never makes a second `R1`.
    pub(crate) fn free_designator_from(&self, sheet: &Path, typed: &str) -> String {
        let used: std::collections::HashSet<&str> =
            self.used_designators(sheet).into_iter().collect();
        let mut candidate = typed.to_owned();
        while used.contains(candidate.as_str()) {
            match Self::increment_designator(&candidate) {
                Some(next) => candidate = next,
                None => break,
            }
        }
        candidate
    }

    /// Keep the designator the ghost shows equal to the one the next click
    /// would use: after a Tab override, a commit, or an undo that freed a
    /// number. Cheap, so it runs after every update.
    pub(crate) fn refresh_placement_reference(&mut self) {
        let Some(session) = self.interaction_state.placement.as_ref() else {
            return;
        };
        let Some(next) = self.next_placement_reference(session) else {
            return;
        };
        let stale = session
            .payload
            .lone_symbol()
            .is_some_and(|s| s.reference != next);
        if stale && let Some(session) = self.interaction_state.placement.as_mut() {
            let payload = session.payload.map_symbols(|s| s.reference = next.clone());
            session.set_payload(payload);
        }
    }

    /// Redraw a paste's ghost with the designators its next click gives, from
    /// the same numbering the click uses. Not per update: the numbering walks
    /// the sheet, so it runs when the ghost changes (armed, turned, placed).
    pub(crate) fn refresh_paste_ghost(&mut self) {
        let Some(session) = self.interaction_state.placement.as_ref() else {
            return;
        };
        if !session.kind.is_paste() {
            return;
        }
        let numbered = self
            .paste_numbering(&session.target_sheet)
            .apply(&session.payload);
        if let Some(session) = self.interaction_state.placement.as_mut() {
            session.ghost = PlacementGhost::new(&numbered);
        }
    }

    /// A click at `(x, y)` (already snapped): place the payload there as one
    /// undo step and stay armed for the next copy.
    pub(crate) fn handle_placement_click(&mut self, x: f64, y: f64) -> Task<Message> {
        self.cancel_placement_session_if_sheet_gone();
        let Some(session) = self.interaction_state.placement.as_ref() else {
            return Task::none();
        };
        if self.document_state.active_path.as_deref() != Some(session.target_sheet.as_path()) {
            // A click in another window: the part belongs to the sheet it
            // was armed for, so nothing lands here and the session stays.
            let message = format!(
                "The part on the cursor belongs to {}; click on that sheet to place it.",
                session.target_sheet.display()
            );
            crate::diagnostics::log_warning(message);
            return Task::none();
        }
        let reference = self.next_placement_reference(session);
        if let (Some(typed), Some(used)) = (session.reference_override.as_deref(), &reference)
            && typed != used
        {
            crate::diagnostics::log_warning(format!(
                "Designator {typed} is already used on this sheet; the part is placed as {used}."
            ));
        }
        let target = session.target_sheet.clone();
        let payload = match &reference {
            Some(reference) => session
                .payload
                .map_symbols(|s| s.reference = reference.clone()),
            None => self.paste_numbering(&target).apply(&session.payload),
        };
        let placed = payload.place_commands(Point::new(x, y));
        let commands = match placed {
            Ok(commands) => commands,
            Err(error) => {
                self.report_place_failure(&format!("It could not be placed: {error}"));
                return Task::none();
            }
        };
        // `false` covers an engine error (logged by the gateway) or an
        // unchanged result; either way nothing was placed.
        if !self.apply_engine_commands(commands, false, false) {
            self.report_place_failure("Nothing was placed; see the log for details.");
            return Task::none();
        }
        self.after_placement_commit(reference.as_deref());
        Task::none()
    }

    /// Step a typed designator and redraw the ghost with the next one; a
    /// one-shot session (paste) ends here. `used` is the designator the
    /// click placed, which differs from the typed one when that was taken.
    fn after_placement_commit(&mut self, used: Option<&str>) {
        let Some(session) = self.interaction_state.placement.as_mut() else {
            return;
        };
        if session.kind.ends_after_click() {
            self.cancel_placement_session();
            return;
        }
        session.reference_override = session
            .reference_override
            .as_ref()
            .and(used)
            .and_then(Self::increment_designator);
        self.refresh_placement_reference();
        self.refresh_paste_ghost();
    }

    /// Space: turn the part on the cursor. `false` when none is armed, so
    /// the caller turns the selection instead.
    pub(crate) fn rotate_placement(&mut self, clockwise: bool) -> bool {
        self.turn_placement(|payload| payload.rotate(clockwise))
    }

    /// X / Y: mirror the part on the cursor.
    pub(crate) fn mirror_placement(&mut self, axis: MirrorAxis) -> bool {
        self.turn_placement(|payload| payload.mirror(axis))
    }

    fn turn_placement(
        &mut self,
        turn: impl FnOnce(&PlacementPayload) -> Result<PlacementPayload, signex_engine::EngineError>,
    ) -> bool {
        let Some(session) = self.interaction_state.placement.as_mut() else {
            return false;
        };
        match turn(&session.payload) {
            Ok(payload) => session.set_payload(payload),
            Err(error) => crate::diagnostics::log_warning(format!(
                "The part on the cursor could not be turned: {error}"
            )),
        }
        self.refresh_paste_ghost();
        true
    }

    /// Tab form: the value typed for the next part.
    pub(crate) fn set_placement_value(&mut self, text: &str) {
        let value = text.trim();
        if value.is_empty() {
            return;
        }
        if let Some(session) = self.interaction_state.placement.as_mut()
            && !session.kind.is_paste()
        {
            let payload = session.payload.map_symbols(|s| s.value = value.to_owned());
            session.set_payload(payload);
        }
    }

    /// Tab form: the designator typed for the next part (blank = automatic).
    pub(crate) fn set_placement_designator(&mut self, text: &str) {
        if let Some(session) = self.interaction_state.placement.as_mut() {
            let text = text.trim();
            session.reference_override = (!text.is_empty()).then(|| text.to_owned());
        }
        self.refresh_placement_reference();
    }

    /// Tab form: the rotation picked for the next part.
    pub(crate) fn set_placement_rotation(&mut self, degrees: f64) {
        let Some(session) = self.interaction_state.placement.as_mut() else {
            return;
        };
        if session.kind.is_paste() {
            return;
        }
        match session.payload.with_rotation(degrees) {
            Ok(payload) => session.set_payload(payload),
            Err(error) => crate::diagnostics::log_warning(format!(
                "The part on the cursor could not be rotated: {error}"
            )),
        }
    }
}
