//! Renders every audited UI surface (a mockup row) in Graphite Light and
//! Dark and drives its checks through `Signex::update`.
//!
//! Not part of the normal run: it needs a GPU adapter. See
//! `tests/audit/README.md` for the environment variables and how to add a
//! row. Writes `<id>-<light|dark>.png` and one JSON line per row and theme
//! to `<AUDIT_DIR>/audit.jsonl`.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; see regression.rs"
)]

#[path = "regression/settle_support.rs"]
#[allow(dead_code)]
mod settle_support;

// Shared with `graphite_screens`, which does not use every helper.
#[allow(dead_code)]
mod fixture;
#[allow(dead_code)]
mod render;
#[allow(dead_code)]
mod surface;
mod surfaces;

use signex_types::theme::ThemeId;
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use surface::Surface;

/// The app's view tree is deep; the default test thread stack overflows.
const STACK_BYTES: usize = 64 * 1024 * 1024;

const THEMES: [(&str, ThemeId); 2] = [
    ("light", ThemeId::GraphiteLight),
    ("dark", ThemeId::GraphiteDark),
];

#[test]
#[ignore = "needs a GPU adapter; writes PNGs and audit.jsonl into $AUDIT_DIR"]
fn audit() {
    let worker = std::thread::Builder::new()
        .name("audit".into())
        .stack_size(STACK_BYTES)
        .spawn(run)
        .expect("spawn the audit thread");
    if let Err(panic) = worker.join() {
        std::panic::resume_unwind(panic);
    }
}

#[derive(Default)]
struct RowOutcome {
    opened: bool,
    checks: Vec<serde_json::Value>,
    error: Option<String>,
}

fn run() {
    // One renderer per process: the font system and device are global.
    let Some(mut renderer) = render::gpu() else {
        eprintln!("no wgpu adapter: skipping the audit");
        return;
    };
    // The in-app log is what the Messages panel lists.
    let _ = signex_app::diagnostics::init_logging();
    let dir = std::env::var_os("AUDIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("signex-audit"));
    std::fs::create_dir_all(&dir).expect("create AUDIT_DIR");
    let only = env_list("AUDIT_ONLY");
    let themes = env_list("AUDIT_THEMES");
    let mut report = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("audit.jsonl"))
        .expect("open audit.jsonl");
    eprintln!("audit output: {}", dir.display());

    let mut rows = 0usize;
    let mut failed = 0usize;
    for (suffix, theme) in THEMES {
        if !themes.is_empty() && !themes.iter().any(|t| t == suffix) {
            continue;
        }
        for (group, group_rows) in surfaces::groups() {
            let whole_group = only.iter().any(|o| o == group);
            for surface in group_rows {
                if !only.is_empty() && !whole_group && !only.iter().any(|o| o == surface.id) {
                    continue;
                }
                let outcome = run_row(&surface, theme, suffix, &mut renderer, &dir);
                rows += 1;
                let bad = !outcome.opened
                    || outcome.error.is_some()
                    || outcome.checks.iter().any(|c| c["ok"] != true);
                if bad {
                    failed += 1;
                    eprintln!(
                        "FAIL {}-{suffix} ({}): {}",
                        surface.id,
                        surface.title,
                        outcome.error.as_deref().unwrap_or("a check failed")
                    );
                }
                let line = serde_json::json!({
                    "id": surface.id,
                    "theme": suffix,
                    "opened": outcome.opened,
                    "checks": outcome.checks,
                    "error": outcome.error,
                });
                writeln!(report, "{line}").expect("append to audit.jsonl");
            }
        }
    }
    eprintln!("audit: {rows} rows, {failed} with a failure");
}

/// A comma-separated environment variable as a list; empty when unset.
fn env_list(name: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Open one surface in a fresh app, save its PNG and run its checks. A panic
/// anywhere is recorded as the row's error instead of ending the run.
fn run_row(
    surface: &Surface,
    theme: ThemeId,
    suffix: &str,
    renderer: &mut iced::Renderer,
    dir: &Path,
) -> RowOutcome {
    let mut outcome = RowOutcome::default();
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let fx = match fixture::fixture() {
            Ok(fx) => fx,
            Err(why) => {
                outcome.error = Some(format!("fixture: {why}"));
                return;
            }
        };
        let mut app = fixture::fresh(theme);
        if let Err(why) = (surface.open)(&mut app, renderer, &fx) {
            outcome.error = Some(why);
            return;
        }
        outcome.opened = true;
        let name = format!("{}-{suffix}.png", surface.id);
        let saved = surface::capture(&app, renderer, surface.window)
            .and_then(|(rgba, size)| render::save_png(&rgba, dir, &name, size));
        if let Err(why) = saved {
            outcome.error = Some(why);
        }
        for (check_name, check) in surface.checks {
            let result = catch_unwind(AssertUnwindSafe(|| check(&mut app)))
                .unwrap_or_else(|panic| Err(format!("panicked: {}", panic_text(&*panic))));
            let ok = result.is_ok();
            outcome.checks.push(serde_json::json!({
                "name": check_name,
                "ok": ok,
                "err": result.err(),
            }));
        }
    }));
    if let Err(panic) = caught {
        outcome.error = Some(format!("panicked: {}", panic_text(&*panic)));
    }
    outcome
}

fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "a non-text panic".to_owned()
    }
}
