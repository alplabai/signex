/// One loaded project in the multi-project workspace. `path` is the
/// canonical identity (`.snxprj` / `.snxprj` location on disk); `data`
/// is the parsed project contents. Multiple projects with different
/// `path`s coexist in `DocumentState.projects`; two identical `path`s
/// at once is a loader bug (existing `open_project_file` de-dupes).
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub id: super::ProjectId,
    pub path: std::path::PathBuf,
    pub data: signex_types::project::ProjectData,
}
