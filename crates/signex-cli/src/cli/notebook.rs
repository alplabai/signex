//! `signex-cli notebook <verb> --project <project> [options]` (mockup
//! rows 201 and 202): a Design Notebook command by its own name, run as
//! `run <project> <command-id> --arg ...` runs it, with the capability it
//! needs. Every option `run` takes is taken here too.

use super::{RunOptions, parse_run};

/// A verb and the catalog command it runs.
struct Verb {
    name: &'static str,
    command: &'static str,
    /// The capabilities the command needs beyond `run`'s default.
    allow: &'static str,
    /// `--flag value` options and the operand each sets.
    values: &'static [(&'static str, &'static str)],
    /// `--flag` switches, the operand each sets and to what.
    switches: &'static [(&'static str, &'static str, bool)],
}

/// Every notebook verb.
const VERBS: &[Verb] = &[Verb {
    name: "export-pdf",
    command: "export_notebook_pdf",
    allow: "export",
    values: &[
        ("--out", "path"),
        ("--notebook", "notebook"),
        ("--pages", "pages"),
        ("--paper", "paper"),
        ("--variant", "variant"),
        ("--annotations", "annotations"),
        ("--stale", "stale"),
    ],
    switches: &[
        ("--no-title-block", "title_block", false),
        ("--no-contents", "contents", false),
        ("--no-bookmarks", "bookmarks", false),
        ("--python-source", "python_source", true),
    ],
}];

/// The verbs, for the usage error.
fn verb_names() -> String {
    VERBS
        .iter()
        .map(|verb| verb.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `notebook <verb> ...` as the run it stands for.
pub(super) fn parse(mut args: impl Iterator<Item = String>) -> Result<RunOptions, String> {
    let name = args
        .next()
        .ok_or_else(|| format!("`notebook` needs a verb: {}", verb_names()))?;
    let verb = VERBS.iter().find(|verb| verb.name == name).ok_or_else(|| {
        format!(
            "unknown notebook verb `{name}`; the verbs are {}",
            verb_names()
        )
    })?;
    let mut project: Option<String> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut passed: Vec<String> = Vec::new();
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| args.next().ok_or_else(|| format!("{flag} needs a value"));
        if arg == "--project" {
            project = Some(value("--project")?);
        } else if arg == "--json" {
            // Every answer is a JSON line already.
        } else if let Some((flag, operand)) = verb.values.iter().find(|(flag, _)| *flag == arg) {
            operands.push(format!("{operand}={}", value(flag)?));
        } else if let Some((_, operand, on)) = verb.switches.iter().find(|(flag, ..)| *flag == arg)
        {
            operands.push(format!("{operand}={on}"));
        } else if !arg.starts_with("--") && project.is_none() {
            project = Some(arg);
        } else {
            passed.push(arg);
        }
    }
    let project = project.ok_or("`notebook` needs the project: --project <file>.snxprj")?;
    let mut run = vec![project.clone(), verb.command.to_string()];
    run.extend(["--arg".to_string(), format!("document={project}")]);
    for operand in operands {
        run.extend(["--arg".to_string(), operand]);
    }
    run.extend(["--allow".to_string(), verb.allow.to_string()]);
    run.extend(passed);
    parse_run(run.into_iter())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::capability::Capability;
    use crate::cli::{Invocation, Route};

    fn notebook(args: &[&str]) -> Result<RunOptions, String> {
        match super::super::parse(
            std::iter::once("notebook")
                .chain(args.iter().copied())
                .map(String::from),
        )? {
            Invocation::Run(options) => Ok(options),
            other => Err(format!("not a run: {other:?}")),
        }
    }

    #[test]
    fn export_pdf_runs_the_export_command_with_the_export_capability() {
        let options = notebook(&[
            "export-pdf",
            "--project",
            "buck-3v3.snxprj",
            "--notebook",
            "generated/design-report",
            "--json",
        ])
        .expect("parses");
        assert_eq!(options.project, std::path::PathBuf::from("buck-3v3.snxprj"));
        assert_eq!(
            options.single,
            Some((
                "export_notebook_pdf".to_string(),
                vec![
                    "document=buck-3v3.snxprj".to_string(),
                    "notebook=generated/design-report".to_string(),
                ]
            ))
        );
        let want = BTreeSet::from([Capability::Read, Capability::Edit, Capability::Export]);
        assert_eq!(options.capabilities, want);
        assert!(
            !options.capabilities.contains(&Capability::Save),
            "nothing else is written"
        );
    }

    #[test]
    fn the_settings_become_operands_and_run_options_pass_through() {
        let options = notebook(&[
            "export-pdf",
            "p.snxprj",
            "--out",
            "/tmp/nb.pdf",
            "--pages",
            "3-7",
            "--stale",
            "fail",
            "--no-bookmarks",
            "--python-source",
            "--headless",
            "--allow-path",
            "/tmp",
        ])
        .expect("parses");
        let (_, operands) = options.single.expect("one command");
        assert_eq!(
            operands,
            [
                "document=p.snxprj",
                "path=/tmp/nb.pdf",
                "pages=3-7",
                "stale=fail",
                "bookmarks=false",
                "python_source=true",
            ]
        );
        assert_eq!(options.route, Route::Headless);
        assert_eq!(options.allow_paths, [std::path::PathBuf::from("/tmp")]);
    }

    #[test]
    fn a_bad_notebook_line_says_what_is_wrong() {
        for (args, wanted) in [
            (&[][..], "needs a verb: export-pdf"),
            (&["publish"], "unknown notebook verb `publish`"),
            (&["export-pdf"], "needs the project"),
            (&["export-pdf", "p.snxprj", "--out"], "--out needs a value"),
            (
                &["export-pdf", "p.snxprj", "--frob"],
                "unknown option `--frob`",
            ),
        ] {
            let err = notebook(args).unwrap_err();
            assert!(err.contains(wanted), "{args:?}: {err}");
        }
    }
}
