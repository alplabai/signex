//! Footprint sketch updates — numeric placement-input buffer concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). Arm
//! bodies are moved verbatim; each keeps its own inner `use`s.

use crate::library::messages::FootprintEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: FootprintEditorMsg) {
    match msg {
        FootprintEditorMsg::SketchPlacementInputChar(ch) => {
            // v0.24 Track D — append `ch` to `placement_input.buffer`,
            // minting a fresh entry against the active tool's matching
            // `PlacementInputKind` if one isn't already pinned. Drops
            // the keypress silently when the active tool / pending
            // state doesn't accept numeric input.
            use crate::library::editor::footprint::state::{PlacementInput, PlacementInputKind};
            let tool = editor.state.active_tool;
            let pending = editor.state.tool_pending.clone();
            let kind_for_active = PlacementInputKind::from_active_tool(tool, &pending);
            // Resolve the kind: if a buffer already exists, keep its
            // kind so the user can finish typing across a second
            // keypress; otherwise mint one matched to the tool.
            let kind = match editor.state.placement_input.as_ref() {
                Some(existing) => existing.kind,
                None => match kind_for_active {
                    Some(k) => k,
                    None => return, // tool doesn't accept numeric input
                },
            };
            // Validation:
            // - digits always allowed,
            // - one decimal point per buffer,
            // - leading minus only for `ArcSweep` and only at position 0,
            // - everything else dropped.
            let buf_ref = editor
                .state
                .placement_input
                .as_ref()
                .map(|p| p.buffer.as_str())
                .unwrap_or("");
            let accept = if ch.is_ascii_digit() {
                true
            } else if ch == '.' {
                !buf_ref.contains('.')
            } else if ch == '-' {
                kind.allows_negative() && buf_ref.is_empty()
            } else {
                false
            };
            if !accept {
                return;
            }
            // Mint or append.
            let entry = editor
                .state
                .placement_input
                .get_or_insert_with(|| PlacementInput {
                    buffer: String::new(),
                    kind,
                });
            entry.buffer.push(ch);
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::SketchPlacementInputBackspace => {
            // v0.24 Track D — pop one character; clear `placement_input`
            // entirely once the buffer empties so the next typed digit
            // mints a fresh entry against the (possibly different)
            // active tool.
            if let Some(entry) = editor.state.placement_input.as_mut() {
                entry.buffer.pop();
                if entry.buffer.is_empty() {
                    editor.state.placement_input = None;
                }
                editor.canvas_cache.clear();
            }
        }
        FootprintEditorMsg::SketchPlacementInputEnter => {
            // v0.24 Track D — Enter is a no-op on state. The buffer
            // stays alive so the next click consumes it. The message
            // is captured at the canvas layer purely so the keypress
            // doesn't fall through to a global shortcut.
        }
        FootprintEditorMsg::SketchPlacementInputEscape => {
            // v0.24 Track D — Esc throws away the buffer immediately;
            // the next click commits at the cursor position with no
            // override. Tool pending state is left intact so the
            // gesture itself isn't cancelled (use right-click / tool
            // Esc for that).
            if editor.state.placement_input.is_some()
                || !editor.state.placement_input_others.is_empty()
            {
                editor.state.placement_input = None;
                // v0.14-footprint — Esc also clears every stashed
                // dimension field so none leaks into the next gesture.
                editor.state.placement_input_others.clear();
                editor.canvas_cache.clear();
            }
        }
        FootprintEditorMsg::SketchPlacementInputTab => {
            // v0.14-footprint — cycle the focused dimension field to the
            // next one in the active tool's Tab order (Line len→angle,
            // Rectangle w→h, Rounded-Rect w→h→radius→w…). The focused
            // field lives in `placement_input`; the rest park in
            // `placement_input_others`, each keeping its own typed
            // digits. The canvas only emits this while a buffer is open
            // on a multi-field tool, but the dispatcher stays defensive
            // and no-ops unless the active tool exposes ≥2 fields.
            use crate::library::editor::footprint::state::{PlacementInput, PlacementInputKind};
            let fields = PlacementInputKind::placement_fields(
                editor.state.active_tool,
                &editor.state.tool_pending,
            );
            if fields.len() >= 2 {
                let current = editor
                    .state
                    .placement_input
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(fields[0]);
                let idx = fields.iter().position(|k| *k == current).unwrap_or(0);
                let next_kind = fields[(idx + 1) % fields.len()];
                // Park the focused field (preserving its digits),
                // replacing any stale same-kind entry; then pull the
                // next field out of the parked set, or mint it empty so
                // the next keypress appends to it.
                let prev_focused = editor.state.placement_input.take();
                let next_focused = match editor
                    .state
                    .placement_input_others
                    .iter()
                    .position(|p| p.kind == next_kind)
                {
                    Some(pos) => editor.state.placement_input_others.remove(pos),
                    None => PlacementInput {
                        buffer: String::new(),
                        kind: next_kind,
                    },
                };
                if let Some(prev) = prev_focused {
                    editor
                        .state
                        .placement_input_others
                        .retain(|p| p.kind != prev.kind);
                    editor.state.placement_input_others.push(prev);
                }
                editor.state.placement_input = Some(next_focused);
                editor.canvas_cache.clear();
            }
        }
        _ => unreachable!(
            "non-numeric placement-input buffer sketch variant routed to sketch_placement.rs"
        ),
    }
}
