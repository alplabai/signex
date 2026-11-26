//! PNG from a rendered SVG, through the rasteriser already in the tree
//! (`resvg`, which the UI toolkit uses for its own vector images). Text is
//! drawn in the bundled default face, so a PNG reads the same on any host.

use std::path::Path;

use resvg::{tiny_skia, usvg};

/// Longer side of the PNG, px.
const LONG_SIDE_PX: f32 = 1024.0;

pub(super) fn write_png(svg: &str, path: &Path) -> Result<(), String> {
    let mut options = usvg::Options::default();
    options.font_family = signex_types::font::BUNDLED_DEFAULT_FAMILY.to_string();
    let fonts = options.fontdb_mut();
    for face in [
        signex_app::fonts::bundled::IOSEVKA_REGULAR,
        signex_app::fonts::bundled::IOSEVKA_BOLD,
        signex_app::fonts::bundled::IOSEVKA_ITALIC,
    ] {
        fonts.load_font_data(face.to_vec());
    }
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| format!("reading the SVG: {e}"))?;
    let size = tree.size();
    let scale = LONG_SIDE_PX / size.width().max(size.height()).max(f32::EPSILON);
    let (w, h) = (
        (size.width() * scale).ceil() as u32,
        (size.height() * scale).ceil() as u32,
    );
    let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("the render is empty")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .save_png(path)
        .map_err(|e| format!("writing {}: {e}", path.display()))
}
