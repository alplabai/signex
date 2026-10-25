//! Assert on rendered diagnostics records in a test (#623).
//!
//! `target = "signex::prefs"` written inside a tracing macro is valid Rust
//! and valid tracing: it is an ordinary key-value field that happens to be
//! called `target`, not the record's target (that is `target: "…"`, with a
//! colon). Sixteen records shipped that way (#616); no gate saw it, because
//! nothing read the rendered record. The Messages panel's Source column
//! read `SIGNEX_APP-FONTS-PREFS_FILE` instead of `SIGNEX-PREFS`.
//!
//! [`assert_logged`] reads what [`crate::diagnostics::recent_entries`]
//! actually rendered; [`macro_calls_with_target_field`] is the source-level
//! twin, which finds the shape without having to execute the call site.

use crate::diagnostics::{DiagnosticLevel, diagnostic_code_from_target, recent_entries};

/// Panic unless the diagnostics ring holds a record at `level`, whose
/// target is `target` (compared through the Source code the Messages
/// panel shows), and whose message contains `contains`.
pub(crate) fn assert_logged(level: DiagnosticLevel, target: &str, contains: &str) {
    let code = diagnostic_code_from_target(target);
    let entries = recent_entries();
    let hit = entries
        .iter()
        .any(|e| e.level == level && e.code == code && e.message.contains(contains));
    assert!(
        hit,
        "no {} record with target {target:?} (code {code:?}) containing {contains:?}; \
         records mentioning the text: {:#?}",
        level.label(),
        entries
            .iter()
            .filter(|e| e.message.contains(contains))
            .collect::<Vec<_>>()
    );
}

/// Line number of every tracing / log macro call whose argument list
/// carries a top-level `target = …` field instead of `target: …`.
pub(crate) fn macro_calls_with_target_field(source: &str) -> Vec<usize> {
    const MACROS: [&str; 13] = [
        "trace!",
        "debug!",
        "info!",
        "warn!",
        "error!",
        "event!",
        "span!",
        "trace_span!",
        "debug_span!",
        "info_span!",
        "warn_span!",
        "error_span!",
        "log!",
    ];
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    for name in MACROS {
        for (at, _) in source.match_indices(name) {
            let boundary =
                at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            if !boundary {
                continue;
            }
            // `warn! (…)`, `warn!{…}` and `warn![…]` are all macro calls.
            let rest = source[at + name.len()..].trim_start();
            if !rest.starts_with(['(', '[', '{']) {
                continue;
            }
            if args_have_target_field(&rest[1..]) {
                lines.push(source[..at].matches('\n').count() + 1);
            }
        }
    }
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// Index just past the string, raw string or char literal starting at
/// `chars[i]`, or `None` when `chars[i]` does not start one (a lifetime
/// tick is not a literal).
fn skip_literal(chars: &[char], i: usize) -> Option<usize> {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    match chars[i] {
        '"' => {
            let mut j = i + 1;
            while j < chars.len() {
                match chars[j] {
                    // A backslash escapes only the next char, so `"\\"` closes.
                    '\\' => j += 2,
                    '"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(chars.len())
        }
        'r' if i == 0 || !is_ident(chars[i - 1]) || chars[i - 1] == 'b' => {
            let hashes = chars[i + 1..].iter().take_while(|&&c| c == '#').count();
            if chars.get(i + 1 + hashes) != Some(&'"') {
                return None;
            }
            let close: Vec<char> = std::iter::once('"')
                .chain(std::iter::repeat_n('#', hashes))
                .collect();
            let body = i + 2 + hashes;
            let end = (body..chars.len())
                .find(|&j| chars[j..].starts_with(&close))
                .map_or(chars.len(), |j| j + close.len());
            Some(end)
        }
        '\'' => match (chars.get(i + 1), chars.get(i + 2)) {
            (Some('\\'), _) => {
                let close = (i + 2..chars.len()).find(|&j| chars[j] == '\'');
                Some(close.map_or(chars.len(), |j| j + 1))
            }
            (Some(_), Some('\'')) => Some(i + 3),
            _ => None,
        },
        _ => None,
    }
}

/// Scans one macro's argument list (the text after its opening
/// delimiter) at depth 0, ignoring string, raw-string and char literals.
fn args_have_target_field(args: &str) -> bool {
    let chars: Vec<char> = args.chars().collect();
    let segment = |from: usize, to: usize| chars[from..to].iter().collect::<String>();
    let mut depth = 0usize;
    let mut arg_start = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if let Some(next) = skip_literal(&chars, i) {
            i = next;
            continue;
        }
        match chars[i] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => {
                return starts_with_target_field(&segment(arg_start, i));
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if starts_with_target_field(&segment(arg_start, i)) {
                    return true;
                }
                arg_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

fn starts_with_target_field(arg: &str) -> bool {
    arg.trim_start()
        .strip_prefix("target")
        .map(str::trim_start)
        .is_some_and(|rest| rest.starts_with('=') && !rest.starts_with("=="))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn init() {
        let _ = crate::diagnostics::init_logging();
    }

    /// The real path: a tracing macro through the log bridge into the
    /// app logger, read back from the ring the Messages panel shows.
    #[test]
    fn the_target_directive_reaches_the_record() {
        init();
        if crate::diagnostics::configured_level() < log::LevelFilter::Warn {
            eprintln!("skipped: SIGNEX_LOG/RUST_LOG filters out WARN records");
            return;
        }
        tracing::warn!(target: "signex::log_probe_good", "good record marker");
        assert_logged(
            DiagnosticLevel::Warning,
            "signex::log_probe_good",
            "good record marker",
        );
    }

    /// The #616 shape. The record exists, but its target is the module
    /// path, so asserting the intended target fails.
    #[test]
    #[should_panic(expected = "no WARN record with target")]
    fn a_target_written_as_a_field_is_caught() {
        init();
        tracing::warn!(target = "signex::log_probe_bad", "bad record marker");
        assert_logged(
            DiagnosticLevel::Warning,
            "signex::log_probe_bad",
            "bad record marker",
        );
    }

    #[test]
    fn the_scanner_tells_the_two_shapes_apart() {
        let bad = "tracing::warn!(\n    target = \"signex::prefs\",\n    \"x\"\n);";
        let late = "warn!(error = %e, target = \"signex::x\", \"y\");";
        let good = "tracing::warn!(target: \"signex::prefs\", error = %e, \"x\");";
        let other = "info!(\"target = 3\"); warn!(retarget = 1, \"z\"); debug!(a == b);";
        assert_eq!(macro_calls_with_target_field(bad), [1]);
        assert_eq!(macro_calls_with_target_field(late), [1]);
        assert!(macro_calls_with_target_field(good).is_empty());
        assert!(macro_calls_with_target_field(other).is_empty());
    }

    /// Literals must not confuse the argument scan: a quote or bracket
    /// inside one neither ends nor opens anything.
    #[test]
    fn the_scanner_reads_literals_and_macro_forms_correctly() {
        let detected = [
            // `"\\"` closes at its second quote; the field after it is real.
            "warn!(\"a\\\\\", target = \"x\");",
            // Char literals holding a quote or a bracket.
            "warn!('\"', target = \"x\");",
            "warn!(')', target = \"x\");",
            "warn!('(', target = \"x\");",
            "warn!('\\'', target = \"x\");",
            "warn!('\\\\', target = \"x\");",
            // Raw strings with quotes and brackets inside.
            "warn!(r#\")\"#, target = \"x\");",
            "warn!(r#\"a\"b)\"#, target = \"x\");",
            "warn! (target = \"x\", \"m\");",
            "warn!{target = \"x\", \"m\"}",
            "warn![target = \"x\", \"m\"]",
            "tracing::info_span!(target = \"x\", \"s\");",
            "span!(Level::INFO, target = \"x\");",
            "event!(target = \"x\", Level::INFO, \"m\");",
        ];
        for source in detected {
            assert_eq!(macro_calls_with_target_field(source), [1], "{source}");
        }
        let clean = [
            "warn!(r#\"target = \"x\"\"#, y);",
            "warn!(\"target = x\\\", target = y\");",
            "warn!(')', target: \"x\");",
            "warn!(target: \"x\", 'a', \"m\");",
            "info_span!(target: \"x\", \"s\");",
        ];
        for source in clean {
            assert!(macro_calls_with_target_field(source).is_empty(), "{source}");
        }
    }

    /// No source file in the workspace writes `target = …` inside a
    /// tracing macro (#616). A source scan rather than a gate script:
    /// the check needs the macro's argument nesting, which the bash
    /// gates (`scripts/check_*.sh`, single-purpose greps) cannot do
    /// reliably, and `cargo test` already runs on every PR.
    #[test]
    fn no_workspace_source_writes_target_as_a_field() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut offenders = Vec::new();
        let mut stack = vec![crates];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read dir").flatten() {
                let path = entry.path();
                let name = entry.file_name();
                if path.is_dir() {
                    if name != "target" && name != "node_modules" {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|e| e == "rs") && name != "log_probe.rs" {
                    let source = std::fs::read_to_string(&path).expect("read source");
                    offenders.extend(
                        macro_calls_with_target_field(&source)
                            .into_iter()
                            .map(|line| format!("{}:{line}", path.display())),
                    );
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "`target = \"…\"` is a field, not the record target; write `target: \"…\"`: {offenders:#?}"
        );
    }
}
