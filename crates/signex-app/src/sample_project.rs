//! The bundled sample project behind Help ▸ Open Sample Project.
//!
//! The design is embedded in the binary and unpacked into the per-user
//! config directory on first use, so the copy the tester opens is
//! writable and survives later launches. A copy that already exists is
//! never overwritten: the tester's edits win.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const PROJECT_FILE: &str = "linear-regulator.snxprj";
const SHEET_FILE: &str = "linear-regulator.snxsch";

const PROJECT_SOURCE: &str = include_str!("../assets/sample-project/linear-regulator.snxprj");
const SHEET_SOURCE: &str = include_str!("../assets/sample-project/linear-regulator.snxsch");

/// `<config_dir>/signex/samples/linear-regulator/`, or `None` when the
/// platform offers no config directory.
pub fn sample_root() -> Option<PathBuf> {
    crate::config_root::config_root().map(|root| root.join("samples").join("linear-regulator"))
}

/// The sample's project file inside `root`.
pub fn project_path(root: &Path) -> PathBuf {
    root.join(PROJECT_FILE)
}

/// The sample's schematic sheet inside `root`.
pub fn sheet_path(root: &Path) -> PathBuf {
    root.join(SHEET_FILE)
}

/// Make sure the sample files exist under `root`, writing only the ones
/// that are missing. Returns the `(project, sheet)` paths.
pub fn ensure_extracted(root: &Path) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("create sample directory {}", root.display()))?;
    let project = project_path(root);
    let sheet = sheet_path(root);
    for (path, source) in [(&project, PROJECT_SOURCE), (&sheet, SHEET_SOURCE)] {
        // An unreadable path must not read as "missing": atomic_write
        // would rename over the tester's copy.
        let exists = path
            .try_exists()
            .with_context(|| format!("check sample file {}", path.display()))?;
        if !exists {
            signex_types::atomic_io::atomic_write(path, source.as_bytes())
                .with_context(|| format!("write sample file {}", path.display()))?;
        }
    }
    Ok((project, sheet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::format::SnxSchematic;

    #[test]
    fn sample_sheet_round_trips() {
        let first = SnxSchematic::parse(SHEET_SOURCE).expect("sample parses");
        let written = first.write_string().expect("sample serialises");
        // The asset must be exactly what the writer emits: a field dropped
        // on parse would otherwise survive a parse -> write -> parse check.
        assert_eq!(written, SHEET_SOURCE, "asset must be writer-canonical");
        let second = SnxSchematic::parse(&written).expect("rewritten sample parses");
        assert_eq!(written, second.write_string().expect("second serialise"));
    }

    #[test]
    fn sample_sheet_is_self_contained_and_erc_clean() {
        let sheet = SnxSchematic::parse(SHEET_SOURCE)
            .expect("sample parses")
            .sheet;
        for symbol in &sheet.symbols {
            assert!(
                sheet.lib_symbols.contains_key(&symbol.lib_id),
                "{} needs its lib symbol embedded so no library has to be mounted",
                symbol.lib_id
            );
        }
        // Same entry point as F8 (`handle_run_erc`): a one-sheet project graph,
        // the sheet's own resolution submap, then the severity overrides.
        let path = PathBuf::from("linear-regulator.snxsch");
        let sheets = std::collections::HashMap::from([(path.clone(), sheet.clone())]);
        let graph = crate::app::project_sheets::project_graph(&sheets, None, None);
        let key = crate::app::project_sheets::sheet_key(&path, None);
        let violations = signex_erc::apply_overrides(
            signex_erc::run_with_project(&sheet, Some(&key), &graph.resolved, &graph.sheets),
            &std::collections::HashMap::new(),
        );
        assert!(
            violations.is_empty(),
            "sample must be ERC-clean: {violations:?}"
        );
    }

    #[test]
    fn extraction_writes_missing_files_and_keeps_existing_edits() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("samples").join("linear-regulator");
        let (project, sheet) = ensure_extracted(&root).expect("first extract");
        assert!(project.exists() && sheet.exists());

        std::fs::write(&sheet, "edited").expect("edit");
        ensure_extracted(&root).expect("second extract");
        assert_eq!(std::fs::read_to_string(&sheet).expect("read"), "edited");
    }
}
