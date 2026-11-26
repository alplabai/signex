//! The off-thread work of the notebook PDF export: reading the notebook
//! files, inspecting them for the dialog and making the PDF, and the names
//! of the design the stale check reads, captured in `update`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_notebook::export::{
    DesignNames, NotebookInspection, OUTPUT_FOLDERS, TitleBlock, default_file_name, export_pdf,
    inspect,
};
use signex_notebook::{LoadProblem, NOTEBOOK_DIR, NotebookSources};
use signex_types::project::{BASE_VARIANT, is_base_variant};

use super::{BuildRequest, BuiltPdf, NotebookPdfMsg};
use crate::app::{LoadedProject, Message, ProjectId, Signex};

impl Signex {
    /// What the design of `project` has now, for the stale check: the
    /// designators the Annotations scan found (when it is of this project)
    /// and the net names of its current net index. A list that is not
    /// there is not checked.
    pub(super) fn notebook_design_names(&self, project: ProjectId) -> DesignNames {
        let annotations = &self.ui_state.annotations;
        let components = annotations
            .target
            .as_ref()
            .filter(|(id, _)| *id == project)
            .and(annotations.scan.as_ref())
            .and_then(|scan| scan.parts.as_ref())
            .map(|parts| parts.keys().cloned().collect());
        let ds = &self.document_state;
        let nets = crate::app::runtime::net_class_styles::plan_projects(ds)
            .into_iter()
            .find(|planned| planned.project == project)
            .and_then(|planned| ds.net_nav.index.fresh(project, &planned.key))
            .map(|data| {
                let index = &data.index;
                (0..index.net_count())
                    .filter_map(|i| index.net(i).map(|net| net.name.clone()))
                    .collect()
            });
        DesignNames {
            components,
            pins: None,
            nets,
        }
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

/// Read the notebook file `notebook` of the project in `project_dir` and
/// the chapters it includes. Blocking: runs on a worker. A chapter that
/// cannot be read is left to the compile, which names it where it is
/// included; the notebook file itself must read.
pub(super) fn read_sources(project_dir: &Path, notebook: &str) -> Result<NotebookSources, String> {
    let folder = project_dir.join(NOTEBOOK_DIR);
    let (sources, problems) =
        NotebookSources::load(notebook, |path| std::fs::read(folder.join(path)));
    match problems.iter().find(|problem| problem.file.is_empty()) {
        Some(problem) => Err(main_problem(notebook, problem)),
        None => Ok(sources),
    }
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
    names: DesignNames,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || -> Result<NotebookInspection, String> {
                let sources = read_sources(&project_dir, &notebook)?;
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
pub(super) fn build_task(request: BuildRequest, names: DesignNames) -> Task<Message> {
    let job = request.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || build(&job, &names))
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

/// Blocking: read, export, and make the output folder.
fn build(request: &BuildRequest, names: &DesignNames) -> Result<BuiltPdf, String> {
    let sources = read_sources(&request.project_dir, &request.notebook)?;
    let ident = pdf_ident(&request.project_name, &request.notebook);
    let export = export_pdf(&sources, &request.options, &ident, |r| names.has(r))
        .map_err(|e| e.to_string())?;
    let path = match &request.path {
        Some(path) => path.clone(),
        None => request.project_dir.join(default_path(
            &request.project_name,
            &request.notebook,
            &export.summary.title,
        )),
    };
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|e| format!("{e}: could not make the folder {}", folder.display()))?;
    }
    Ok(BuiltPdf { path, export })
}
