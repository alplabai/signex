//! `signex-cli library check <dir> [--with <dir>]... [--json]` (#3111): check
//! a library directory headlessly, for converters and their CI. Exit 0 when
//! the check finds no error (warnings allowed), 1 when it does, 2 on a usage
//! error. See `docs/CLI.md`, "Library check".

use std::io::{self, Write};
use std::path::PathBuf;

use signex_library::check::{Problem, Severity, check_library, has_errors};

#[derive(Debug, PartialEq, Eq)]
pub struct LibraryCheckOptions {
    pub library: PathBuf,
    pub with: Vec<PathBuf>,
    pub json: bool,
}

/// Parse the words after `library`.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<LibraryCheckOptions, String> {
    match args.next().as_deref() {
        Some("check") => {}
        Some(other) => return Err(format!("unknown `library` subcommand `{other}`")),
        None => return Err("`library` needs a subcommand: check".into()),
    }
    let mut library = None;
    let mut with = Vec::new();
    let mut json = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--with" => with.push(PathBuf::from(
                args.next().ok_or("--with needs a library folder")?,
            )),
            flag if flag.starts_with("--") => return Err(format!("unknown option `{flag}`")),
            _ if library.is_none() => library = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    let library = library.ok_or("`library check` needs a library folder or .snxlib")?;
    Ok(LibraryCheckOptions {
        library,
        with,
        json,
    })
}

/// Run the check, print the result and say whether it found an error.
pub fn run(options: &LibraryCheckOptions, out: &mut impl Write) -> io::Result<bool> {
    let problems = check_library(&options.library, &options.with);
    if options.json {
        writeln!(out, "{}", json_report(options, &problems))?;
    } else {
        for p in &problems {
            let severity = match p.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let item = p
                .item
                .as_deref()
                .map(|i| format!(" [{i}]"))
                .unwrap_or_default();
            writeln!(
                out,
                "{severity} {}: {}{item}: {}",
                p.code, p.file, p.message
            )?;
        }
        let (errors, warnings) = counts(&problems);
        writeln!(out, "{errors} error(s), {warnings} warning(s)")?;
    }
    Ok(has_errors(&problems))
}

fn counts(problems: &[Problem]) -> (usize, usize) {
    let errors = problems
        .iter()
        .filter(|p| p.severity == Severity::Error)
        .count();
    (errors, problems.len() - errors)
}

fn json_report(options: &LibraryCheckOptions, problems: &[Problem]) -> serde_json::Value {
    let (errors, warnings) = counts(problems);
    serde_json::json!({
        "ok": errors == 0,
        "library": options.library,
        "errors": errors,
        "warnings": warnings,
        "problems": problems,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<LibraryCheckOptions, String> {
        parse(args.iter().map(ToString::to_string))
    }

    #[test]
    fn check_takes_a_library_with_libraries_and_json() {
        let options =
            parse_args(&["check", "lib", "--with", "a", "--with", "b", "--json"]).unwrap();
        assert_eq!(options.library, PathBuf::from("lib"));
        assert_eq!(options.with, [PathBuf::from("a"), PathBuf::from("b")]);
        assert!(options.json);
    }

    #[test]
    fn usage_errors_are_refused() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["verify", "lib"]).is_err());
        assert!(parse_args(&["check"]).is_err());
        assert!(parse_args(&["check", "lib", "--with"]).is_err());
        assert!(parse_args(&["check", "lib", "--strict"]).is_err());
        assert!(parse_args(&["check", "lib", "other"]).is_err());
    }

    #[test]
    fn the_json_report_lists_problems_with_file_and_row() {
        let options = parse_args(&["check", "/nowhere/at/all", "--json"]).unwrap();
        let mut out = Vec::new();
        assert!(
            run(&options, &mut out).unwrap(),
            "a missing library is an error"
        );
        let line: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(line["ok"], false);
        assert_eq!(line["errors"], 1);
        assert_eq!(line["problems"][0]["code"], "library.not_found");
        assert_eq!(line["problems"][0]["severity"], "error");
        assert!(line["problems"][0].get("file").is_some());
    }
}
