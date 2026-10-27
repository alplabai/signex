//! Pixel goldens for the GPU smoke scenes (#113 item 3).
//!
//! Each scene renders offscreen at 128x128 RGBA8 and is compared against a
//! checked-in PNG under `tests/golden/pixels/`.
//!
//! * **Strict compare** runs only on Mesa lavapipe/llvmpipe. The references are
//!   generated there; WARP (Windows) and Metal (macOS) rasterize edges and
//!   blend differently, so comparing them exactly would only be flaky.
//! * **Every adapter** gets coarse invariants: the frame is not all black and
//!   the scene's dominant colours are present. The reason a strict compare was
//!   skipped is printed (`cargo test -- --nocapture`).
//! * A missing adapter fails loudly, as in the other GPU tests.
//! * `SIGNEX_REQUIRE_LAVAPIPE=1` (set by CI) fails closed: a non-lavapipe adapter
//!   or a missing reference panics instead of degrading to coarse checks / SKIP.
//! * Text shapes with the faces in `tests/fonts/` only — never system fonts.
//!
//! Regenerate references (lavapipe only; refuses on any other adapter):
//!
//! ```text
//! SIGNEX_UPDATE_GOLDENS=1 cargo test -p signex-gfx --test pixel_goldens -- --nocapture
//! ```
//!
//! With `SIGNEX_GOLDEN_CANDIDATES=<dir>`, a lavapipe run writes each missing
//! reference to `<dir>/<scene>.png` instead (CI uploads these for review).
//!
//! See `tests/golden/pixels/README.md`.

use signex_gfx::debug_pass::{GOLDEN_SIZE_PX, GoldenFrame, GoldenScene, render_golden_scene};
use std::path::PathBuf;

/// Largest allowed difference in any single channel of any pixel (0-255).
/// Absorbs anti-aliased edge coverage differing between Mesa versions.
const MAX_CHANNEL_DIFF: u8 = 64;
/// Largest allowed mean absolute error over all channels (0-255). Bounds how
/// many edge pixels may move; a missing shape blows far past it.
const MAX_MEAN_ABS_ERROR: f64 = 1.5;
/// A colour counts as "present" when at least this many pixels are within
/// [`COLOUR_SLACK`] of it per channel.
const MIN_COLOUR_PIXELS: usize = 20;
const COLOUR_SLACK: u8 = 24;

const FONTS: &[&[u8]] = &[
    include_bytes!("../fonts/Iosevka-Regular.ttf"),
    include_bytes!("../fonts/Iosevka-Italic.ttf"),
];

struct Case {
    scene: GoldenScene,
    name: &'static str,
    /// RGB colours the frame must contain; empty means "just not all black".
    dominant: &'static [[u8; 3]],
}

const CASES: &[Case] = &[
    Case {
        scene: GoldenScene::LineCircle,
        name: "line_circle",
        dominant: &[[255, 255, 255]],
    },
    Case {
        scene: GoldenScene::Arc,
        name: "arc",
        dominant: &[[255, 255, 255]],
    },
    Case {
        scene: GoldenScene::Polygon,
        name: "polygon",
        dominant: &[[51, 179, 230], [230, 51, 51]],
    },
    Case {
        scene: GoldenScene::Text,
        name: "text",
        dominant: &[],
    },
    Case {
        scene: GoldenScene::Grid,
        name: "grid",
        dominant: &[],
    },
];

/// CI sets `SIGNEX_REQUIRE_LAVAPIPE=1` so the strict compare cannot silently
/// degrade to a skip.
fn require_lavapipe() -> bool {
    std::env::var_os("SIGNEX_REQUIRE_LAVAPIPE").is_some_and(|v| v == "1")
}

fn reference_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/pixels")
        .join(format!("{name}.png"))
}

fn count_near(rgba: &[u8], rgb: [u8; 3]) -> usize {
    rgba.chunks_exact(4)
        .filter(|px| (0..3).all(|c| px[c].abs_diff(rgb[c]) <= COLOUR_SLACK))
        .count()
}

/// Coarse, adapter-independent checks. Returns a description of the first
/// failure.
fn coarse_invariants(case: &Case, rgba: &[u8]) -> Result<(), String> {
    let side = GOLDEN_SIZE_PX as usize;
    if rgba.len() != side * side * 4 {
        return Err(format!(
            "expected {} bytes, got {}",
            side * side * 4,
            rgba.len()
        ));
    }
    // Pixels are u8, so NaN cannot survive readback; an all-black frame is the
    // failure mode of a pipeline that drew nothing.
    let lit = rgba
        .chunks_exact(4)
        .filter(|px| px[..3].iter().any(|&c| c > 16))
        .count();
    if lit == 0 {
        return Err("frame is all black".to_string());
    }
    for rgb in case.dominant {
        let n = count_near(rgba, *rgb);
        if n < MIN_COLOUR_PIXELS {
            return Err(format!("only {n} pixels near expected colour {rgb:?}"));
        }
    }
    Ok(())
}

/// `(max channel diff, mean absolute error)` between two equal-sized buffers.
fn diff_stats(a: &[u8], b: &[u8]) -> (u8, f64) {
    let mut max = 0u8;
    let mut sum = 0u64;
    for (x, y) in a.iter().zip(b) {
        let d = x.abs_diff(*y);
        max = max.max(d);
        sum += u64::from(d);
    }
    (max, sum as f64 / a.len() as f64)
}

fn write_reference(name: &str, rgba: &[u8]) {
    write_png(name, reference_path(name), rgba);
}

fn write_png(name: &str, path: PathBuf, rgba: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("reference dir")).expect("create golden dir");
    image::save_buffer(
        &path,
        rgba,
        GOLDEN_SIZE_PX,
        GOLDEN_SIZE_PX,
        image::ColorType::Rgba8,
    )
    .expect("write golden png");
    println!("pixel golden `{name}`: wrote {}", path.display());
}

fn check_strict(case: &Case, frame: &GoldenFrame) {
    let path = reference_path(case.name);
    if !path.exists() {
        println!(
            "SKIP pixel golden `{}`: no reference at {} — generate it on lavapipe with \
             SIGNEX_UPDATE_GOLDENS=1 (see tests/golden/pixels/README.md)",
            case.name,
            path.display()
        );
        // CI mode: hand the rendered frame out as a reviewable candidate. Never
        // touches the source tree.
        if let Some(dir) = std::env::var_os("SIGNEX_GOLDEN_CANDIDATES").filter(|d| !d.is_empty()) {
            write_png(
                case.name,
                PathBuf::from(dir).join(format!("{}.png", case.name)),
                &frame.rgba,
            );
        }
        if require_lavapipe() {
            panic!(
                "pixel golden `{}`: no reference at {} and SIGNEX_REQUIRE_LAVAPIPE=1 — \
                 commit the candidate (see tests/golden/pixels/README.md)",
                case.name,
                path.display()
            );
        }
        return;
    }
    let reference = image::open(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
        .to_rgba8();
    assert_eq!(
        (reference.width(), reference.height()),
        (GOLDEN_SIZE_PX, GOLDEN_SIZE_PX),
        "{} has the wrong size",
        path.display()
    );
    let (max, mae) = diff_stats(reference.as_raw(), &frame.rgba);
    assert!(
        max <= MAX_CHANNEL_DIFF && mae <= MAX_MEAN_ABS_ERROR,
        "pixel golden `{}` drifted: max channel diff {max} (limit {MAX_CHANNEL_DIFF}), \
         mean abs error {mae:.3} (limit {MAX_MEAN_ABS_ERROR}). If the change is intended, \
         regenerate with SIGNEX_UPDATE_GOLDENS=1 on lavapipe.",
        case.name
    );
}

#[test]
fn smoke_scenes_match_pixel_goldens() {
    let update = std::env::var_os("SIGNEX_UPDATE_GOLDENS").is_some_and(|v| v == "1");
    for case in CASES {
        let frame = pollster::block_on(render_golden_scene(case.scene, FONTS))
            .unwrap_or_else(|err| panic!("pixel golden `{}`: {err}", case.name));
        coarse_invariants(case, &frame.rgba).unwrap_or_else(|err| {
            panic!(
                "pixel golden `{}` on {}: {err}",
                case.name, frame.adapter_name
            )
        });

        if update {
            assert!(
                frame.is_lavapipe,
                "SIGNEX_UPDATE_GOLDENS=1 refused: adapter `{}` ({}) is not lavapipe/llvmpipe",
                frame.adapter_name, frame.backend
            );
            write_reference(case.name, &frame.rgba);
        } else if frame.is_lavapipe {
            check_strict(case, &frame);
        } else if require_lavapipe() {
            panic!(
                "SIGNEX_REQUIRE_LAVAPIPE=1 but adapter `{}` ({}) is not lavapipe/llvmpipe; \
                 pixel golden `{}` cannot be compared strictly",
                frame.adapter_name, frame.backend, case.name
            );
        } else {
            println!(
                "pixel golden `{}`: coarse invariants only — adapter `{}` ({}) is not \
                 lavapipe, references are lavapipe-exact",
                case.name, frame.adapter_name, frame.backend
            );
        }
    }
}

#[test]
fn diff_stats_flags_a_missing_shape_but_not_edge_noise() {
    let blank = vec![0u8; 16 * 4];
    let mut shape = blank.clone();
    shape[..32].fill(255);
    let (max, mae) = diff_stats(&blank, &shape);
    assert!(max > MAX_CHANNEL_DIFF && mae > MAX_MEAN_ABS_ERROR);

    let mut noisy = blank.clone();
    noisy[3] = 40;
    let (max, mae) = diff_stats(&blank, &noisy);
    assert!(max <= MAX_CHANNEL_DIFF && mae <= MAX_MEAN_ABS_ERROR);
}
