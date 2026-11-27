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

    /// Write `designator` to sheet symbol `id` through the Name edit. `true`
    /// when the symbol holds it afterwards, `false` when the edit was refused.
    fn write_designator(&mut self, id: Uuid, designator: String) -> bool {
        let holds_it = |app: &Self| {
            app.edited_sheet_symbol(id)
                .is_some_and(|cs| cs.name == designator)
        };
        if holds_it(&*self) {
            return true;
        }
        self.document_view.panel_ctx.child_sheet_name_buf = Some((id, designator.clone()));
        self.commit_child_sheet_name(id);
        holds_it(&*self)
    }

    /// The Repeat switch: on writes `Repeat(CH, 1, 4)`, off restores the
    /// plain designator the symbol last had.
    pub(super) fn toggle_repeat(&mut self, id: Uuid, on: bool) {
        let Some(is_repeated) = self
            .edited_sheet_symbol(id)
            .map(|cs| ChannelRepeat::parse(&cs.name).is_some())
        else {
            return;
        };
        self.ui_state.compiled_design.repeat_draft = None;
        if on && !is_repeated {
            self.write_designator(id, default_repeat().to_designator());
        } else if !on && is_repeated {
            let plain = self.plain_designator_for(id);
            self.write_designator(id, plain);
        }
    }

    /// The plain designator in the sheet's undo history, so typing a repeat
    /// and undoing an off both keep it. With none there (the file already
    /// held the repeat), the file's stem, numbered (`stem_2`, ...) when
    /// another sheet symbol holds it (#2790), as a symbol made from a sheet
    /// is named; a file with no stem takes the channel name.
    fn plain_designator_for(&self, id: Uuid) -> String {
        let Some(engine) = self.document_state.edit_engine() else {
            return String::new();
        };
        if let Some(name) = engine.earlier_plain_sheet_name(id) {
            return name.to_string();
        }
        let Some(cs) = self.edited_sheet_symbol(id) else {
            return String::new();
        };
        let stem = std::path::Path::new(&cs.filename)
            .file_stem()
            .map(|s| s.to_string_lossy().trim().to_string())
            .filter(|stem| !stem.is_empty())
            .or_else(|| ChannelRepeat::parse(&cs.name).map(|repeat| repeat.channel))
            .unwrap_or_default();
        signex_engine::unused_sheet_symbol_name(engine.document(), &stem)
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
        draft.refused = false;
    }

    /// Enter in a Repeat field: the typed fields over the current repeat
    /// make the new designator. A result that is not a valid repeat (an
    /// empty name, From after To, more than 64 channels) or that another
    /// sheet symbol holds is not written; the typed text stays, marked, so
    /// it can be fixed.
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
        let is_written = ChannelRepeat::parse(&text)
            .is_some_and(|repeat| self.write_designator(id, repeat.to_designator()));
        let store = &mut self.ui_state.compiled_design;
        if is_written {
            store.repeat_draft = None;
        } else if let Some(draft) = store.repeat_draft.as_mut() {
            draft.refused = true;
        }
    }
}
