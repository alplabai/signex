use std::path::PathBuf;

use super::DrawMode;

pub(super) const ALL_LIBRARIES: &str = "All Libraries";

/// Find the KiCad symbol library directory.
pub(super) fn find_kicad_symbols_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        for path in [
            "/Applications/KiCad/KiCad.app/Contents/SharedSupport/symbols",
            "/Applications/KiCad/KiCad Nightly.app/Contents/SharedSupport/symbols",
            "/opt/homebrew/share/kicad/symbols",
            "/usr/local/share/kicad/symbols",
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
            "/usr/share/kicad/symbols",
            "/usr/local/share/kicad/symbols",
            "/var/lib/flatpak/app/org.kicad.KiCad/current/active/files/share/kicad/symbols",
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
        let p = PathBuf::from(format!("C:/Program Files/KiCad/{ver}/share/kicad/symbols"));
        if p.exists() {
            return Some(p);
        }
    }
    }
    None
}

/// List .kicad_sym filenames in a directory.
pub(super) fn list_kicad_libraries(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "kicad_sym"))
                .map(|e| {
                    e.path()
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default()
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

