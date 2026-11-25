//! #2945 (mockup row 201): the `signex notebook` command line, the exit
//! code with `--fail-on`, and the summary as text.

use std::path::Path;

use serde_json::json;
use signex_notebook::generate::FailOn;

use super::{GenerateCommand, NotebookCommand, USAGE, finish_answer, parse, summary_line};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| arg.to_string()).collect()
}

fn generate(list: &[&str]) -> Result<GenerateCommand, String> {
    match parse(&args(list), Path::new("/work/buck-3v3"))? {
        NotebookCommand::Generate(command) => Ok(command),
        NotebookCommand::Help => Err("help".into()),
    }
}

#[test]
fn the_drawn_command_line_reads() {
    let command = generate(&["generate", "--project", "buck-3v3.snxprj", "--json"]).unwrap();
    assert_eq!(
        command.project,
        Path::new("/work/buck-3v3").join("buck-3v3.snxprj")
    );
    assert!(command.json && !command.force);
    assert!(command.fail_on.is_empty() && command.operands.is_empty());
}

#[test]
fn the_options_become_the_commands_operands() {
    let command = generate(&[
        "generate",
        "--project",
        "/p/buck.snxprj",
        "--force",
        "--fail-on",
        "requirements",
        "--sections",
        "summary,checks",
        "--document",
        "DN-0043",
        "--revision",
        "A",
        "--checks",
        "last",
        "--variant",
        "Lite",
        "--template",
        "design-report-a4",
    ])
    .unwrap();
    assert_eq!(command.project, Path::new("/p/buck.snxprj"));
    assert!(command.force);
    assert_eq!(command.fail_on, [FailOn::Requirements]);
    let named: Vec<(&str, &str)> = command
        .operands
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            ("sections", "summary,checks"),
            ("document_number", "DN-0043"),
            ("revision", "A"),
            ("checks", "last"),
            ("variant", "Lite"),
            ("template", "design-report-a4"),
        ]
    );
}

#[test]
fn a_bad_command_line_is_refused_with_the_reason() {
    let why = |list: &[&str]| generate(list).unwrap_err();
    assert!(why(&["generate", "--json"]).contains("--project"));
    assert!(why(&["generate", "--project"]).contains("needs a value"));
    assert!(why(&["generate", "--project", "p", "--bogus"]).contains("--bogus"));
    assert!(generate(&["generate", "--project", "p", "--fail-on", "x"]).is_err());
    let unknown = parse(&args(&["publish"]), Path::new("/")).unwrap_err();
    assert!(unknown.contains("`publish`"), "{unknown}");
    assert_eq!(parse(&args(&[]), Path::new("/")), Ok(NotebookCommand::Help));
    assert!(USAGE.contains("--fail-on") && USAGE.contains("--force"));
}

fn answer(failing: u64, erc_errors: u64) -> serde_json::Value {
    json!({
        "command": "notebook.generate",
        "output": "notebook/generated/design-report.snxnbk",
        "pages": 9,
        "requirements": {"total": 6, "pass": 6 - failing, "fail": failing, "failed": []},
        "checks": {"erc": {"errors": erc_errors, "warnings": 2, "waived": 0}, "drc": null},
        "written": true,
        "status": "ok",
    })
}

#[test]
fn a_failing_requirement_exits_zero_unless_fail_on_names_it() {
    let mut value = answer(1, 0);
    assert_eq!(finish_answer(&mut value, &[]), 0);
    assert_eq!(value["status"], "ok");
    assert!(value.get("failed_on").is_none());
    let mut value = answer(1, 0);
    assert_eq!(finish_answer(&mut value, &[FailOn::Requirements]), 1);
    assert_eq!(value["status"], "fail");
    assert_eq!(value["failed_on"], json!(["requirements"]));
    let mut value = answer(0, 3);
    assert_eq!(finish_answer(&mut value, &[FailOn::Requirements]), 0);
    assert_eq!(finish_answer(&mut value, &[FailOn::Checks]), 1);
}

#[test]
fn a_report_that_did_not_compile_exits_one() {
    let mut value = answer(0, 0);
    value["status"] = json!("error");
    assert_eq!(finish_answer(&mut value, &[]), 1);
}

#[test]
fn the_summary_reads_as_one_line() {
    let line = summary_line(&answer(1, 0));
    assert_eq!(
        line,
        "wrote notebook/generated/design-report.snxnbk: 9 pages; requirements 5 pass, 1 fail; \
         ERC 0 errors, 2 warnings; DRC not run; status ok"
    );
}
