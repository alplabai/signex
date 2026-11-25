//! `signex notebook …` (#2945, mockup row 201): the Design Notebook from the
//! command line, with no window. `signex notebook generate --project
//! <file>.snxprj --json` runs the catalog command `generate_design_notebook`
//! headless, writes `notebook/generated/design-report.snxnbk` and prints the
//! summary. It exits 0 when the report was made, a failing requirement
//! included, unless `--fail-on` names the condition; it never writes over a
//! report that is there without `--force`. The run is in [`run`].

mod run;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_notebook::generate::{FailOn, parse_fail_on};

pub use run::run;

/// What `signex notebook` prints for `--help` and for a usage error.
pub const USAGE: &str = "\
Usage: signex notebook generate --project <file>.snxprj [OPTIONS]

Generate the project's design report into notebook/generated/design-report.snxnbk.

Options:
  --project <file>     The project file (required)
  --json               Print the summary as JSON
  --force              Replace a design report that is already there
  --fail-on <list>     Exit 1 when one holds: requirements, checks
  --sections <list>    The sections, comma-separated, or all (default all)
  --variant <name>     The variant the BOM is built for (default: the active one)
  --template <name>    The page layout (default design-report-a4)
  --document <number>  The report's document number
  --revision <rev>     The report's revision
  --checks <run|last>  Run ERC, DRC and the requirement checks now, or use the
                       last results (default run)";

/// A `signex notebook` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotebookCommand {
    Help,
    Generate(GenerateCommand),
}

/// `signex notebook generate …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateCommand {
    /// The project file, absolute.
    pub project: PathBuf,
    pub json: bool,
    pub force: bool,
    pub fail_on: Vec<FailOn>,
    /// The command's own operands the flags give, by operand name.
    pub operands: Vec<(String, String)>,
}

/// The run's thread stack: the app's update path runs deep, and the main
/// thread's stack is 1 MB on Windows (#2325).
const RUN_STACK_BYTES: usize = 16 << 20;

/// Run `command` from `main`; the process exit code.
pub fn main(command: NotebookCommand) -> u8 {
    let generate = match command {
        NotebookCommand::Help => {
            println!("{USAGE}");
            return 0;
        }
        NotebookCommand::Generate(generate) => generate,
    };
    let spawned = std::thread::Builder::new()
        .name("signex-notebook".into())
        .stack_size(RUN_STACK_BYTES)
        .spawn(move || {
            run(
                &generate,
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            )
        });
    match spawned.map(|handle| handle.join()) {
        Ok(Ok(code)) => code,
        Ok(Err(_)) => {
            eprintln!("signex notebook: the run stopped on an internal error (panic)");
            2
        }
        Err(error) => {
            eprintln!("signex notebook: starting the run thread: {error}");
            2
        }
    }
}

/// The flags that carry an operand of `generate_design_notebook`.
const OPERAND_FLAGS: [(&str, &str); 6] = [
    ("--sections", "sections"),
    ("--variant", "variant"),
    ("--template", "template"),
    ("--document", "document_number"),
    ("--revision", "revision"),
    ("--checks", "checks"),
];

/// The `signex notebook` command `args` (after `notebook`) names; a relative
/// `--project` is made absolute against `cwd`.
pub fn parse(args: &[String], cwd: &Path) -> Result<NotebookCommand, String> {
    let mut args = args.iter();
    match args.next().map(String::as_str) {
        None | Some("--help" | "-h" | "help") => Ok(NotebookCommand::Help),
        Some("generate") => parse_generate(args, cwd).map(NotebookCommand::Generate),
        Some(other) => Err(format!("unknown notebook command `{other}`")),
    }
}

fn parse_generate<'a>(
    mut args: impl Iterator<Item = &'a String>,
    cwd: &Path,
) -> Result<GenerateCommand, String> {
    let mut project = None;
    let mut command = GenerateCommand {
        project: PathBuf::new(),
        json: false,
        force: false,
        fail_on: Vec::new(),
        operands: Vec::new(),
    };
    while let Some(flag) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("`{flag}` needs a value"))
        };
        match flag.as_str() {
            "--json" => command.json = true,
            "--force" => command.force = true,
            "--project" => project = Some(PathBuf::from(value("--project")?)),
            "--fail-on" => command.fail_on = parse_fail_on(&value("--fail-on")?)?,
            other => match OPERAND_FLAGS.iter().find(|(name, _)| *name == other) {
                Some((name, operand)) => {
                    let given = value(name)?;
                    command.operands.retain(|(op, _)| op != operand);
                    command.operands.push((operand.to_string(), given));
                }
                None => return Err(format!("unknown option `{other}`")),
            },
        }
    }
    let project = project.ok_or("`--project <file>.snxprj` is required")?;
    command.project = if project.is_absolute() {
        project
    } else {
        cwd.join(project)
    };
    Ok(command)
}

/// Apply `fail_on` to the summary `answer`: the conditions that hold are
/// named in `failed_on` and the status becomes `fail`. The exit code: 0 for
/// an ok summary, 1 for a failed condition or a report that did not compile.
pub fn finish_answer(answer: &mut Value, fail_on: &[FailOn]) -> u8 {
    let number = |value: &Value| value.as_u64().unwrap_or(0);
    let failed = number(&answer["requirements"]["fail"]);
    let errors =
        number(&answer["checks"]["erc"]["errors"]) + number(&answer["checks"]["drc"]["errors"]);
    let held: Vec<&str> = fail_on
        .iter()
        .filter(|condition| condition.holds_on(failed, errors))
        .map(|condition| condition.token())
        .collect();
    if !held.is_empty() && answer["status"] == "ok" {
        answer["status"] = Value::from("fail");
    }
    if !held.is_empty() {
        answer["failed_on"] = Value::from(held);
    }
    u8::from(answer["status"] != "ok")
}

/// The summary as one line of text, for a run without `--json`.
pub fn summary_line(answer: &Value) -> String {
    let number = |value: &Value| value.as_u64().unwrap_or(0);
    let requirements = &answer["requirements"];
    let check = |name: &str| match &answer["checks"][name] {
        Value::Null => format!("{} not run", name.to_uppercase()),
        counts => format!(
            "{} {} errors, {} warnings",
            name.to_uppercase(),
            number(&counts["errors"]),
            number(&counts["warnings"])
        ),
    };
    let written = if answer["written"] == true {
        "wrote"
    } else {
        "did not write"
    };
    format!(
        "{written} {}: {} pages; requirements {} pass, {} fail; {}; {}; status {}",
        answer["output"].as_str().unwrap_or_default(),
        number(&answer["pages"]),
        number(&requirements["pass"]),
        number(&requirements["fail"]),
        check("erc"),
        check("drc"),
        answer["status"].as_str().unwrap_or_default()
    )
}
