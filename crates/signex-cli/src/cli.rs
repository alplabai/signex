//! The command line of `signex-cli` (#1923), parsed by hand: three
//! subcommands and a handful of flags.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::capability::{self, Capability};
use crate::input::Limits;

pub const USAGE: &str = "\
signex-cli: run Signex commands without a window

USAGE:
    signex-cli run <project> [OPTIONS] < commands.jsonl
    signex-cli run <project> <command-id> [--arg key=value]... [OPTIONS]
    signex-cli commands [--json]
    signex-cli --help | --version

run reads one JSON command per line from standard input
({\"id\": \"place_junction\", \"args\": {\"at\": [10.16, 5.08]}}) and prints one
JSON result line per command. It stops at the first command that is not ok.

OPTIONS (run):
    --keep-going            run the rest after a refused or failed command
    --save                  allow commands that write files (capability `save`)
    --allow <caps>          also allow capabilities: read, edit, save, export,
                            delete, library-write (default: read,edit)
    --allow-path <dir>      also allow paths inside <dir> (repeatable)
    --max-line-bytes <n>    longest command line (default 65536)
    --max-commands <n>      most commands per run (default 10000)
    --max-file-bytes <n>    largest file a command may read (default 67108864)

Nothing is written without --save. See docs/CLI.md.";

#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    Help,
    Version,
    Commands,
    Run(RunOptions),
}

#[derive(Debug, PartialEq, Eq)]
pub struct RunOptions {
    pub project: PathBuf,
    /// A command id and its `key=value` arguments, instead of reading stdin.
    pub single: Option<(String, Vec<String>)>,
    pub keep_going: bool,
    pub capabilities: BTreeSet<Capability>,
    pub allow_paths: Vec<PathBuf>,
    pub limits: Limits,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Invocation, String> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        None | Some("--help" | "-h" | "help") => Ok(Invocation::Help),
        Some("--version" | "-V") => Ok(Invocation::Version),
        Some("commands") => match args.collect::<Vec<_>>().as_slice() {
            [] => Ok(Invocation::Commands),
            [flag] if flag == "--json" => Ok(Invocation::Commands),
            [other, ..] => Err(format!("`commands` takes only --json, not `{other}`")),
        },
        Some("run") => parse_run(args).map(Invocation::Run),
        Some(other) => Err(format!("unknown subcommand `{other}`")),
    }
}

fn parse_run(mut args: impl Iterator<Item = String>) -> Result<RunOptions, String> {
    let mut positional: Vec<String> = Vec::new();
    let mut operands: Vec<String> = Vec::new();
    let mut options = RunOptions {
        project: PathBuf::new(),
        single: None,
        keep_going: false,
        capabilities: Capability::DEFAULT.into_iter().collect(),
        allow_paths: Vec::new(),
        limits: Limits::default(),
    };
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| args.next().ok_or_else(|| format!("{flag} needs a value"));
        match arg.as_str() {
            "--keep-going" => options.keep_going = true,
            "--save" => {
                options.capabilities.insert(Capability::Save);
            }
            "--allow" => options
                .capabilities
                .extend(capability::parse_list(&value("--allow")?)?),
            "--allow-path" => options
                .allow_paths
                .push(PathBuf::from(value("--allow-path")?)),
            "--arg" => operands.push(value("--arg")?),
            "--max-line-bytes" => {
                options.limits.max_line_bytes =
                    number("--max-line-bytes", &value("--max-line-bytes")?)?
            }
            "--max-commands" => {
                options.limits.max_commands = number("--max-commands", &value("--max-commands")?)?
            }
            "--max-file-bytes" => {
                options.limits.max_file_bytes =
                    number("--max-file-bytes", &value("--max-file-bytes")?)?
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option `{flag}`")),
            _ => positional.push(arg),
        }
    }
    let mut positional = positional.into_iter();
    options.project = positional
        .next()
        .map(PathBuf::from)
        .ok_or("`run` needs a project path")?;
    if let Some(id) = positional.next() {
        options.single = Some((id, operands));
    } else if !operands.is_empty() {
        return Err("--arg needs a command id after the project".into());
    }
    if let Some(extra) = positional.next() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    Ok(options)
}

fn number<T: std::str::FromStr>(flag: &str, text: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{flag} needs a whole number, got `{text}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Invocation, String> {
        parse(args.iter().map(ToString::to_string))
    }

    fn run(args: &[&str]) -> RunOptions {
        match parse_args(args).unwrap() {
            Invocation::Run(options) => options,
            other => panic!("expected run, got {other:?}"),
        }
    }

    #[test]
    fn the_subcommands_and_the_help_and_version_flags_parse() {
        assert_eq!(parse_args(&[]).unwrap(), Invocation::Help);
        assert_eq!(parse_args(&["--help"]).unwrap(), Invocation::Help);
        assert_eq!(parse_args(&["--version"]).unwrap(), Invocation::Version);
        assert_eq!(
            parse_args(&["commands", "--json"]).unwrap(),
            Invocation::Commands
        );
        assert_eq!(parse_args(&["commands"]).unwrap(), Invocation::Commands);
        assert!(
            parse_args(&["commands", "--xml"])
                .unwrap_err()
                .contains("only --json")
        );
        assert!(parse_args(&["commands", "--json", "x"]).is_err());
        assert!(
            parse_args(&["frobnicate"])
                .unwrap_err()
                .contains("unknown subcommand")
        );
    }

    #[test]
    fn a_run_holds_read_and_edit_until_it_names_more() {
        let plain = run(&["run", "p.snxprj"]);
        assert_eq!(plain.capabilities, BTreeSet::from(Capability::DEFAULT));
        assert!(plain.single.is_none() && !plain.keep_going);
        assert_eq!(plain.limits, Limits::default());

        let more = run(&[
            "run",
            "p.snxprj",
            "--save",
            "--allow",
            "export,delete",
            "--keep-going",
        ]);
        let want = BTreeSet::from([
            Capability::Read,
            Capability::Edit,
            Capability::Save,
            Capability::Export,
            Capability::Delete,
        ]);
        assert_eq!(more.capabilities, want);
        assert!(more.keep_going);
    }

    #[test]
    fn paths_limits_and_a_single_command_parse() {
        let options = run(&[
            "run",
            "p.snxprj",
            "--allow-path",
            "a",
            "--allow-path",
            "b",
            "--max-line-bytes",
            "10",
            "--max-commands",
            "2",
            "--max-file-bytes",
            "3",
            "place_junction",
            "--arg",
            "at=1,2",
        ]);
        assert_eq!(
            options.allow_paths,
            [PathBuf::from("a"), PathBuf::from("b")]
        );
        assert_eq!(
            options.limits,
            Limits {
                max_line_bytes: 10,
                max_commands: 2,
                max_file_bytes: 3
            }
        );
        assert_eq!(
            options.single,
            Some(("place_junction".to_string(), vec!["at=1,2".to_string()]))
        );
    }

    #[test]
    fn a_bad_run_line_says_what_is_wrong() {
        for (args, wanted) in [
            (&["run"][..], "needs a project path"),
            (&["run", "p", "--allow"], "--allow needs a value"),
            (&["run", "p", "--allow", "nuke"], "not a capability"),
            (&["run", "p", "--save2"], "unknown option `--save2`"),
            (&["run", "p", "--arg", "a=b"], "--arg needs a command id"),
            (&["run", "p", "id", "extra"], "unexpected argument `extra`"),
            (
                &["run", "p", "--max-commands", "many"],
                "needs a whole number",
            ),
        ] {
            let err = parse_args(args).unwrap_err();
            assert!(err.contains(wanted), "{args:?}: {err}");
        }
    }
}
