//! Command-line document arguments (#317).
//!
//! The installers register Signex as the handler for its native file
//! types (`signex.exe "%1"`, `Exec=signex %F`), so the OS launches the
//! binary with the document path(s) as positional arguments. This module
//! turns that argv tail into the paths to open — nothing else. Opening
//! goes through the ordinary `FileMsg::Opened` flow, which already knows
//! every supported extension and reports failures.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Paths named on the command line, made absolute against `cwd`, the
/// working directory, or the error reading it.
///
/// `args` is argv *without* the program name. Anything starting with `-`
/// is a flag this app does not own and is skipped, as is an empty
/// argument. Extensions are deliberately not filtered: an unsupported
/// file must reach the open flow so the user sees an error rather than an
/// empty workspace.
///
/// A relative path is never passed on as relative: a sheet opened from one
/// has no folder, so its child-sheet references resolve against nothing.
/// When the working directory cannot be read, the first relative path is
/// refused with a message saying why; absolute paths need no working
/// directory.
pub fn document_paths<I>(args: I, cwd: Result<&Path, &io::Error>) -> Result<Vec<PathBuf>, String>
where
    I: IntoIterator<Item = OsString>,
{
    args.into_iter()
        .filter(|arg| !arg.is_empty() && !arg.to_string_lossy().starts_with('-'))
        .map(|arg| absolute(PathBuf::from(arg), cwd))
        .collect()
}

/// `path` as an absolute path, or why it cannot be made one.
fn absolute(path: PathBuf, cwd: Result<&Path, &io::Error>) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    match cwd {
        Ok(cwd) => Ok(cwd.join(path)),
        Err(why) => Err(format!(
            "cannot open {}: it is a relative path and the working directory \
             cannot be read ({why}); pass an absolute path",
            path.display()
        )),
    }
}

/// What the command line asks the process to do.
#[derive(Debug, PartialEq, Eq)]
pub enum StartupAction {
    /// `--version` / `-V`: print the version and exit without a window.
    PrintVersion,
    /// `--help` / `-h`: print usage and exit without a window.
    PrintHelp,
    /// Launch the GUI, opening these documents (possibly none), every one
    /// an absolute path.
    Launch(Vec<PathBuf>),
    /// A document path cannot be resolved: print why and exit without a
    /// window.
    Refused(String),
    /// `signex notebook …` (#2945): a Design Notebook command run with no
    /// window.
    Notebook(crate::notebook_cli::NotebookCommand),
}

/// Classify argv (without the program name). `--version` / `--help` win
/// over everything else, in argument order, so the early-exit paths never
/// need a display or GPU.
pub fn startup_action<I>(args: I, cwd: Result<&Path, &io::Error>) -> StartupAction
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    // `signex notebook …` owns everything after `notebook`, its own --help
    // included.
    if args.first().and_then(|arg| arg.to_str()) == Some("notebook") {
        return notebook_action(&args[1..], cwd);
    }
    for arg in &args {
        match arg.to_str() {
            Some("--version" | "-V") => return StartupAction::PrintVersion,
            Some("--help" | "-h") => return StartupAction::PrintHelp,
            _ => {}
        }
    }
    match document_paths(args, cwd) {
        Ok(paths) => StartupAction::Launch(paths),
        Err(why) => StartupAction::Refused(why),
    }
}

/// `signex notebook <args>`: the command, or why the line is refused.
fn notebook_action(args: &[OsString], cwd: Result<&Path, &io::Error>) -> StartupAction {
    let cwd = match cwd {
        Ok(cwd) => cwd,
        Err(why) => {
            return StartupAction::Refused(format!(
                "signex notebook: the working directory cannot be read ({why})"
            ));
        }
    };
    let args: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    match crate::notebook_cli::parse(&args, cwd) {
        Ok(command) => StartupAction::Notebook(command),
        Err(why) => StartupAction::Refused(format!(
            "signex notebook: {why}\n\n{}",
            crate::notebook_cli::USAGE
        )),
    }
}

/// Text printed by `--version`.
pub fn version_text() -> String {
    format!("signex {}", env!("CARGO_PKG_VERSION"))
}

/// Text printed by `--help`.
pub fn help_text() -> String {
    [
        version_text().as_str(),
        "",
        "Usage: signex [OPTIONS] [FILE]...",
        "       signex notebook <COMMAND> [OPTIONS]",
        "",
        "Arguments:",
        "  [FILE]...  Documents to open at startup (.snxsch, .snxpcb, .snxprj, ...)",
        "",
        "Commands:",
        "  notebook   Design Notebook commands with no window (signex notebook --help)",
        "",
        "Options:",
        "  -V, --version  Print the version and exit",
        "  -h, --help     Print this help and exit",
    ]
    .join(
        "
",
    )
}

/// [`startup_action`] over the real process argv and working directory.
/// An unreadable working directory only refuses relative paths: absolute
/// paths (what every OS launcher passes) still work.
pub fn startup_action_from_env() -> StartupAction {
    let cwd = std::env::current_dir();
    startup_action(std::env::args_os().skip(1), cwd.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_opens_nothing() {
        assert_eq!(
            document_paths(args(&[]), Ok(Path::new("/work"))),
            Ok(vec![])
        );
    }

    #[test]
    fn relative_path_resolves_against_cwd() {
        let cwd = Path::new("/work");
        assert_eq!(
            document_paths(args(&["a/b.snxsch"]), Ok(cwd)),
            Ok(vec![cwd.join("a/b.snxsch")])
        );
    }

    #[test]
    fn absolute_path_is_kept_as_is() {
        let abs = std::env::temp_dir().join("p.snxprj");
        let got = document_paths(
            vec![abs.clone().into_os_string()],
            Ok(Path::new("/somewhere/else")),
        );
        assert_eq!(got, Ok(vec![abs]));
    }

    #[test]
    fn flags_and_empty_arguments_are_ignored() {
        let cwd = Path::new("/work");
        assert_eq!(
            document_paths(args(&["--foo", "", "-x", "a.snxsch"]), Ok(cwd)),
            Ok(vec![cwd.join("a.snxsch")])
        );
    }

    #[test]
    fn several_paths_keep_their_order() {
        let cwd = Path::new("/work");
        assert_eq!(
            document_paths(args(&["b.snxsch", "a.snxpcb", "p.snxprj"]), Ok(cwd)),
            Ok(vec![
                cwd.join("b.snxsch"),
                cwd.join("a.snxpcb"),
                cwd.join("p.snxprj")
            ])
        );
    }

    #[test]
    fn unsupported_extension_is_passed_through_for_the_open_flow_to_report() {
        let cwd = Path::new("/work");
        assert_eq!(
            document_paths(args(&["notes.txt"]), Ok(cwd)),
            Ok(vec![cwd.join("notes.txt")])
        );
    }

    #[test]
    fn version_flag_requests_version_even_with_paths() {
        let cwd = Path::new("/work");
        assert_eq!(
            startup_action(args(&["a.snxsch", "--version"]), Ok(cwd)),
            StartupAction::PrintVersion
        );
        assert_eq!(
            startup_action(args(&["-V"]), Ok(cwd)),
            StartupAction::PrintVersion
        );
    }

    #[test]
    fn help_flag_requests_help() {
        let cwd = Path::new("/work");
        assert_eq!(
            startup_action(args(&["--help"]), Ok(cwd)),
            StartupAction::PrintHelp
        );
        assert_eq!(
            startup_action(args(&["-h"]), Ok(cwd)),
            StartupAction::PrintHelp
        );
    }

    #[test]
    fn no_early_exit_flag_launches_with_the_paths() {
        let cwd = Path::new("/work");
        assert_eq!(
            startup_action(args(&["--foo", "a.snxsch"]), Ok(cwd)),
            StartupAction::Launch(vec![cwd.join("a.snxsch")])
        );
        assert_eq!(
            startup_action(args(&[]), Ok(cwd)),
            StartupAction::Launch(vec![])
        );
    }

    /// #2619: with no readable working directory a relative sheet path is
    /// refused by name, never passed on relative.
    #[test]
    fn a_relative_path_is_refused_when_the_working_directory_cannot_be_read() {
        let unreadable = io::Error::new(io::ErrorKind::NotFound, "the folder was removed");
        let why = document_paths(args(&["sheets/top.snxsch"]), Err(&unreadable))
            .expect_err("a relative path has nothing to resolve against");
        let shown = Path::new("sheets/top.snxsch").display().to_string();
        assert!(why.contains(&shown), "{why}");
        assert!(why.contains("working directory cannot be read"), "{why}");
        assert!(why.contains("the folder was removed"), "{why}");
        assert_eq!(
            startup_action(args(&["top.snxsch"]), Err(&unreadable)),
            StartupAction::Refused(
                document_paths(args(&["top.snxsch"]), Err(&unreadable)).expect_err("refused")
            )
        );
    }

    /// #2619: an absolute path needs no working directory.
    #[test]
    fn an_absolute_path_opens_when_the_working_directory_cannot_be_read() {
        let unreadable = io::Error::new(io::ErrorKind::NotFound, "the folder was removed");
        let abs = std::env::temp_dir().join("top.snxsch");
        assert_eq!(
            startup_action(vec![abs.clone().into_os_string()], Err(&unreadable)),
            StartupAction::Launch(vec![abs])
        );
    }

    /// Every launched path is absolute, so a sheet always has a folder for
    /// its child-sheet references.
    #[test]
    fn launched_paths_are_absolute() {
        let cwd = std::env::temp_dir();
        let StartupAction::Launch(paths) =
            startup_action(args(&["a.snxsch", "sub/b.snxpcb"]), Ok(cwd.as_path()))
        else {
            panic!("expected a launch");
        };
        assert!(paths.iter().all(|path| path.is_absolute()), "{paths:?}");
    }

    #[test]
    fn version_text_carries_the_crate_version() {
        assert!(version_text().ends_with(env!("CARGO_PKG_VERSION")));
        assert!(help_text().contains("--version"));
        assert!(help_text().contains("signex notebook"));
    }

    /// #2945: `signex notebook …` is a command, its --help its own.
    #[test]
    fn notebook_takes_the_rest_of_the_line() {
        let cwd = Path::new("/work/buck-3v3");
        let action = startup_action(
            args(&[
                "notebook",
                "generate",
                "--project",
                "buck-3v3.snxprj",
                "--json",
            ]),
            Ok(cwd),
        );
        let StartupAction::Notebook(crate::notebook_cli::NotebookCommand::Generate(command)) =
            action
        else {
            panic!("expected a notebook command: {action:?}");
        };
        assert_eq!(command.project, cwd.join("buck-3v3.snxprj"));
        assert!(command.json);
        assert_eq!(
            startup_action(args(&["notebook", "--help"]), Ok(cwd)),
            StartupAction::Notebook(crate::notebook_cli::NotebookCommand::Help)
        );
        let StartupAction::Refused(why) = startup_action(args(&["notebook", "lint"]), Ok(cwd))
        else {
            panic!("expected a refusal");
        };
        assert!(why.contains("`lint`"), "{why}");
    }
}
