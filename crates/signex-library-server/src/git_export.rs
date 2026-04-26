//! Git-export helper.
//!
//! `signex library export --git <out>` materialises the database into a
//! local-git library directory. Layout matches LIBRARY_PLAN §6: one folder per
//! component UUID, one `.snxpart` per revision, plus a top-level
//! `manifest.toml` declaring the library mode as `LocalGit`.

use std::path::Path;

use signex_library::manifest::{LibraryMeta, LibraryMode, Manifest, UsersConfig, WorkflowConfig};
use signex_library::snxpart::{SCHEMA_VERSION, SnxPartFile, snxpart_filename, write_snxpart};
use uuid::Uuid;

use crate::db::AppState;

pub async fn export_to_dir(state: &AppState, out: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(out)?;
    write_manifest(out)?;

    let summaries = state
        .list_components()
        .await
        .map_err(|e| std::io::Error::other(format!("db: {e}")))?;
    for s in summaries {
        let comp = state
            .fetch_component(s.uuid)
            .await
            .map_err(|e| std::io::Error::other(format!("db: {e}")))?;
        let Some(comp) = comp else { continue };

        let comp_dir = out.join(comp.uuid.to_string());
        std::fs::create_dir_all(&comp_dir)?;

        // Refactored .snxprt is one file per component (revisions live inside
        // the embedded `Component`), so the per-revision loop collapses.
        let file = SnxPartFile {
            schema_version: SCHEMA_VERSION,
            component: comp.clone(),
        };
        let part_path = comp_dir.join(snxpart_filename(comp.uuid));
        write_snxpart(&part_path, &file)
            .map_err(|e| std::io::Error::other(format!("snxpart: {e}")))?;
    }
    Ok(())
}

fn write_manifest(out: &Path) -> std::io::Result<()> {
    let manifest = Manifest {
        library: LibraryMeta {
            name: "exported-library".into(),
            library_id: Uuid::now_v7(),
            description: Some("Exported from Signex library server".into()),
        },
        mode: LibraryMode::LocalGit,
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
    };
    let text = manifest
        .write()
        .map_err(|e| std::io::Error::other(format!("toml: {e}")))?;
    std::fs::write(out.join("manifest.toml"), text)?;
    Ok(())
}
