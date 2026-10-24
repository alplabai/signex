//! Saving an editor's changes into a container file that another Signex
//! window may have rewritten since the editor loaded it (#902).
//!
//! A `.snxsym` / `.snxfpt` holds several primitives. An editor keeps the
//! whole container in memory, so writing that copy back would also write
//! back stale copies of the primitives it never touched, dropping whatever
//! another window saved to them. Instead the save is a three-way merge at
//! primitive granularity, keyed by uuid:
//!
//! - `baseline`: the container as the editor last loaded or saved it;
//! - `editor`: the editor's current container;
//! - `disk`: the container as it is on disk now.
//!
//! A primitive the editor changed, added or removed since `baseline` is
//! applied to `disk`. Every other primitive keeps its `disk` version. When
//! `disk` also changed one of the editor's primitives, the editor's version
//! wins and the uuid is reported, so the caller can say which change was
//! replaced.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{Footprint, FootprintFile, Symbol, SymbolFile};

/// What a merge applied, by primitive uuid.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContainerMerge {
    /// Primitives the editor changed, added or removed.
    pub changed: Vec<Uuid>,
    /// Of those, the ones another writer had also changed or removed on
    /// disk since the editor loaded them. The editor's version replaced
    /// theirs.
    pub overwritten: Vec<Uuid>,
}

/// Why a container could not be merged: two of its primitives share a
/// uuid, so a merge keyed by uuid cannot tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicatedUuid {
    /// In the editor's copy or the file it loaded: the file had them
    /// already.
    InEditorCopy,
    /// Only in the file on disk now: another writer added them since.
    OnDisk,
}

impl ContainerMerge {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty()
    }
}

/// The primitive types a container holds.
trait ContainerItem: Clone + PartialEq {
    fn uuid(&self) -> Uuid;
}

impl ContainerItem for Symbol {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

impl ContainerItem for Footprint {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

fn find<T: ContainerItem>(items: &[T], uuid: Uuid) -> Option<&T> {
    items.iter().find(|item| item.uuid() == uuid)
}

/// Whether two primitives in `items` share a uuid. A merge keyed by uuid
/// cannot tell them apart, so it refuses such a container.
fn has_duplicate_uuids<T: ContainerItem>(items: &[T]) -> bool {
    let mut seen = std::collections::HashSet::new();
    !items.iter().all(|item| seen.insert(item.uuid()))
}

/// Uuids of the primitives in `editor` that differ from, or are missing in,
/// `baseline`, followed by those `editor` removed.
fn changed_uuids<T: ContainerItem>(editor: &[T], baseline: &[T]) -> Vec<Uuid> {
    let edited = editor
        .iter()
        .filter(|item| find(baseline, item.uuid()) != Some(*item))
        .map(ContainerItem::uuid);
    let removed = baseline
        .iter()
        .filter(|item| find(editor, item.uuid()).is_none())
        .map(ContainerItem::uuid);
    edited.chain(removed).collect()
}

/// Apply the editor's `changed` primitives onto `disk`, keeping `disk`'s
/// order and appending primitives `disk` does not hold.
fn merge_items<T: ContainerItem>(
    disk: &mut Vec<T>,
    editor: &[T],
    baseline: &[T],
    changed: &[Uuid],
) -> Vec<Uuid> {
    let mut overwritten = Vec::new();
    for &uuid in changed {
        if find(disk, uuid) != find(baseline, uuid) {
            overwritten.push(uuid);
        }
        let position = disk.iter().position(|item| item.uuid() == uuid);
        match (find(editor, uuid), position) {
            (Some(item), Some(index)) => disk[index] = item.clone(),
            (Some(item), None) => disk.push(item.clone()),
            (None, Some(index)) => {
                disk.remove(index);
            }
            (None, None) => {}
        }
    }
    overwritten
}

impl SymbolFile {
    /// Uuids of the symbols this editor copy changed, added or removed
    /// since `baseline`.
    pub fn changed_since(&self, baseline: &SymbolFile) -> Vec<Uuid> {
        changed_uuids(&self.symbols, &baseline.symbols)
    }

    /// Merge this editor copy's changes since `baseline` into `disk`; see
    /// the module docs. `disk`'s file-level fields are kept, except the
    /// display name when this copy renamed the file, and `updated` is set to
    /// `now` when anything changed. Refused when two symbols share a uuid,
    /// which a merge cannot tell apart.
    pub fn merge_into(
        &self,
        baseline: &SymbolFile,
        mut disk: SymbolFile,
        now: DateTime<Utc>,
    ) -> Result<(SymbolFile, ContainerMerge), DuplicatedUuid> {
        if has_duplicate_uuids(&self.symbols) || has_duplicate_uuids(&baseline.symbols) {
            return Err(DuplicatedUuid::InEditorCopy);
        }
        if has_duplicate_uuids(&disk.symbols) {
            return Err(DuplicatedUuid::OnDisk);
        }
        let mut changed = self.changed_since(baseline);
        let renamed = self.display_name != baseline.display_name;
        let overwritten = merge_items(
            &mut disk.symbols,
            &self.symbols,
            &baseline.symbols,
            &changed,
        );
        if renamed {
            disk.display_name = self.display_name.clone();
        }
        if !changed.is_empty() || renamed {
            disk.updated = now;
        }
        if renamed && changed.is_empty() {
            changed.push(self.file_uuid);
        }
        Ok((
            disk,
            ContainerMerge {
                changed,
                overwritten,
            },
        ))
    }
}

impl FootprintFile {
    /// Uuids of the footprints this editor copy changed, added or removed
    /// since `baseline`.
    pub fn changed_since(&self, baseline: &FootprintFile) -> Vec<Uuid> {
        changed_uuids(&self.footprints, &baseline.footprints)
    }

    /// The footprint counterpart of [`SymbolFile::merge_into`].
    ///
    /// A footprint whose only difference from `baseline` is the repair the
    /// loader made (its `load_repairs` record) is written only while `disk`
    /// still holds the version that was repaired. When another window saved
    /// it since, its version is kept: this editor never edited it.
    pub fn merge_into(
        &self,
        baseline: &FootprintFile,
        mut disk: FootprintFile,
        now: DateTime<Utc>,
    ) -> Result<(FootprintFile, ContainerMerge), DuplicatedUuid> {
        if has_duplicate_uuids(&self.footprints) || has_duplicate_uuids(&baseline.footprints) {
            return Err(DuplicatedUuid::InEditorCopy);
        }
        if has_duplicate_uuids(&disk.footprints) {
            return Err(DuplicatedUuid::OnDisk);
        }
        let mut changed = self.changed_since(baseline);
        changed.retain(|&uuid| {
            let (Some(edited), Some(loaded)) = (
                find(&self.footprints, uuid),
                find(&baseline.footprints, uuid),
            ) else {
                return true;
            };
            let mut repaired = loaded.clone();
            repaired.load_repairs.clear();
            *edited != repaired || find(&disk.footprints, uuid) == Some(loaded)
        });
        let renamed = self.display_name != baseline.display_name;
        let overwritten = merge_items(
            &mut disk.footprints,
            &self.footprints,
            &baseline.footprints,
            &changed,
        );
        if renamed {
            disk.display_name = self.display_name.clone();
        }
        if !changed.is_empty() || renamed {
            disk.updated = now;
        }
        if renamed && changed.is_empty() {
            changed.push(self.file_uuid);
        }
        Ok((
            disk,
            ContainerMerge {
                changed,
                overwritten,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(name: &str) -> Symbol {
        Symbol::empty(name)
    }

    fn container(symbols: Vec<Symbol>) -> SymbolFile {
        let mut file = SymbolFile::from_symbol(symbols[0].clone());
        file.symbols = symbols;
        file
    }

    #[test]
    fn edits_to_different_symbols_from_two_editors_both_survive() {
        let (a, b) = (symbol("A"), symbol("B"));
        let baseline = container(vec![a.clone(), b.clone()]);

        let mut first = baseline.clone();
        first.symbols[0].description = "first".into();
        let mut second = baseline.clone();
        second.symbols[1].description = "second".into();

        let (disk, _) = first
            .merge_into(&baseline, baseline.clone(), Utc::now())
            .unwrap();
        let (disk, merge) = second.merge_into(&baseline, disk, Utc::now()).unwrap();

        assert_eq!(disk.symbols[0].description, "first");
        assert_eq!(disk.symbols[1].description, "second");
        assert_eq!(merge.changed, [b.uuid]);
        assert!(merge.overwritten.is_empty());
    }

    #[test]
    fn an_unchanged_editor_copy_changes_nothing_on_disk() {
        let baseline = container(vec![symbol("A")]);
        let mut disk = baseline.clone();
        disk.symbols[0].description = "elsewhere".into();

        let (merged, merge) = baseline
            .merge_into(&baseline, disk.clone(), Utc::now())
            .unwrap();

        assert!(merge.is_empty());
        assert_eq!(merged, disk);
    }

    #[test]
    fn the_same_symbol_changed_on_both_sides_is_reported_and_the_editor_wins() {
        let a = symbol("A");
        let baseline = container(vec![a.clone()]);
        let mut disk = baseline.clone();
        disk.symbols[0].description = "elsewhere".into();
        let mut editor = baseline.clone();
        editor.symbols[0].description = "here".into();

        let (merged, merge) = editor.merge_into(&baseline, disk, Utc::now()).unwrap();

        assert_eq!(merged.symbols[0].description, "here");
        assert_eq!(merge.overwritten, [a.uuid]);
    }

    #[test]
    fn added_and_removed_symbols_are_applied_and_others_added_elsewhere_kept() {
        let (a, b, c, d) = (symbol("A"), symbol("B"), symbol("C"), symbol("D"));
        let baseline = container(vec![a.clone(), b.clone()]);
        let mut disk = baseline.clone();
        disk.symbols.push(c.clone());
        let editor = container(vec![a.clone(), d.clone()]);

        let (merged, merge) = editor.merge_into(&baseline, disk, Utc::now()).unwrap();

        let names: Vec<&str> = merged.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["A", "C", "D"]);
        assert_eq!(merge.changed, [d.uuid, b.uuid]);
        assert!(merge.overwritten.is_empty());
    }

    #[test]
    fn a_container_with_a_duplicated_uuid_is_not_merged() {
        let a = symbol("A");
        let baseline = container(vec![a.clone(), a.clone()]);
        let mut editor = baseline.clone();
        editor.symbols[1].description = "second copy".into();

        assert_eq!(
            editor.merge_into(&baseline, baseline.clone(), Utc::now()),
            Err(DuplicatedUuid::InEditorCopy)
        );

        // #904 review: a duplicate only on disk was added by another
        // writer since; the save is refused rather than overwriting it.
        let single = container(vec![a.clone()]);
        let mut disk = container(vec![a.clone(), symbol("B")]);
        disk.symbols[1].uuid = a.uuid;
        assert_eq!(
            single.merge_into(&single, disk, Utc::now()),
            Err(DuplicatedUuid::OnDisk)
        );
    }

    fn footprint(name: &str) -> Footprint {
        Footprint::empty(name)
    }

    /// #902 review: a footprint repaired on load but never edited must not
    /// replace another window's later save of it.
    #[test]
    fn a_footprint_only_repaired_on_load_keeps_another_windows_save() {
        let (y, z) = (footprint("Y"), footprint("Z"));
        let mut loaded = FootprintFile::from_footprint(y.clone());
        loaded.footprints.push(z.clone());
        loaded.footprints[0]
            .load_repairs
            .push(crate::primitive::LoadRepair::DanglingRemoved(1));
        let mut editor = loaded.clone();
        editor.footprints[0].load_repairs.clear();
        editor.footprints[1].description = "edited here".into();
        let mut disk = loaded.clone();
        disk.footprints[0].description = "saved elsewhere".into();

        let (merged, merge) = editor.merge_into(&loaded, disk, Utc::now()).unwrap();

        assert_eq!(merged.footprints[0].description, "saved elsewhere");
        assert_eq!(merged.footprints[1].description, "edited here");
        assert_eq!(merge.changed, [z.uuid]);
    }

    /// While the disk still holds the repaired version, the repair is saved.
    #[test]
    fn a_footprint_repaired_on_load_is_written_while_the_disk_is_unchanged() {
        let y = footprint("Y");
        let mut loaded = FootprintFile::from_footprint(y.clone());
        loaded.footprints[0]
            .load_repairs
            .push(crate::primitive::LoadRepair::DanglingRemoved(1));
        let mut editor = loaded.clone();
        editor.footprints[0].load_repairs.clear();

        let (_, merge) = editor
            .merge_into(&loaded, loaded.clone(), Utc::now())
            .unwrap();

        assert_eq!(merge.changed, [y.uuid]);
    }
}
