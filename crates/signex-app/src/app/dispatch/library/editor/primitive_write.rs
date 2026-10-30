//! The blocking half of a symbol / footprint container save (#859): the
//! merge into the file on disk, the library commit and the version
//! cascade. Runs on a worker, never on the UI thread.

use super::cascade::{self, SaveLibrary, SavedKind};
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
    pub now: chrono::DateTime<chrono::Utc>,
    pub snapshot: Snapshot,
    /// This write's place in the owning library's write order (#893): the
    /// worker waits for it, so the write neither overtakes nor is
    /// overtaken by the library's row writes.
    pub turn: Option<crate::library::writes::Turn>,
}

/// What both container kinds' writes share.
struct WriteContext<'a> {
    library: &'a SaveLibrary,
    path: &'a std::path::Path,
    now: chrono::DateTime<chrono::Utc>,
    options: SaveOptions,
}

impl PrimitiveWrite {
    pub(super) fn run(self) -> Result<PrimitiveWritten, String> {
        if let Some(turn) = &self.turn {
            turn.wait();
        }
        let library = SaveLibrary::resolve(&self.path, self.mounted_library.as_deref());
        let context = WriteContext {
            library: &library,
            path: &self.path,
            now: self.now,
            options: SaveOptions {
                mode: self.mode,
                bump_versions: library.bumps_versions(self.mode),
            },
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
        written.map_err(|e| format!("{e} (writing {})", self.path.display()))
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
    );
    Ok(PrimitiveWritten {
        file: WrittenContainer::Symbol(file),
        versions: save.versions,
        commit_message: format!("Save symbol {name}"),
        cascaded_rows: outcome.bumped,
        cascade_owed: outcome.owed,
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
    );
    Ok(PrimitiveWritten {
        file: WrittenContainer::Footprint(file),
        versions: save.versions,
        commit_message: format!("Save footprint {name}"),
        cascaded_rows: outcome.bumped,
        cascade_owed: outcome.owed,
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
