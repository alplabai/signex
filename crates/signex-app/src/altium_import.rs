//! File ▸ Import ▸ Altium Project: run the `signex-altium-import` companion.
//!
//! The converter is a separate binary from the `alplabai/signex-altium-import`
//! repo, reached only across a process boundary; nothing of it is linked in.
//! The contract (flags, JSON summary) is that repo's
//! `docs/APP_INTEGRATION.md`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Binary name looked up on `PATH`.
pub const BINARY: &str = "signex-altium-import";

/// Overrides the `PATH` lookup with an explicit binary path.
pub const ENV_OVERRIDE: &str = "SIGNEX_ALTIUM_IMPORT";

/// What a finished import produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    /// The file to open: the `.snxprj` for a project, else the first output
    /// (`.snxsch`, `.snxpcb`, `.snxsym` or `.snxfpt`).
    pub open: PathBuf,
    /// What the converter approximated or skipped, with counts.
    pub warnings: Vec<(String, usize)>,
}

/// The converter binary: `$SIGNEX_ALTIUM_IMPORT`, else `PATH`.
pub fn locate() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(ENV_OVERRIDE).map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let name = format!("{BINARY}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&name))
        .find(|p| p.is_file())
}

/// User-facing text for a missing converter.
pub fn not_installed_message() -> String {
    format!(
        "The Altium importer ({BINARY}) was not found on PATH.\n\
         Install it with:\n  cargo install --git https://github.com/alplabai/signex-altium-import {BINARY}\n\
         or set {ENV_OVERRIDE} to the binary's path."
    )
}

/// Converted files go in a sibling folder so the Altium tree is untouched.
pub fn output_dir_for(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "altium".into());
    input
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{stem}-signex"))
}

/// Run the converter. Blocking; call from `spawn_blocking`.
///
/// Never passes `--force`: if the output folder already holds converted
/// files, the converter refuses and that refusal is the error shown.
pub fn run(binary: &Path, input: &Path, out_dir: &Path) -> Result<ImportOutcome, String> {
    let output = Command::new(binary)
        .arg(input)
        .arg("--output")
        .arg(out_dir)
        .arg("--json")
        .output()
        .map_err(|e| format!("could not start {}: {e}", binary.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_summary(&stdout).map_err(|e| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.trim().is_empty() {
            e
        } else {
            format!("{e}\n{}", stderr.trim())
        }
    })
}

/// Parse the converter's `--json` summary.
pub fn parse_summary(stdout: &str) -> Result<ImportOutcome, String> {
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| "the importer printed no result".to_string())?;
    let v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("unreadable importer result: {e}"))?;
    if v["ok"].as_bool() != Some(true) {
        return Err(v["error"]
            .as_str()
            .unwrap_or("the importer failed without a message")
            .to_string());
    }
    let open = v["project"]
        .as_str()
        .or_else(|| v["files"].as_array()?.first()?.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "the importer reported success but wrote no files".to_string())?;
    let warnings = v["warnings"]
        .as_array()
        .map(|ws| {
            ws.iter()
                .filter_map(|w| {
                    let msg = w["message"].as_str()?.to_string();
                    let n = w["count"].as_u64().unwrap_or(1) as usize;
                    Some((msg, n))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ImportOutcome { open, warnings })
}

/// One line per warning, for the advisory notice.
pub fn warnings_text(warnings: &[(String, usize)]) -> String {
    warnings
        .iter()
        .map(|(m, n)| {
            if *n > 1 {
                format!("• {m} (×{n})")
            } else {
                format!("• {m}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_result_opens_the_project() {
        let out = parse_summary(
            r#"{"ok":true,"error":null,"project":"/o/B.snxprj","files":["/o/A.snxsch","/o/B.snxprj"],"warnings":[{"message":"copper arc imported as straight track segments","count":3}]}"#,
        )
        .unwrap();
        assert_eq!(out.open, PathBuf::from("/o/B.snxprj"));
        assert_eq!(
            out.warnings,
            [(
                "copper arc imported as straight track segments".to_string(),
                3
            )]
        );
    }

    /// Verbatim output of `signex-altium-import Demo.PrjPcb --json` (paths shortened).
    #[test]
    fn parses_real_converter_output() {
        let out = parse_summary(
            r#"{"error":null,"files":["/o/simple.snxsch","/o/simple.snxpcb","/o/Demo.snxprj"],"ok":true,"project":"/o/Demo.snxprj","warnings":[{"count":1,"message":"embedded image not imported"}]}"#,
        )
        .unwrap();
        assert_eq!(out.open, PathBuf::from("/o/Demo.snxprj"));
        assert_eq!(
            out.warnings,
            [("embedded image not imported".to_string(), 1)]
        );
    }

    #[test]
    fn single_document_opens_the_first_file() {
        let out =
            parse_summary(r#"{"ok":true,"project":null,"files":["/o/A.snxsch"],"warnings":[]}"#)
                .unwrap();
        assert_eq!(out.open, PathBuf::from("/o/A.snxsch"));
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn failure_carries_the_importer_error() {
        let err = parse_summary(
            r#"{"ok":false,"error":"/o/A.snxsch already exists; pass --force to replace it","files":[]}"#,
        )
        .unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_summary("").is_err());
        assert!(parse_summary("{not json").is_err());
        assert!(parse_summary(r#"{"ok":true,"files":[]}"#).is_err());
    }

    #[test]
    fn output_goes_beside_the_input() {
        assert_eq!(
            output_dir_for(Path::new("/d/Board.PrjPcb")),
            PathBuf::from("/d/Board-signex")
        );
    }

    #[test]
    fn warnings_render_with_counts() {
        let t = warnings_text(&[("a".into(), 1), ("b".into(), 4)]);
        assert_eq!(t, "• a\n• b (×4)");
    }
}
