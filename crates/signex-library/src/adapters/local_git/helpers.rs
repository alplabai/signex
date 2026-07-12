//! Free helper functions for the local-git adapter.

use super::*;

// ── Free helpers ───────────────────────────────────────────────────────────

/// Project a `git2::Commit` onto the trait-level [`HistoryEntry`].
///
/// Diff-stat fields (`additions`, `deletions`, `files_changed`) stay
/// at the scaffold defaults — Stage 17 ships the list shape without
/// the lazy diff plumbing. The author timestamp is preferred over
/// the committer's so rebases/cherry-picks don't visually skew the
/// "12 minutes ago" labels.
pub(super) fn commit_to_history_entry(commit: &git2::Commit<'_>) -> HistoryEntry {
    let author = commit.author();
    let secs = author.when().seconds();
    let time =
        chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0).unwrap_or_else(chrono::Utc::now);
    let raw = commit.message().unwrap_or("");
    let (subject, body) = match raw.find("\n\n") {
        Some(i) => (raw[..i].trim_end().to_string(), raw[i + 2..].to_string()),
        None => (raw.trim_end().to_string(), String::new()),
    };
    HistoryEntry {
        sha: commit.id().to_string(),
        author_name: author.name().unwrap_or_default().to_string(),
        author_email: author.email().unwrap_or_default().to_string(),
        time,
        subject,
        body,
        parent_shas: commit.parent_ids().map(|id| id.to_string()).collect(),
        files_changed: Vec::new(),
        additions: 0,
        deletions: 0,
    }
}

/// True if `commit` modified `rel_path` relative to *any* of its
/// parents (or, for the root commit, if the path exists in its
/// tree). Mirrors the behaviour of `git log -- <path>` for the simple
/// non-rename case the scaffold targets.
pub(super) fn commit_touches_path(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    rel_path: &str,
) -> Result<bool, LibraryError> {
    let new_tree = commit
        .tree()
        .map_err(|e| LibraryError::Backend(format!("git commit tree: {e}")))?;

    if commit.parent_count() == 0 {
        // Root commit: include if the path exists in this tree at all.
        return Ok(new_tree.get_path(Path::new(rel_path)).is_ok());
    }

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(rel_path);

    for parent in commit.parents() {
        let old_tree = parent
            .tree()
            .map_err(|e| LibraryError::Backend(format!("git parent tree: {e}")))?;
        let diff = repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))
            .map_err(|e| LibraryError::Backend(format!("git diff: {e}")))?;
        if diff.deltas().len() > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) const LEGACY_ROW_ID_COL: &str = "row_id";

/// Header expected for the v0.9 fixed-schema `[tables.<name>]` blocks
/// — same column ordering as the pre-refactor `tables/*.tsv` files
/// so the conversion through [`row_to_record`] / [`record_to_row`]
/// stays bit-exact. Stage 12 lifts the fixed-schema constraint.
pub(super) fn legacy_columns() -> Vec<String> {
    TABLE_HEADER.iter().map(|s| (*s).to_string()).collect()
}

/// Verify a `[tables.<name>]` block's column order matches the
/// legacy schema. Mismatches are loud — a hand-edited `.snxlib`
/// where someone reordered or renamed columns would silently
/// miscolumn data otherwise.
pub(super) fn validate_legacy_header(table: &str, columns: &[String]) -> Result<(), LibraryError> {
    if columns.len() != TABLE_HEADER.len() {
        return Err(LibraryError::Backend(format!(
            "table {table:?} schema mismatch: {} columns, expected {}",
            columns.len(),
            TABLE_HEADER.len()
        )));
    }
    for (got, want) in columns.iter().zip(TABLE_HEADER.iter()) {
        if got.as_str() != *want {
            return Err(LibraryError::Backend(format!(
                "table {table:?} schema mismatch: column {got:?}, expected {want:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn component_to_library_row(row: &ComponentRow) -> Result<LibraryRow, LibraryError> {
    let cells = row_to_record(row)?;
    let mut lib_row = LibraryRow::default();
    for (col, val) in TABLE_HEADER.iter().zip(cells) {
        lib_row.cells.insert((*col).to_string(), val);
    }
    Ok(lib_row)
}

pub(super) fn library_row_to_component(row: &LibraryRow) -> Result<ComponentRow, LibraryError> {
    let mut record = csv::StringRecord::new();
    for col in TABLE_HEADER.iter() {
        let val = row.cells.get(*col).map(String::as_str).unwrap_or("");
        record.push_field(val);
    }
    record_to_row(&record)
}

pub(super) fn synthesize_manifest(snx: &SnxlibManifest) -> Manifest {
    Manifest {
        library: LibraryMeta {
            name: snx.library.name.clone(),
            library_id: snx.library_id,
            description: snx.library.description.clone(),
        },
        mode: snx.mode.clone(),
        workflow: snx.workflow.clone(),
        users: snx.users.clone(),
        // The new model stores tables inside `LibraryFile.tables`, not
        // in the manifest header — leave the legacy field empty so
        // `Manifest::table_for_class` falls back to the mechanical
        // plural until Stage 8/12 retires that caller surface.
        tables: Vec::new(),
    }
}

pub(super) fn parent_dir(p: &Path) -> Result<PathBuf, LibraryError> {
    p.parent().map(Path::to_path_buf).ok_or_else(|| {
        LibraryError::Backend(format!(
            "library file {} has no parent directory",
            p.display()
        ))
    })
}

pub(super) fn file_name_str(p: &Path) -> Result<String, LibraryError> {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .ok_or_else(|| {
            LibraryError::Backend(format!(
                "library file path {} has no UTF-8 file name",
                p.display()
            ))
        })
}

pub(super) fn write_lfs_attributes(root_dir: &Path) -> Result<(), LibraryError> {
    let path = root_dir.join(GITATTRIBUTES_FILE);
    let mut text = String::new();
    text.push_str(
        "# Git LFS attributes for Signex 3D model binaries.\n\
         # Written at library-create time when LFS opt-in was selected.\n",
    );
    for ext in LFS_EXTENSIONS {
        text.push_str(&format!("*.{ext} filter=lfs diff=lfs merge=lfs -text\n"));
    }
    fs::write(&path, text)?;
    Ok(())
}

/// Slugify a human-facing name into a safe filename component.
/// Lowercased, ASCII-only, runs of non-alphanumeric chars collapsed to
/// `-`. Empty result falls back to `"untitled"`.
pub(super) fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

pub(super) fn primitive_subdir(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOLS_DIR,
        PrimitiveKind::Footprint => FOOTPRINTS_DIR,
        PrimitiveKind::Sim => SIMS_DIR,
    }
}

pub(super) fn primitive_ext(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOL_EXT,
        PrimitiveKind::Footprint => FOOTPRINT_EXT,
        PrimitiveKind::Sim => SIM_EXT,
    }
}

pub(super) fn primitive_kind_str(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "symbol",
        PrimitiveKind::Footprint => "footprint",
        PrimitiveKind::Sim => "sim",
    }
}

pub(super) fn identity_for_repo(repo: &git2::Repository) -> (String, String) {
    let cfg = repo.config().ok();
    let name = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "Signex Library".to_string());
    let email = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_else(|| "library@signex.local".to_string());
    (name, email)
}
