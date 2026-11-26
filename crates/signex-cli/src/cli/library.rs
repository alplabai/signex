//! `signex-cli library <verb>`: library tools that need no project and no
//! window. `textconv <file>` prints a footprint file with each embedded
//! model's `data` block as one summary line (#3109), for git's `textconv`
//! diff driver; see `docs/CLI.md`, "Library tools".

use std::io::Write;
use std::path::PathBuf;

/// A `library` verb and its operands.
#[derive(Debug, PartialEq, Eq)]
pub enum LibraryCommand {
    /// `library textconv <file>`.
    Textconv(PathBuf),
}

const VERBS: &str = "textconv";

/// `library <verb> ...`.
pub(super) fn parse(mut args: impl Iterator<Item = String>) -> Result<LibraryCommand, String> {
    let verb = args
        .next()
        .ok_or_else(|| format!("`library` needs a verb: {VERBS}"))?;
    match verb.as_str() {
        "textconv" => {
            let mut file = None;
            for arg in args {
                match arg {
                    flag if flag.starts_with("--") => {
                        return Err(format!("unknown option `{flag}`"));
                    }
                    _ if file.is_none() => file = Some(PathBuf::from(arg)),
                    _ => return Err(format!("unexpected argument `{arg}`")),
                }
            }
            let file = file.ok_or("`library textconv` needs a file")?;
            Ok(LibraryCommand::Textconv(file))
        }
        other => Err(format!(
            "unknown library verb `{other}`; the verbs are {VERBS}"
        )),
    }
}

/// Run `command`, writing its text to `out`.
pub fn run(command: &LibraryCommand, out: &mut impl Write) -> Result<(), String> {
    match command {
        LibraryCommand::Textconv(file) => {
            let bytes = std::fs::read(file)
                .map_err(|error| format!("reading {}: {error}", file.display()))?;
            // A file that is not text is printed as it is: git still diffs it.
            let shown = match std::str::from_utf8(&bytes) {
                Ok(text) => signex_library::textconv_footprint_text(text).into_bytes(),
                Err(_) => bytes,
            };
            out.write_all(&shown)
                .map_err(|error| format!("writing to standard output: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Invocation, parse as parse_cli};
    use signex_library::{Footprint, FootprintFile};

    fn parse_args(args: &[&str]) -> Result<Invocation, String> {
        parse_cli(args.iter().map(ToString::to_string))
    }

    #[test]
    fn textconv_parses_its_file() {
        assert_eq!(
            parse_args(&["library", "textconv", "a.snxfpt"]).unwrap(),
            Invocation::Library(LibraryCommand::Textconv("a.snxfpt".into()))
        );
        for (args, wanted) in [
            (&["library"][..], "needs a verb"),
            (&["library", "frob"], "unknown library verb `frob`"),
            (&["library", "textconv"], "needs a file"),
            (
                &["library", "textconv", "a", "b"],
                "unexpected argument `b`",
            ),
            (&["library", "textconv", "--x", "a"], "unknown option `--x`"),
        ] {
            let err = parse_args(args).unwrap_err();
            assert!(err.contains(wanted), "{args:?}: {err}");
        }
    }

    #[test]
    fn textconv_prints_each_model_as_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = FootprintFile::from_footprint(Footprint::empty("SOIC-8"));
        let uuid = file.footprints[0].uuid;
        let step: Vec<u8> = (0..5000u32).flat_map(|i| i.to_be_bytes()).collect();
        let hash = file
            .attach_step_model(uuid, "SOIC-8.step", &step)
            .unwrap()
            .content_hash;
        let text = file.to_toml_string().unwrap();
        let path = dir.path().join("f.snxfpt");
        std::fs::write(&path, &text).unwrap();

        let mut out = Vec::new();
        run(&LibraryCommand::Textconv(path), &mut out).unwrap();
        let shown = String::from_utf8(out).unwrap();
        assert_eq!(shown, signex_library::textconv_footprint_text(&text));
        let summary = format!(
            "data = <deflate+base64, {} bytes, sha256 {}…>\n",
            step.len(),
            &hash[..12]
        );
        assert!(shown.ends_with(&summary), "{shown}");
        assert!(!shown.contains("data = '''"), "{shown}");
    }

    #[test]
    fn textconv_names_a_file_it_cannot_read() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none.snxfpt");
        let err = run(&LibraryCommand::Textconv(missing), &mut Vec::new()).unwrap_err();
        assert!(err.contains("none.snxfpt"), "{err}");
    }
}
