//! Smart Paste placed at once from the dialog's choices (#2665, mockup row
//! 155): the operand form of `smart_paste`. The dialog's own checks
//! (`SmartPasteState`) and copies (`smart_paste::plan`), numbered from the
//! project's designators as read, placed as one undo step with copy 1 at the
//! anchor plus the offset, as the dialog's click places them.

use signex_i18n::{Translatable, ids};
use signex_types::schematic::Point;

use super::PasteSeed;
use crate::app::Signex;
use crate::app::handlers::clipboard_workflows::designator_rule;
use crate::app::mutation_gateway::MutationOutcome;
use crate::smart_paste::SmartPasteState;
use crate::smart_paste::plan::{self, NetAfter};

/// What a Smart Paste placed at once.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlacedPaste {
    /// Where the copies' anchor landed; copy `k` sits `k` offsets from it.
    pub(crate) at: Point,
    pub(crate) copies: u32,
    /// The offset of copy 1 and the step between copies, in mm.
    pub(crate) delta: (f64, f64),
    /// The items placed, every copy counted.
    pub(crate) items: usize,
    /// Each copied designator and what each copy calls it.
    pub(crate) designators: Vec<(String, Vec<String>)>,
    /// Each copied net and what the copies call it.
    pub(crate) nets: Vec<(String, NetAfter)>,
}

impl Signex {
    /// Place the in-app copy on `state.target`, the sheet the edit targets,
    /// as `state` chooses, at once, as one undo step, anchored at `at` (else
    /// the copy's own place).
    /// `seed` is the project as read: its designators number the copies and
    /// its net names stop a rewrite that would join a net, as in the dialog.
    /// Refused with the reason the dialog gives, nothing placed.
    pub(crate) fn smart_paste_at_once(
        &mut self,
        state: &mut SmartPasteState,
        seed: &PasteSeed,
        at: Option<Point>,
    ) -> Result<PlacedPaste, Translatable> {
        let target = state.target.clone();
        let copied = self
            .interaction_state
            .clipboard
            .clone()
            .ok_or_else(|| Translatable::new(ids::SMART_PASTE_NOTHING_COPIED))?;
        let read_only = self
            .document_state
            .engines
            .get(&target)
            .is_some_and(|engine| engine.document().unknown.newer_minor_source.is_some());
        if read_only {
            return Err(Translatable::new(ids::NOTICE_PASTE_READ_ONLY)
                .arg("path", target.display().to_string()));
        }
        let source = copied.payload.fragment();
        state.copied_nets = plan::copied_nets(source);
        state.names_in_use = self.paste_names_in_use(&target, seed.net_names.clone());
        state.unread_sheets = seed.sheets_unread();
        let input = state
            .input()
            .map_err(|_| Translatable::new(ids::SMART_PASTE_CHOICES_INVALID))?;
        let taken = state.taken_names();
        if !taken.is_empty() {
            return Err(
                Translatable::new(ids::SMART_PASTE_NAMES_TAKEN).arg("names", taken.join(", "))
            );
        }
        let unread = state.unread_sheets_text();
        if !unread.is_empty() {
            return Err(Translatable::new(ids::SMART_PASTE_SHEET_UNREAD).arg("sheets", unread));
        }
        let copies = plan::copies_payload(&copied.payload, &input);
        let numbered = self
            .paste_numbering_over(
                &target,
                designator_rule(input.designators),
                seed.counters.clone(),
                seed.used.clone(),
            )
            .apply(&copies);
        let anchor = at.unwrap_or(copied.origin);
        let not_placed =
            |why: String| Translatable::new(ids::SMART_PASTE_NOT_PLACED).arg("error", why);
        let commands = numbered
            .place_commands(anchor, &mut signex_types::object_id::UuidV7Ids)
            .map_err(|error| not_placed(error.to_string()))?;
        let (outcome, reason) = self.apply_engine_commands_reporting(commands, true, true);
        if outcome == MutationOutcome::Failed {
            return Err(not_placed(reason.unwrap_or_default()));
        }
        Ok(PlacedPaste {
            at: anchor,
            copies: input.copies,
            delta: (input.dx, input.dy),
            items: numbered.item_count(),
            designators: plan::designator_mapping(source, numbered.fragment(), input.copies),
            nets: plan::after_paste(&state.copied_nets, &input),
        })
    }
}
