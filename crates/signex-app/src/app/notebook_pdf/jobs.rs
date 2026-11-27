//! The off-thread work of the notebook PDF export: reading the notebook
//! files, inspecting them for the dialog and making the PDF. The design the
//! stale check reads (its designators, pins and nets) and the part values
//! the Python cells read come from the named project itself, read on the
//! worker from what `update` captured (the open sheets, with their unsaved
//! edits), never from what another panel happens to hold.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_notebook::attachment::DesignValues;
use signex_notebook::cell::{CELL_CACHE_PATH, CellCache, NoRuntime, SandboxPolicy, Trust};
use signex_notebook::export::{
    DesignNames, ExportCells, NotebookInspection, OUTPUT_FOLDERS, StaleRule, TitleBlock,
    default_file_name, export_pdf_running_cells, inspect,
};
use signex_notebook::{LoadProblem, NOTEBOOK_DIR, NotebookSources};
use signex_types::project::{BASE_VARIANT, is_base_variant};

use super::{BuildRequest, BuiltPdf, NotebookPdfMsg};
use crate::app::notebook_measurements::DesignInputs;
use crate::app::notebook_project::read_notebook_file;
use crate::app::project_sheets::open_sheets_snapshot;
use crate::app::{LoadedProject, Message, ProjectId, Signex};

impl Signex {
    /// What the design of `project` is read from on the worker: its sheets,
    /// the open ones with their unsaved edits. `None` for a project with no
    /// sheet.
    pub(super) fn notebook_design_inputs(&self, project: ProjectId) -> Option<DesignInputs> {
        let loaded = self.document_state.project_by_id(project)?;
        DesignInputs::of(loaded, open_sheets_snapshot(&self.document_state))
    }
}

/// Blocking: the design's names and part values, `None` names when the
/// design could not be read whole.
fn design_of(
    design: Option<&DesignInputs>,
    project_dir: &Path,
) -> (Option<DesignNames>, DesignValues) {
    match design {
        Some(design) => (design.names(project_dir), design.values()),
        None => (None, DesignValues::new()),
    }
}

/// Base, then the project's own variants in their order.
pub(super) fn variants_of(project: &LoadedProject) -> Vec<String> {
    let mut variants = vec![BASE_VARIANT.to_string()];
    for name in &project.data.variant_definitions {
        if !is_base_variant(name) && !variants.contains(name) {
            variants.push(name.clone());
        }
    }
    variants
}

/// The id the PDF is made with: the project's name and the notebook path,
/// so the same notebook gives the same file in any folder.
pub(super) fn pdf_ident(project_name: &str, notebook: &str) -> String {
    format!("{project_name}/{NOTEBOOK_DIR}/{notebook}")
}

/// The default file, relative to the project folder:
/// `Project Outputs/notebook/<project>-<document>-rev<R>.pdf`.
pub(super) fn default_path(project_name: &str, notebook: &str, title: &TitleBlock) -> PathBuf {
    let mut path: PathBuf = OUTPUT_FOLDERS.iter().collect();
    path.push(default_file_name(project_name, notebook, title));
    path
}

/// The Save to text as a file: relative to the project folder unless it is
/// absolute; `None` when it is empty.
pub(super) fn resolve_path(project_dir: &Path, text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let path = PathBuf::from(text);
    Some(if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    })
}

/// Read the notebook file `notebook` of the project in `project_dir`, the
/// chapters it includes and the attachments they name, so the pages print
/// each figure and plot (#3021). Blocking: runs on a worker. A chapter that
/// cannot be read is left to the compile, which names it where it is
/// included; the notebook file itself must read. With `confine` (a
/// headless or agent run) only regular files inside the project are read,
/// through no link out of it (#3020).
pub(super) fn read_sources(
    project_dir: &Path,
    notebook: &str,
    confine: Option<&Path>,
) -> Result<NotebookSources, String> {
    let folder = project_dir.join(NOTEBOOK_DIR);
    let (mut sources, problems) = NotebookSources::load(notebook, |path| {
        read_notebook_file(confine, &folder.join(path))
    });
    if let Some(problem) = problems.iter().find(|problem| problem.file.is_empty()) {
        return Err(main_problem(notebook, problem));
    }
    for path in sources.missing_attachments() {
        if let Ok(bytes) = read_notebook_file(confine, &folder.join(&path)) {
            let _held = sources.insert(&path, bytes);
        }
    }
    Ok(sources)
}

fn main_problem(notebook: &str, problem: &LoadProblem) -> String {
    match &problem.kind {
        signex_notebook::LoadProblemKind::Unreadable { reason } => {
            format!("{NOTEBOOK_DIR}/{notebook}: {reason}")
        }
        signex_notebook::LoadProblemKind::OutsideNotebook => {
            format!("{notebook} is not a path inside {NOTEBOOK_DIR}/")
        }
    }
}

/// Read and compile the notebook for the dialog off the update thread.
pub(super) fn inspect_task(
    generation: u64,
    project_dir: PathBuf,
    notebook: String,
    design: Option<DesignInputs>,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || -> Result<NotebookInspection, String> {
                let sources = read_sources(&project_dir, &notebook, None)?;
                let names = design
                    .as_ref()
                    .and_then(|design| design.names(&project_dir))
                    .unwrap_or_default();
                Ok(inspect(&sources, |r| names.has(r)))
            })
            .await
            .unwrap_or_else(|e| Err(format!("the notebook read did not finish: {e}")))
        },
        move |result| {
            Message::from(NotebookPdfMsg::Inspected {
                generation,
                result: Arc::new(result),
            })
        },
    )
}

/// Read the notebook, make its PDF and make the folder it goes in, off the
/// update thread; the file itself is written by the export write path.
pub(super) fn build_task(request: BuildRequest, design: Option<DesignInputs>) -> Task<Message> {
    let job = request.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || build(&job, design.as_ref()))
                .await
                .unwrap_or_else(|e| Err(format!("the export did not finish: {e}")))
        },
        move |result| {
            Message::from(NotebookPdfMsg::Built {
                request: Box::new(request),
                result: Arc::new(result),
            })
        },
    )
}

/// Blocking: the project's cell results; none when the file is missing or
/// this build cannot read it (it is a cache).
fn read_cell_cache(project_dir: &Path) -> CellCache {
    std::fs::read(project_dir.join(CELL_CACHE_PATH))
        .map(|bytes| CellCache::read(&bytes).0)
        .unwrap_or_default()
}

/// Blocking: read, run the Python cells, export, and make the output
/// folder. The cells run as an export runs them (mockup row 200): this
/// build has no Python runtime, so each prints its cached result greyed
/// with the slim-install note, or none.
fn build(request: &BuildRequest, design: Option<&DesignInputs>) -> Result<BuiltPdf, String> {
    let confine = request.confined.then_some(request.project_dir.as_path());
    let sources = read_sources(&request.project_dir, &request.notebook, confine)?;
    let (names, values) = design_of(design, &request.project_dir);
    // Under Fail a design that could not be read whole checks nothing, so
    // the export does not pass it silently.
    let names = match names {
        Some(names) => names,
        None if request.options.stale == StaleRule::Fail => {
            return Err(
                "the project's design could not be read whole, so its references cannot be \
                 checked for stale=fail"
                    .to_string(),
            );
        }
        None => DesignNames::default(),
    };
    let values = &values;
    let ident = pdf_ident(&request.project_name, &request.notebook);
    let mut cache = read_cell_cache(&request.project_dir);
    let policy = SandboxPolicy::new(&request.project_dir.to_string_lossy());
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string();
    let cells = ExportCells {
        design: values,
        runner: &NoRuntime,
        policy: &policy,
        trust: Trust::Unasked,
        now: &now,
        cache: &mut cache,
    };
    let (export, _report) =
        export_pdf_running_cells(&sources, &request.options, &ident, |r| names.has(r), cells)
            .map_err(|e| e.to_string())?;
    let path = match &request.path {
        Some(path) => path.clone(),
        None => request.project_dir.join(default_path(
            &request.project_name,
            &request.notebook,
            &export.summary.title,
        )),
    };
    // A command's default file is checked here, once the title block named
    // it; a path it named was checked in `update`.
    if request.path.is_none() && request.overwrite == Some(false) && path.exists() {
        return Err(format!(
            "{} exists; pass overwrite=true to replace it",
            path.display()
        ));
    }
    // A headless or agent run writes its default file only inside the
    // project, through no link (#3020); a named `path` is the caller's
    // allowed-path policy's to check.
    if request.path.is_none() {
        crate::app::notebook_project::confined_write(confine, &path)?;
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|e| format!("{e}: could not make the folder {}", folder.display()))?;
    }
    Ok(BuiltPdf { path, export })
}

#[cfg(test)]
mod tests {
    use super::read_sources;

    /// #3021: the PDF read loads the attachments the notebook names, so
    /// each figure prints rather than "Missing attachment".
    #[test]
    fn the_pdf_read_loads_the_attachments() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("notebook");
        std::fs::create_dir_all(folder.join("attachments/scope")).unwrap();
        std::fs::write(
            folder.join("design.snxnbk"),
            "#import \"@signex/notebook:0.1.0\": *\n#attachment(\"scope/a.csv\")\n",
        )
        .unwrap();
        std::fs::write(folder.join("attachments/scope/a.csv"), "t,v\n0,1\n").unwrap();
        let sources = read_sources(dir.path(), "design.snxnbk", None).expect("read");
        assert!(sources.missing_attachments().is_empty());
        assert!(sources.get("attachments/scope/a.csv").is_some());
    }

    /// #3020: a headless or agent export reads no chapter linked out of
    /// the project; a person's export may.
    #[cfg(unix)]
    #[test]
    fn a_confined_read_follows_no_link_out_of_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("k.snxnbk"), "SECRET\n").unwrap();
        let folder = dir.path().join("notebook");
        std::fs::create_dir_all(folder.join("chapters")).unwrap();
        std::fs::write(
            folder.join("design.snxnbk"),
            "= A\n#include \"chapters/k.snxnbk\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("k.snxnbk"),
            folder.join("chapters/k.snxnbk"),
        )
        .unwrap();
        let confined = read_sources(dir.path(), "design.snxnbk", Some(dir.path())).expect("read");
        assert!(confined.get("chapters/k.snxnbk").is_none(), "not read");
        let person = read_sources(dir.path(), "design.snxnbk", None).expect("read");
        assert!(person.get("chapters/k.snxnbk").is_some());
    }

    /// #3020: a headless export writes its default file only inside the
    /// project, through no link.
    #[cfg(unix)]
    #[test]
    fn a_confined_write_refuses_a_link_out_of_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        let target = dir.path().join("out/notebook.pdf");
        let confine = Some(dir.path());
        assert!(crate::app::notebook_project::confined_write(confine, &target).is_err());
        let inside = dir.path().join("exports/notebook.pdf");
        assert!(crate::app::notebook_project::confined_write(confine, &inside).is_ok());
        assert!(crate::app::notebook_project::confined_write(None, &target).is_ok());
    }
}
