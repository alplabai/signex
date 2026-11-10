//! `signex-cli`: thin entry over the library; see `docs/CLI.md`.

use std::io::{self, Write};
use std::process::ExitCode;

use signex_cli::cli::{self, Invocation, USAGE};
use signex_cli::{agent_log, catalog, run};

/// A usage or boot error: the command line or the project is wrong.
const EXIT_ERROR: u8 = 2;
/// The run finished and a command was refused or failed.
const EXIT_NOT_OK: u8 = 1;

fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Ok(Invocation::Help) => print_line(USAGE),
        Ok(Invocation::Version) => print_line(&format!("signex-cli {}", env!("CARGO_PKG_VERSION"))),
        Ok(Invocation::Commands) => print_line(&catalog::catalog_json().to_string()),
        Ok(Invocation::Run(options)) => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            match run(options, &mut stdin.lock(), &mut stdout.lock()) {
                Ok(report) if report.not_ok == 0 => ExitCode::SUCCESS,
                Ok(_) => ExitCode::from(EXIT_NOT_OK),
                Err(error) => fail(&error),
            }
        }
        Ok(Invocation::AgentLog(project, which)) => {
            let mut out = io::stdout().lock();
            match agent_log::run(&project, which, &mut out, &mut io::stderr().lock()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => fail(&error),
            }
        }
        Err(error) => fail(&format!("{error}\n\n{USAGE}")),
    }
}

fn print_line(text: &str) -> ExitCode {
    match writeln!(io::stdout().lock(), "{text}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&format!("writing to standard output: {error}")),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("signex-cli: {message}");
    ExitCode::from(EXIT_ERROR)
}
