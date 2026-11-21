//! The Repeat rows of a sheet symbol: the switch, Channel name and From /
//! To. Each writes the symbol's designator (`Repeat(CH, 1, 4)`) through the
//! child-sheet Name edit, one undo step per write, so typing the designator
//! and using the rows give one result.

use signex_types::schematic::{ChannelRepeat, ChildSheet};
use uuid::Uuid;

use super::state::RepeatDraft;
use crate::app::Signex;
use crate::panels::compiled_design::RepeatField;

/// The repeat the switch writes when a plain sheet symbol is first repeated.
pub(super) fn default_repeat() -> ChannelRepeat {
    ChannelRepeat {
        channel: "CH".to_string(),
        from: 1,
        to: 4,
    }
}

impl Signex {
    /// The sheet symbol `id` on the sheet being edited.
    pub(super) fn edited_sheet_symbol(&self, id: Uuid) -> Option<&ChildSheet> {
        let path = self.document_state.edit_path()?;
        self.document_state
            .engines
            .get(path)?
            .document()
            .child_sheets
            .iter()
            .find(|cs| cs.uuid == id)
    }

    /// Write `designator` to sheet symbol `id` through the Name edit.
    fn write_designator(&mut self, id: Uuid, designator: String) {
        let unchanged = self
            .edited_sheet_symbol(id)
            .is_some_and(|cs| cs.name == designator);
        if unchanged {
            return;
        }
        self.document_state.panel_ctx.child_sheet_name_buf = Some((id, designator));
        self.commit_child_sheet_name(id);
    }

    /// The Repeat switch: on writes a repeat (the last one typed, else
    /// `Repeat(CH, 1, 4)`) and keeps the plain designator; off restores it.
    pub(super) fn toggle_repeat(&mut self, id: Uuid, on: bool) {
        let Some(current) = self.edited_sheet_symbol(id).map(|cs| cs.name.clone()) else {
            return;
        };
        let is_repeated = ChannelRepeat::parse(&current).is_some();
        let store = &mut self.ui_state.compiled_design;
        store.repeat_draft = None;
        if on && !is_repeated {
            store.plain_names.insert(id, current);
            self.write_designator(id, default_repeat().to_designator());
        } else if !on && is_repeated {
            let kept = store.plain_names.remove(&id);
            let plain = kept.unwrap_or_else(|| self.plain_designator_for(id));
            self.write_designator(id, plain);
        }
    }

    /// A name for a symbol that was repeated before this session: its
    /// file's stem.
    fn plain_designator_for(&self, id: Uuid) -> String {
        self.edited_sheet_symbol(id)
            .and_then(|cs| {
                std::path::Path::new(&cs.filename)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_default()
    }

    pub(super) fn type_repeat_field(&mut self, id: Uuid, field: RepeatField, text: String) {
        let store = &mut self.ui_state.compiled_design;
        let draft = store.repeat_draft.get_or_insert_with(|| RepeatDraft {
            sheet_symbol: id,
            ..RepeatDraft::default()
        });
        if draft.sheet_symbol != id {
            *draft = RepeatDraft {
                sheet_symbol: id,
                ..RepeatDraft::default()
            };
        }
        draft.fields.insert(field, text);
    }

    /// Enter in a Repeat field: the typed fields over the current repeat
    /// make the new designator. A result that is not a valid repeat (an
    /// empty name, From after To, more than 64 channels) is not written; the
    /// typed text stays so it can be fixed.
    pub(super) fn submit_repeat_fields(&mut self, id: Uuid) {
        let Some(current) = self
            .edited_sheet_symbol(id)
            .and_then(|cs| ChannelRepeat::parse(&cs.name))
        else {
            return;
        };
        let Some(draft) = self
            .ui_state
            .compiled_design
            .repeat_draft
            .clone()
            .filter(|d| d.sheet_symbol == id)
        else {
            return;
        };
        let field = |f: RepeatField, now: String| draft.fields.get(&f).cloned().unwrap_or(now);
        let text = format!(
            "Repeat({}, {}, {})",
            field(RepeatField::Channel, current.channel.clone()),
            field(RepeatField::From, current.from.to_string()),
            field(RepeatField::To, current.to.to_string()),
        );
        let Some(repeat) = ChannelRepeat::parse(&text) else {
            return;
        };
        self.ui_state.compiled_design.repeat_draft = None;
        self.write_designator(id, repeat.to_designator());
    }
}
