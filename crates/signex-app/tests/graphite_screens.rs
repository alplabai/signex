//! Renders the Graphite screens at 1440x900 in both Graphite themes, for a
//! visual check against the design mock-ups.
//!
//! Not part of the normal run: it needs a GPU adapter. Set `GRAPHITE_SHOTS_DIR`
//! to the output directory (and optionally `GRAPHITE_ONLY` to one screen name)
//! and run
//!
//! ```text
//! GRAPHITE_SHOTS_DIR=out cargo test -p signex-app --test graphite_screens -- --ignored
//! ```
//!
//! Files are `<screen>-<light|dark>.png`. A screen that cannot be set up is
//! reported and skipped. Each screen is an audit surface (`tests/audit/`),
//! named here by its mockup row id.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; see regression.rs"
)]

#[path = "regression/settle_support.rs"]
#[allow(dead_code)]
mod settle_support;

#[path = "audit/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "audit/render.rs"]
#[allow(dead_code)]
mod render;
#[path = "audit/surface.rs"]
#[allow(dead_code)]
mod surface;
#[path = "audit/surfaces/mod.rs"]
#[allow(dead_code)]
mod surfaces;

use signex_types::theme::ThemeId;
use std::path::PathBuf;

/// Screen name (the PNG file stem) and the audit surface it renders.
const SCREENS: &[(&str, &str)] = &[
    ("start", "m1"),
    ("schematic", "m86"),
    ("schematic-selected", "m42"),
    ("components", "m52"),
    ("pcb", "m93"),
    ("library", "m118"),
    ("component", "m128"),
    ("footprint", "m124"),
    ("footprint-pad", "m125"),
    ("symbol", "m123"),
    ("preferences", "m144"),
    ("annotate", "m75"),
    ("palette", "m9"),
];

#[test]
#[ignore = "needs a GPU adapter; writes PNGs into $GRAPHITE_SHOTS_DIR"]
fn render_graphite_screens() {
    let Some(mut renderer) = render::gpu() else {
        eprintln!("no wgpu adapter: skipping Graphite screens");
        return;
    };
    // The in-app log is what the Messages panel lists.
    let _ = signex_app::diagnostics::init_logging();
    let dir = PathBuf::from(std::env::var("GRAPHITE_SHOTS_DIR").expect("GRAPHITE_SHOTS_DIR"));
    std::fs::create_dir_all(&dir).expect("out dir");
    let only = std::env::var("GRAPHITE_ONLY").ok();
    // `GRAPHITE_THEMES=light,dark` picks the themes (default both).
    let wanted = std::env::var("GRAPHITE_THEMES").unwrap_or_default();
    let wanted: Vec<&str> = wanted.split(',').filter(|t| !t.is_empty()).collect();
    let all: Vec<surface::Surface> = surfaces::groups()
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .collect();
    for (suffix, theme) in [
        ("light", ThemeId::GraphiteLight),
        ("dark", ThemeId::GraphiteDark),
    ] {
        let on = wanted.is_empty() || wanted.contains(&suffix);
        if !on {
            continue;
        }
        for (name, id) in SCREENS {
            if only.as_deref().is_some_and(|o| o != *name) {
                continue;
            }
            let Some(screen) = all.iter().find(|s| s.id == *id) else {
                eprintln!("SKIP {name}-{suffix}: no audit surface {id}");
                continue;
            };
            let fx = fixture::fixture().expect("fixture");
            let mut app = fixture::fresh(theme);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (screen.open)(&mut app, &mut renderer, &fx)?;
                surface::capture(&app, &mut renderer, screen.window)
            }));
            match result {
                Ok(Ok((rgba, size))) => {
                    render::save_png(&rgba, &dir, &format!("{name}-{suffix}.png"), size)
                        .expect("write png");
                }
                Ok(Err(why)) => eprintln!("SKIP {name}-{suffix}: {why}"),
                Err(_) => eprintln!("SKIP {name}-{suffix}: panicked"),
            }
        }
    }
}
