//! The blocking half of a symbol / footprint container save (#859): the
//! merge into the file on disk, the library commit and the version
//! cascade. Runs on a worker, never on the UI thread.

use super::cascade::{self, OtherLibrary, SaveLibrary, SavedKind, same_library_file};
use super::container_save::{self, ContainerWrite, SaveOptions};
use crate::persist::{PrimitiveWritten, WrittenContainer};
use signex_library::{FootprintFile, LibraryError, SymbolFile};
use uuid::Uuid;

/// A container as the editor held it when the save began.
pub(super) enum Snapshot {
    Symbol {
        file: SymbolFile,
        baseline: SymbolFile,
        name: String,
        owed: Vec<Uuid>,
    },
    Footprint {
        file: FootprintFile,
        baseline: FootprintFile,
        name: String,
        owed: Vec<Uuid>,
    },
}

/// One blocking container write. Run it off the UI thread.
pub(super) struct PrimitiveWrite {
    pub path: std::path::PathBuf,
    pub mode: ContainerWrite,
    /// The `.snxlib` of the open library whose directory holds `path`.
    pub mounted_library: Option<std::path::PathBuf>,
    /// `path` spelled under the mounted library's folder, set when the raw
    /// prefix does not hold. Used only if it names the same directory.
    pub mounted_alias: Option<std::path::PathBuf>,
    pub now: chrono::DateTime<chrono::Utc>,
    pub snapshot: Snapshot,
    /// This write's place in the owning library's write order (#893): the
    /// worker waits for it, so the write neither overtakes nor is
    /// overtaken by the library's row writes.
    pub turn: Option<crate::library::writes::Turn>,
    /// The other open libraries, each with its place in its write order: a
    /// row of one of them may use a primitive of the library saved in, so the
    /// cascade moves those rows too (#2004).
    pub others: Vec<(std::path::PathBuf, crate::library::writes::Turn)>,
}

/// What both container kinds' writes share.
struct WriteContext<'a> {
    library: &'a SaveLibrary,
    /// The `.snxlib` `library` is, reported back as where the write went.
    through: Option<std::path::PathBuf>,
    path: &'a std::path::Path,
    now: chrono::DateTime<chrono::Utc>,
    options: SaveOptions,
    /// The ledger of this app's writes to the owning library, when the
    /// write holds a place in its order.
    own: Option<&'a crate::library::writes::OwnRevisions>,
    others: Vec<OtherLibrary<'a>>,
}

impl PrimitiveWrite {
    /// The file to write and the mounted library to write it through. A
    /// draft held under another spelling of the library's folder is written
    /// at that spelling only when both parents are the same directory on
    /// disk; a case variant on a case-sensitive volume, or `..` after a
    /// symlink, is another folder, and the draft keeps its own file and the
    /// library found from it.
    fn route(&self) -> (std::path::PathBuf, Option<std::path::PathBuf>) {
        let Some(library) = self.mounted_library.clone() else {
            return (self.path.clone(), None);
        };
        let Some(alias) = &self.mounted_alias else {
            return (self.path.clone(), Some(library));
        };
        let same_dir = match (
            self.path.parent().map(std::fs::canonicalize),
            alias.parent().map(std::fs::canonicalize),
        ) {
            (Some(Ok(own)), Some(Ok(mounted))) => own == mounted,
            _ => false,
        };
        if same_dir {
            (alias.clone(), Some(library))
        } else {
            (self.path.clone(), None)
        }
    }

    pub(super) fn run(self) -> Result<PrimitiveWritten, String> {
        if let Some(turn) = &self.turn {
            turn.wait();
        }
        self.write(&|turn| {
            turn.wait();
            true
        })
    }

    /// [`Self::run`] on the UI thread: gives up when the turn does not come
    /// within `limit`, writing nothing.
    pub(super) fn run_within(self, limit: std::time::Duration) -> Result<PrimitiveWritten, String> {
        // One deadline for every turn this save waits on, not `limit` each.
        let deadline = std::time::Instant::now() + limit;
        let left = move || deadline.saturating_duration_since(std::time::Instant::now());
        if let Some(turn) = &self.turn
            && !turn.wait_within(left())
        {
            return Err(format!(
                "{}: the library is still busy reading; the save was not made, try again",
                self.path.display()
            ));
        }
        self.write(&|turn| turn.wait_within(left()))
    }

    /// `wait` takes an other library's place in its write order: `false` when
    /// it did not come in time.
    fn write(
        self,
        wait: &dyn Fn(&crate::library::writes::Turn) -> bool,
    ) -> Result<PrimitiveWritten, String> {
        let (path, mounted) = self.route();
        let library = SaveLibrary::resolve(&path, mounted.as_deref());
        let through = library.root();
        // Every place in a write order this save holds. The library the
        // write goes through may not be the one the UI thread picked (a
        // nested library that is not mounted, or an alias the worker
        // rejected): its own ledger is the one the cascade records in, and
        // the one picked becomes an other library for the cascade (#1972).
        let mut held: Vec<(std::path::PathBuf, crate::library::writes::Turn)> = self
            .mounted_library
            .into_iter()
            .zip(self.turn)
            .chain(self.others)
            .collect();
        let own_turn = through.as_deref().and_then(|root| {
            let at = held
                .iter()
                .position(|(held_root, _)| same_library_file(held_root, root))?;
            Some(held.remove(at).1)
        });
        if let Some(turn) = &own_turn
            && !wait(turn)
        {
            return Err(format!(
                "{}: the library is still busy reading; the save was not made, try again",
                path.display()
            ));
        }
        let context = WriteContext {
            library: &library,
            through: through.clone(),
            path: &path,
            now: self.now,
            options: SaveOptions {
                mode: self.mode,
                bump_versions: library.bumps_versions(self.mode),
            },
            own: own_turn.as_ref().map(|turn| turn.own_revisions()),
            others: held
                .iter()
                .map(|(root, turn)| OtherLibrary {
                    root,
                    own: Some(turn.own_revisions()),
                    ready: Box::new(move || wait(turn)),
                })
                .collect(),
        };
        let written = match self.snapshot {
            Snapshot::Symbol {
                file,
                baseline,
                name,
                owed,
            } => write_symbol(&context, file, &baseline, &name, &owed),
            Snapshot::Footprint {
                file,
                baseline,
                name,
                owed,
            } => write_footprint(&context, file, &baseline, &name, &owed),
        };
        library.report_held_back();
        // The reason leads: the Messages panel cuts long lines.
        written.map_err(|e| format!("{e} (writing {})", path.display()))
    }
}

fn write_symbol(
    context: &WriteContext<'_>,
    mut file: SymbolFile,
    baseline: &SymbolFile,
    name: &str,
    owed: &[Uuid],
) -> Result<PrimitiveWritten, LibraryError> {
    let save = container_save::save_symbol_container(
        context.library,
        context.path,
        &file,
        baseline,
        context.now,
        context.options,
        &format!("save symbol {name}"),
    )?;
    let name_in = |container: &SymbolFile, uuid: Uuid| {
        container.get_symbol(uuid).map(|symbol| symbol.name.clone())
    };
    report_overwritten(
        context.path,
        &save.merge.overwritten,
        |uuid| name_in(&file, uuid),
        |uuid| name_in(baseline, uuid),
    );
    // Carry the versions the save bumped into the copy, then move the rows
    // that use them onto the new version (#832), after the library lock
    // was released.
    cascade::apply_symbol_versions(&mut file, &save.versions);
    let outcome = cascade::cascade_after_container_save(
        context.library,
        context.options.mode,
        SavedKind::Symbol,
        &cascade::saved_symbols(&file, &with_owed(&save.merge.changed, owed)),
        context.own,
        &context.others,
    );
    Ok(PrimitiveWritten {
        file: WrittenContainer::Symbol(file),
        versions: save.versions,
        commit_message: format!("Save symbol {name}"),
        library: context.through.clone(),
        cascaded_rows: outcome.bumped,
        cascade_owed: outcome.owed,
        cascaded_elsewhere: outcome.bumped_elsewhere,
    })
}

fn write_footprint(
    context: &WriteContext<'_>,
    mut file: FootprintFile,
    baseline: &FootprintFile,
    name: &str,
    owed: &[Uuid],
) -> Result<PrimitiveWritten, LibraryError> {
    let save = container_save::save_footprint_container(
        context.library,
        context.path,
        &file,
        baseline,
        context.now,
        context.options,
        &format!("save footprint {name}"),
    )?;
    let name_in = |container: &FootprintFile, uuid: Uuid| {
        container
            .footprints
            .iter()
            .find(|footprint| footprint.uuid == uuid)
            .map(|footprint| footprint.name.clone())
    };
    report_overwritten(
        context.path,
        &save.merge.overwritten,
        |uuid| name_in(&file, uuid),
        |uuid| name_in(baseline, uuid),
    );
    cascade::apply_footprint_versions(&mut file, &save.versions);
    let outcome = cascade::cascade_after_container_save(
        context.library,
        context.options.mode,
        SavedKind::Footprint,
        &cascade::saved_footprints(&file, &with_owed(&save.merge.changed, owed)),
        context.own,
        &context.others,
    );
    Ok(PrimitiveWritten {
        file: WrittenContainer::Footprint(file),
        versions: save.versions,
        commit_message: format!("Save footprint {name}"),
        library: context.through.clone(),
        cascaded_rows: outcome.bumped,
        cascade_owed: outcome.owed,
        cascaded_elsewhere: outcome.bumped_elsewhere,
    })
}

/// The primitives the save changed plus those a failed cascade still owes.
fn with_owed(changed: &[Uuid], owed: &[Uuid]) -> Vec<Uuid> {
    let mut all = changed.to_vec();
    for uuid in owed {
        if !all.contains(uuid) {
            all.push(*uuid);
        }
    }
    all
}

/// Name the primitives whose on-disk copy another writer had changed and
/// this save replaced. One this editor deleted is named from the baseline.
fn report_overwritten(
    path: &std::path::Path,
    overwritten: &[Uuid],
    in_file: impl Fn(Uuid) -> Option<String>,
    in_baseline: impl Fn(Uuid) -> Option<String>,
) {
    let (mut replaced, mut deleted) = (Vec::new(), Vec::new());
    for &uuid in overwritten {
        match in_file(uuid) {
            Some(name) => replaced.push(name),
            None => deleted.extend(in_baseline(uuid)),
        }
    }
    container_save::report_overwritten(path, &replaced, &deleted);
}
