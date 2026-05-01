//! Signex component library subsystem (v0.9-refactor-2 — DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md`, components are **rows in TSV/JSONB
//! tables**, not files. Each row references reusable primitives (`Symbol`,
//! `Footprint`, `SimModel`) by `(library_id, uuid)` tuples ([`PrimitiveRef`])
//! instead of embedding their geometry. Symbols / footprints / sims stay as
//! standalone editable primitive files; the row binds them with metadata.

pub mod adapter;
pub mod adapters;
#[cfg(feature = "ai-stub")]
pub mod ai_stub;
pub mod cascade;
pub mod component;
pub mod diff;
pub mod distributor;
#[cfg(feature = "distributors-community")]
pub mod distributors;
pub mod hash;
pub mod identity;
pub mod library_file;
pub mod lifecycle;
pub mod manifest;
pub mod manufacturer;
pub mod param;
pub mod primitive;
pub mod search;
#[cfg(feature = "search-tantivy")]
pub mod search_index;
pub mod tables;
pub mod templates;
pub mod where_used;

pub use adapter::{
    ComponentSummary, FieldSet, HistoryEntry, LibraryAdapter, LibraryError, LibraryQuery,
    PrimitiveSummary,
};
pub use adapters::library_set::LibrarySet;
#[cfg(feature = "ai-stub")]
pub use ai_stub::{PinGuess, PinoutGuess, extract_pinout};
pub use cascade::{
    CascadeReport, cascade_after_footprint_save, cascade_after_sim_save,
    cascade_after_symbol_save, patch_bump,
};
pub use component::{ComponentRow, DatasheetRef, PinPadOverride, PlmReserved};
pub use diff::{
    BumpKind, LifecycleDiff, ListDiff, ParameterDiff, PinMapDiff, RowDiff, auto_bump_kind,
    diff_rows,
};
pub use distributor::{DistributorAdapter, DistributorError, DistributorPart, DistributorSource};
#[cfg(feature = "distributors-community")]
pub use distributors::{
    DigiKeyAdapter, DistributorCache, JlcpcbAdapter, KeyringStore, LcscAdapter, MouserAdapter,
};
pub use hash::hash_row_content;
pub use identity::{ComponentClass, InternalPn, Mpn, RowId};
pub use library_file::{
    ClassEntry, FORMAT_TOKEN, LibraryFile, LibraryFileError, LibraryRow, LibrarySection,
    LibraryTable, SnxlibManifest,
};
pub use lifecycle::LifecycleState;
pub use manifest::{
    LibraryMeta, LibraryMode, Manifest, TableConfig, UserEntry, UsersConfig, WorkflowConfig,
    WorkflowMode,
};
pub use manufacturer::{AlternateStatus, DistributorListing, ManufacturerPart};
pub use param::{ParamMap, ParamValue};
pub use primitive::{
    Body3D, BodyShape, ComponentType, Drill, Footprint, FpGraphic, FpGraphicKind, LayerId, Pad,
    PadKind, PadShape, PinDirection, PinOrientation, PinSymbolKind, Polygon, PrimitiveKind,
    PrimitiveRef, SimKind, SimModel, StepAttachment, Symbol, SymbolFile, SymbolGraphic,
    SymbolGraphicKind, SymbolPin,
};
pub use search::{Facet, FacetOp, SearchIndex, SearchQuery};
#[cfg(feature = "search-tantivy")]
pub use search_index::{TantivyIndexError, TantivySearchIndex};
pub use tables::{
    TABLE_HEADER, TableSchema, append_row, delete_row, read_table, update_row, write_table,
};
pub use templates::{ParamKind, ParamSlot, ParameterTemplate, TemplateRegistry, TemplateViolation};
pub use where_used::{UseSite, WhereUsedIndex};

#[cfg(feature = "local-git")]
pub use adapters::local_git::LocalGitAdapter;

/// Project-level "Enable Version Control" helper. Runs
/// `git2::Repository::init` at `project_dir`, optionally writes a
/// `.gitattributes` opting common binary-model extensions (`*.step`,
/// `*.stp`, `*.wrl`, `*.iges`) into Git LFS, then stages every
/// tracked file and creates the initial commit "chore: enable
/// version control".
///
/// Used by `signex-app` so the per-project Enable Version Control
/// flow doesn't need to pull `git2` in directly. Errors propagate
/// through the existing [`adapter::LibraryError`] variants so the UI
/// can surface them in one place.
#[cfg(feature = "local-git")]
pub fn enable_project_version_control(
    project_dir: &std::path::Path,
    use_lfs: bool,
) -> Result<(), adapter::LibraryError> {
    use adapter::LibraryError;

    if project_dir.join(".git").exists() {
        return Err(LibraryError::Conflict(format!(
            "{} already has a .git directory",
            project_dir.display()
        )));
    }
    std::fs::create_dir_all(project_dir).map_err(|e| {
        LibraryError::Backend(format!("create project dir {}: {e}", project_dir.display()))
    })?;

    let mut wrote_lfs_attributes = false;
    if use_lfs {
        let attributes = "\
*.step filter=lfs diff=lfs merge=lfs -text\n\
*.stp filter=lfs diff=lfs merge=lfs -text\n\
*.wrl filter=lfs diff=lfs merge=lfs -text\n\
*.iges filter=lfs diff=lfs merge=lfs -text\n";
        std::fs::write(project_dir.join(".gitattributes"), attributes).map_err(|e| {
            LibraryError::Backend(format!("write .gitattributes: {e}"))
        })?;
        wrote_lfs_attributes = true;
    }

    // Helper that removes the orphan `.gitattributes` if subsequent
    // git ops fail — we don't want a non-version-controlled
    // directory to keep an LFS-tracking file that has no `.git/` to
    // give it meaning.
    let cleanup = |dir: &std::path::Path, wrote: bool| {
        if wrote {
            let _ = std::fs::remove_file(dir.join(".gitattributes"));
        }
    };

    let repo = git2::Repository::init(project_dir).map_err(|e| {
        cleanup(project_dir, wrote_lfs_attributes);
        LibraryError::Backend(format!("git init: {e}"))
    })?;

    // Identity for the commit — same fallback the LocalGitAdapter
    // uses (env GIT_AUTHOR_NAME / EMAIL → repo config → "signex").
    let cfg = repo.config().ok();
    let sig_name = std::env::var("GIT_AUTHOR_NAME")
        .ok()
        .or_else(|| {
            cfg.as_ref()
                .and_then(|c| c.get_string("user.name").ok())
        })
        .unwrap_or_else(|| "signex".to_string());
    let sig_email = std::env::var("GIT_AUTHOR_EMAIL")
        .ok()
        .or_else(|| {
            cfg.as_ref()
                .and_then(|c| c.get_string("user.email").ok())
        })
        .unwrap_or_else(|| "signex@localhost".to_string());
    let sig = git2::Signature::now(&sig_name, &sig_email)
        .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

    let mut index = repo
        .index()
        .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
    index
        .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| LibraryError::Backend(format!("git add .: {e}")))?;
    index
        .write()
        .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "chore: enable version control",
        &tree,
        &[],
    )
    .map_err(|e| LibraryError::Backend(format!("git initial commit: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_compiles() {
        // Smoke test: ensures the crate builds with all module declarations.
    }
}
