//! `Command::CreateUnion`, `UpdateUnion`, `SetUnionMembers` and `BreakUnion`
//! (#158), and the step every changed edit ends with: members the sheet no
//! longer holds leave their union, and a union left empty goes, in the same
//! undo step. Rules: `docs/SCHEMATIC_UNIONS.md`.

use std::collections::HashSet;

use signex_types::schematic::SchUnion;
use uuid::Uuid;

use crate::history::HistoryStep;
use crate::unions::{UnionRefusal, normalized_tags, object_kinds};
use crate::*;

/// The typing-run key of a union rename or tag edit; the run is also keyed
/// by the union's id, which no symbol shares.
const UNION_RUN_KEY: &str = "union";

fn union_patch() -> PatchPair {
    PatchPair {
        semantic: SemanticPatch::StyleUpdated,
        document: DocumentPatch::UNIONS,
    }
}

impl Engine {
    pub(crate) fn exec_unions(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        match cmd {
            Command::CreateUnion { union } => self.create_union(before, union),
            Command::UpdateUnion {
                uuid,
                name,
                tags,
                locked,
            } => self.update_union(before, uuid, &name, &tags, locked),
            Command::SetUnionMembers { uuid, members } => {
                self.set_union_members(before, uuid, members)
            }
            Command::BreakUnion { uuid } => self.break_union(before, uuid),
            _ => unreachable!("exec_unions routes only the union commands"),
        }
    }

    /// `members` without repeats, checked: each is an object of this sheet
    /// and in no union but `union`.
    fn checked_members(&self, union: Uuid, members: Vec<Uuid>) -> Result<Vec<Uuid>, EngineError> {
        let kinds = object_kinds(&self.document);
        let mut seen = HashSet::new();
        let mut out = Vec::with_capacity(members.len());
        for id in members {
            if !seen.insert(id) {
                continue;
            }
            if !kinds.contains_key(&id) {
                return Err(UnionRefusal::MemberNotOnSheet.into());
            }
            out.push(id);
        }
        let mut objects = 0;
        let mut unions = Vec::new();
        for other in self.document.unions.iter().filter(|u| u.uuid != union) {
            let taken = other.members.iter().filter(|m| seen.contains(m)).count();
            if taken > 0 {
                objects += taken;
                unions.push(other.name.clone());
            }
        }
        if !unions.is_empty() {
            return Err(UnionRefusal::MemberInAnotherUnion { objects, unions }.into());
        }
        Ok(out)
    }

    fn union_slot(&self, uuid: Uuid) -> Result<usize, EngineError> {
        self.document
            .unions
            .iter()
            .position(|u| u.uuid == uuid)
            .ok_or_else(|| UnionRefusal::NotFound(uuid).into())
    }

    fn create_union(
        &mut self,
        before: SchematicSheet,
        union: SchUnion,
    ) -> Result<CommandResult, EngineError> {
        let name = union.name.trim().to_string();
        if name.is_empty() {
            return Err(UnionRefusal::NameEmpty.into());
        }
        if self.document.unions.iter().any(|u| u.uuid == union.uuid) {
            return Err(UnionRefusal::IdInUse(union.uuid).into());
        }
        let members = self.checked_members(union.uuid, union.members)?;
        if members.is_empty() {
            return Err(UnionRefusal::NoMembers.into());
        }
        self.document.unions.push(SchUnion {
            uuid: union.uuid,
            name,
            tags: normalized_tags(&union.tags),
            members,
            locked: union.locked,
        });
        let patch_pair = union_patch();
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    /// A rename or tag edit is a typing run (one undo step for the keys of
    /// one field); a lock change is a step of its own.
    fn update_union(
        &mut self,
        before: SchematicSheet,
        uuid: Uuid,
        name: &str,
        tags: &[String],
        locked: bool,
    ) -> Result<CommandResult, EngineError> {
        let at = self.union_slot(uuid)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(UnionRefusal::NameEmpty.into());
        }
        let tags = normalized_tags(tags);
        let slot = &mut self.document.unions[at];
        let lock_changed = slot.locked != locked;
        if slot.name == name && slot.tags == tags && !lock_changed {
            return Ok(CommandResult::unchanged());
        }
        slot.name = name.to_string();
        slot.tags = tags;
        slot.locked = locked;
        let patch_pair = union_patch();
        if lock_changed {
            self.record_history(before, patch_pair);
        } else {
            self.record_field_edit(before, patch_pair, uuid, UNION_RUN_KEY);
        }
        Ok(CommandResult::changed(patch_pair))
    }

    /// Replace the members; an empty list breaks the union. Refused on a
    /// locked union.
    fn set_union_members(
        &mut self,
        before: SchematicSheet,
        uuid: Uuid,
        members: Vec<Uuid>,
    ) -> Result<CommandResult, EngineError> {
        let at = self.union_slot(uuid)?;
        self.refuse_locked_union(at, "changed")?;
        if members.is_empty() {
            return self.break_union(before, uuid);
        }
        let members = self.checked_members(uuid, members)?;
        let slot = &mut self.document.unions[at];
        if slot.members == members {
            return Ok(CommandResult::unchanged());
        }
        slot.members = members;
        let patch_pair = union_patch();
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    /// Remove the union; its objects stay on the sheet.
    fn break_union(
        &mut self,
        before: SchematicSheet,
        uuid: Uuid,
    ) -> Result<CommandResult, EngineError> {
        let at = self.union_slot(uuid)?;
        self.refuse_locked_union(at, "broken")?;
        self.document.unions.remove(at);
        let patch_pair = union_patch();
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    fn refuse_locked_union(&self, at: usize, action: &'static str) -> Result<(), EngineError> {
        let union = &self.document.unions[at];
        if union.locked {
            return Err(UnionRefusal::Locked {
                name: union.name.clone(),
                action,
            }
            .into());
        }
        Ok(())
    }

    /// After a changed edit: drop members the sheet no longer holds and the
    /// unions this edit leaves empty, in the step the edit recorded. The
    /// patch of the result and of that history step then covers the unions
    /// too. A union that was empty already (the loader keeps one) is left
    /// alone, so an unrelated edit never changes it.
    pub(crate) fn prune_union_members(&mut self, result: &mut Result<CommandResult, EngineError>) {
        let Ok(done) = result.as_mut() else { return };
        if !done.changed || self.document.unions.is_empty() {
            return;
        }
        let kinds = object_kinds(&self.document);
        let mut pruned = false;
        let mut emptied: Vec<Uuid> = Vec::new();
        for union in &mut self.document.unions {
            let count = union.members.len();
            union.members.retain(|id| kinds.contains_key(id));
            if union.members.len() != count {
                pruned = true;
                if union.members.is_empty() {
                    emptied.push(union.uuid);
                }
            }
        }
        self.document.unions.retain(|u| !emptied.contains(&u.uuid));
        if !pruned {
            return;
        }
        if let Some(pair) = done.patch_pair.as_mut() {
            pair.document |= DocumentPatch::UNIONS;
        }
        let semantic = done
            .patch_pair
            .map_or(SemanticPatch::StyleUpdated, |p| p.semantic);
        if self.history_suppressed {
            self.absorb_into_transaction(PatchPair {
                semantic,
                document: DocumentPatch::UNIONS,
            });
        } else if let Some(HistoryStep::Edit(entry)) = self.history.undo_steps_mut().last_mut() {
            entry.patch_pair.document |= DocumentPatch::UNIONS;
        }
    }
}
