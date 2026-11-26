//! The off-thread part of a Generate run: what it needs is captured from the
//! Model in `update` ([`Signex::notebook_generate_job`]); [`run`] then runs
//! the checks or takes the last results, builds the BOM, reads the
//! hand-written notebook, builds the report and, when asked, writes it.

use std::path::{Path, PathBuf};

use signex_notebook::generate::{
    BomFacts, ChecksSource, Fact, ProjectFacts, WriteDecision, generate, generated_path,
    notebook_blocks, revision_of, sheets_of, write_decision,
};
use signex_notebook::{DEFAULT_NOTEBOOK_FILE, NOTEBOOK_DIR, NotebookSources};
use signex_types::check::CheckReport;
use signex_types::pcb::PcbBoard;

use super::{GenerateOutcome, GenerateRequest};
use crate::app::Signex;
use crate::app::handlers::erc::ERC_CHECK;
use crate::app::handlers::{
    BomInputs, DRC_CHECK, REQUIREMENTS_CHECK, project_board_path, project_boms,
};
use crate::app::project_sheets::project_root_sheet_path;
use crate::app::state::LoadedProject;

/// A check run, ready to run off the UI thread.
type Run = Box<dyn FnOnce() -> Result<CheckReport, String> + Send>;

/// Where one check's report comes from.
enum CheckSource {
    /// Run it now, over this file.
    Run(PathBuf, Run),
    /// The newest kept report, or why there is none.
    Kept(Box<Fact<CheckReport>>),
}

/// Everything a run needs, captured from memory.
pub(super) struct GenerateJob {
    request: GenerateRequest,
    facts: ProjectFacts,
    dir: PathBuf,
    /// The folder reads stay inside, for a headless run or an agent.
    confine: Option<PathBuf>,
    erc: CheckSource,
    drc: CheckSource,
    requirements: CheckSource,
    bom: BomInputs,
    board: Option<PcbBoard>,
    /// The design the stale references are counted against.
    design: Option<crate::app::notebook_measurements::DesignInputs>,
}

impl Signex {
    /// The job for `request` on `project`.
    pub(super) fn notebook_generate_job(
        &self,
        request: GenerateRequest,
        project: &LoadedProject,
    ) -> GenerateJob {
        let confined = self.headless_answers().is_some() || self.agent.serving;
        let board_file = project_board_path(project);
        let (erc, drc, requirements) = match request.options.checks {
            ChecksSource::RunNow => self.checks_to_run(project, board_file.as_deref()),
            ChecksSource::LastResults => self.kept_checks(project, board_file.as_deref()),
        };
        let variant = request.variant.clone();
        GenerateJob {
            facts: base_facts(project, variant.clone()),
            dir: project.dir().to_path_buf(),
            confine: confined.then(|| project.dir().to_path_buf()),
            erc,
            drc,
            requirements,
            bom: self.requirements_bom_inputs(project).for_variant(variant),
            board: board_file
                .as_deref()
                .and_then(|path| self.document_state.pcb_board(path))
                .cloned(),
            design: crate::app::notebook_measurements::DesignInputs::of(
                project,
                crate::app::project_sheets::open_sheets_snapshot(&self.document_state),
            ),
            request,
        }
    }

    fn checks_to_run(
        &self,
        project: &LoadedProject,
        board: Option<&Path>,
    ) -> (CheckSource, CheckSource, CheckSource) {
        let erc = match project_root_sheet_path(project) {
            Some(root) => {
                let job = self.erc_check_job(&root, Some(project));
                let run: Run = Box::new(move || Ok(job()));
                CheckSource::Run(project.path.clone(), run)
            }
            None => CheckSource::Kept(Box::new(Fact::not_read("the project has no root sheet"))),
        };
        let drc = match board {
            Some(board) => {
                let run: Run = Box::new(self.drc_check_job(board, Some(project)));
                CheckSource::Run(board.to_path_buf(), run)
            }
            None => CheckSource::Kept(Box::new(Fact::not_read("the project has no board"))),
        };
        let run: Run = Box::new(self.requirements_check_job(project));
        let requirements = CheckSource::Run(project.path.clone(), run);
        (erc, drc, requirements)
    }

    fn kept_checks(
        &self,
        project: &LoadedProject,
        board: Option<&Path>,
    ) -> (CheckSource, CheckSource, CheckSource) {
        let runs = &self.document_state.check_runs;
        let kept = |check: &str, scope: Option<&Path>| {
            let found = scope.and_then(|scope| runs.last_report(check, scope));
            CheckSource::Kept(Box::new(match found {
                Some(report) => {
                    Fact::read(report.clone(), format!("check_{check} run {}", report.run))
                }
                None => Fact::not_read(format!("no {check} run of this project is kept")),
            }))
        };
        (
            kept(ERC_CHECK, Some(project.path.as_path())),
            kept(DRC_CHECK, board),
            kept(REQUIREMENTS_CHECK, Some(project.path.as_path())),
        )
    }
}

/// The facts read straight from the project file; the rest are filled by
/// [`run`].
pub(super) fn base_facts(project: &LoadedProject, variant: Option<String>) -> ProjectFacts {
    let data = &project.data;
    let project_file = project
        .path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    fn not_yet<T>() -> Fact<T> {
        Fact::not_read("not read yet")
    }
    ProjectFacts {
        project_file,
        name: data.name.clone(),
        revision: revision_of(data),
        variant: variant.unwrap_or_else(|| signex_types::project::BASE_VARIANT.to_string()),
        sheets: sheets_of(data),
        power_nets: Vec::new(),
        requirements: data.requirements.clone(),
        blocks: not_yet(),
        requirement_check: not_yet(),
        erc: not_yet(),
        drc: not_yet(),
        bom: not_yet(),
        simulation: Fact::not_read("no simulation run is recorded for the project"),
        outputs: Fact::not_read("no output run is recorded for the project"),
        changes: Fact::not_read("the project version history is not read yet"),
        stale_references: 0,
    }
}

/// Run `job`: blocking, so it runs off the UI thread. The checks it ran,
/// whatever came of the report, and the report or why it was refused.
pub(super) fn run(
    job: GenerateJob,
) -> (Vec<(PathBuf, CheckReport)>, Result<GenerateOutcome, String>) {
    let GenerateJob {
        request,
        mut facts,
        dir,
        confine,
        erc,
        drc,
        requirements,
        bom,
        board,
        design,
    } = job;
    let mut ran = Vec::new();
    facts.erc = take(erc, "ERC", &mut ran);
    facts.drc = take(drc, "DRC", &mut ran);
    facts.requirement_check = take(requirements, "Requirement checks", &mut ran);
    facts.bom = match project_boms(bom, board.as_ref(), confine.as_deref()) {
        Ok(boms) => Fact::read(
            BomFacts {
                lines: boms.grouped.rows.len(),
                parts: boms
                    .grouped
                    .rows
                    .iter()
                    .map(|row| row.fitted_qty as usize)
                    .sum(),
                cost: None,
                unpriced: Vec::new(),
                no_stock: Vec::new(),
            },
            format!("BOM of variant {}", facts.variant),
        ),
        Err(why) => Fact::not_read(why),
    };
    facts.blocks = read_blocks(&dir, confine.as_deref());
    facts.stale_references = stale_references(&dir, confine.as_deref(), design.as_ref());
    let mut notebook = generate(&facts, &request.options);
    if request.save {
        if let Err(why) = write_report(&dir, &notebook.text, request.update, confine.as_deref()) {
            return (ran, Err(why));
        }
        notebook.summary.written = true;
    }
    (
        ran,
        Ok(GenerateOutcome {
            notebook,
            ran: Vec::new(),
        }),
    )
}

/// How many distinct references of the notebook name nothing the design
/// has now, as the PDF export counts them (#3021); 0 when the design could
/// not be read whole.
fn stale_references(
    dir: &Path,
    confine: Option<&Path>,
    design: Option<&crate::app::notebook_measurements::DesignInputs>,
) -> usize {
    let Some(names) = design.and_then(|design| design.names(dir)) else {
        return 0;
    };
    let folder = dir.join(NOTEBOOK_DIR);
    let (sources, _) = NotebookSources::load(DEFAULT_NOTEBOOK_FILE, |path| {
        crate::app::notebook_project::read_notebook_file(confine, &folder.join(path))
    });
    signex_notebook::export::StaleCheck::run(&sources, |r| names.has(r))
        .stale
        .len()
}

/// The report of `source`; one run now is kept in `ran` with its file.
fn take(
    source: CheckSource,
    name: &str,
    ran: &mut Vec<(PathBuf, CheckReport)>,
) -> Fact<CheckReport> {
    match source {
        CheckSource::Kept(fact) => *fact,
        CheckSource::Run(scope, run) => match run() {
            Ok(report) => {
                ran.push((scope, report.clone()));
                Fact::read(report, format!("{name} run for this report"))
            }
            Err(why) => Fact::not_read(why),
        },
    }
}

/// The blocks of `notebook/design.snxnbk` and its chapters.
fn read_blocks(dir: &Path, confine: Option<&Path>) -> Fact<Vec<signex_notebook::Annotation>> {
    let folder = dir.join(NOTEBOOK_DIR);
    let main = folder.join(DEFAULT_NOTEBOOK_FILE);
    if !main.is_file() {
        return Fact::not_read(format!(
            "the project has no {NOTEBOOK_DIR}/{DEFAULT_NOTEBOOK_FILE}"
        ));
    }
    let (sources, _) = NotebookSources::load(DEFAULT_NOTEBOOK_FILE, |path| {
        let file = folder.join(path);
        if let Some(dir) = confine {
            crate::app::document_ids::confined_read(dir, &file).map_err(std::io::Error::other)?;
        }
        std::fs::read(file)
    });
    let blocks = notebook_blocks(&sources);
    let from = format!(
        "{DEFAULT_NOTEBOOK_FILE}, {} blocks in {} files",
        blocks.len(),
        sources.paths().count()
    );
    Fact::read(blocks, from)
}

/// Write the report into the project, unless one is there and `update` was
/// not asked for.
fn write_report(
    dir: &Path,
    text: &str,
    update: bool,
    confine: Option<&Path>,
) -> Result<(), String> {
    let path = dir.join(generated_path());
    // A headless or agent run writes only inside the project, through no
    // link such as a linked `notebook/generated` (#3020).
    crate::app::notebook_project::confined_write(confine, &path)?;
    match write_decision(path.exists(), update) {
        WriteDecision::Refuse => Err(format!(
            "{} already exists; run with update=true (signex-cli notebook generate --force) to replace it",
            generated_path()
        )),
        WriteDecision::Create | WriteDecision::Replace => {
            signex_types::atomic_io::atomic_write(&path, text.as_bytes())
                .map_err(|error| format!("{}: {error}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    /// #3020: with `notebook/generated` linked out of the project, a
    /// headless `notebook generate` is refused and writes nothing there.
    #[cfg(unix)]
    #[test]
    fn a_headless_report_is_not_written_through_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("notebook")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("notebook/generated")).unwrap();
        let refused = super::write_report(dir.path(), "+++\n+++\n", true, Some(dir.path()));
        assert!(refused.is_err(), "{refused:?}");
        let left: Vec<_> = std::fs::read_dir(outside.path()).unwrap().collect();
        assert!(left.is_empty(), "the outside folder stays empty");
    }
}
