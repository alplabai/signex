use std::path::PathBuf;

use super::DrawMode;

pub(super) const ALL_LIBRARIES: &str = "All Libraries";

/// Find the Standard symbol library directory.
pub(super) fn find_standard_symbols_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        for path in [
            "/Applications/Standard/Standard.app/Contents/SharedSupport/symbols",
            "/Applications/Standard/Standard Nightly.app/Contents/SharedSupport/symbols",
            "/opt/homebrew/share/standard/symbols",
            "/usr/local/share/standard/symbols",
        ] {
            let candidate = PathBuf::from(path);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        for path in [
            "/usr/share/standard/symbols",
            "/usr/local/share/standard/symbols",
            "/var/lib/flatpak/app/org.standard.Standard/current/active/files/share/standard/symbols",
        ] {
            let candidate = PathBuf::from(path);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        for ver in &["9.0", "8.0", "7.0"] {
            let p = PathBuf::from(format!(
                "C:/Program Files/Standard/{ver}/share/standard/symbols"
            ));
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// List .standard_sym filenames in a directory.
///
/// Read once, from `Signex::new()`, into the Components panel's library
/// pick_list — so a directory that cannot be read leaves that list empty
/// for the whole session. `list_dir_or_report` surfaces that instead of
/// letting it read as "no standard libraries installed".
pub(super) fn list_standard_libraries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> =
        super::dir_listing::list_dir_or_report(dir, "standard symbol libraries")
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "standard_sym"))
            .map(|path| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
    names.sort();
    names
}

/// Given a start and end point, produce wire segments constrained by the draw mode.
/// - Ortho90: horizontal then vertical (two segments forming a 90-degree corner)
/// - Angle45: snap to nearest 45-degree angle (may produce one or two segments)
/// - FreeAngle: single straight segment
pub(super) fn constrain_segments(
    start: signex_types::schematic::Point,
    end: signex_types::schematic::Point,
    mode: DrawMode,
) -> Vec<(
    signex_types::schematic::Point,
    signex_types::schematic::Point,
)> {
    use signex_types::schematic::Point;

    let dx = end.x - start.x;
    let dy = end.y - start.y;

    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        return vec![];
    }

    match mode {
        DrawMode::FreeAngle => {
            vec![(start, end)]
        }
        DrawMode::Ortho90 => {
            // Horizontal first, then vertical (like Altium default)
            if dx.abs() < 0.01 {
                // Pure vertical
                vec![(start, end)]
            } else if dy.abs() < 0.01 {
                // Pure horizontal
                vec![(start, end)]
            } else {
                let corner = Point::new(end.x, start.y);
                vec![(start, corner), (corner, end)]
            }
        }
        DrawMode::Angle45 => {
            // Snap to nearest 45-degree increment
            let adx = dx.abs();
            let ady = dy.abs();
            if adx < 0.01 || ady < 0.01 {
                // Already axis-aligned
                vec![(start, end)]
            } else if (adx - ady).abs() < adx * 0.4 {
                // Close to 45-degree: make it exactly 45-degree
                let d = adx.min(ady);
                let sx = if dx > 0.0 { 1.0 } else { -1.0 };
                let sy = if dy > 0.0 { 1.0 } else { -1.0 };
                let diag_end = Point::new(start.x + d * sx, start.y + d * sy);
                if (adx - ady).abs() < 0.01 {
                    // Exactly 45-degree
                    vec![(start, diag_end)]
                } else if adx > ady {
                    // Diagonal then horizontal
                    vec![(start, diag_end), (diag_end, Point::new(end.x, diag_end.y))]
                } else {
                    // Diagonal then vertical
                    vec![(start, diag_end), (diag_end, Point::new(diag_end.x, end.y))]
                }
            } else {
                // Mostly axis-aligned: use ortho
                let corner = Point::new(end.x, start.y);
                vec![(start, corner), (corner, end)]
            }
        }
    }
}
