//! Placed images in an export (#1116, mockup row 204). The caller reads and
//! checks the image files and hands their bytes over in
//! [`ExportImages`]; this crate reads no disk. A PNG, JPEG or GIF becomes an
//! [`SvgElement::Image`] (embedded at its own resolution by the PDF and as a
//! data URI by the SVG document); an SVG picture becomes vector paths, so it
//! prints sharp at any size. An image whose bytes are not given (its file is
//! missing) leaves its box empty, with its border line if it has one.
//! Rules: docs/SCHEMATIC_IMAGES.md, "Export".

use std::collections::HashMap;
use std::sync::Arc;

use signex_types::image_asset::{ImageFormat, sniff_format};
use signex_types::schematic::SchImage;

use super::{SvgElement, SvgPathCommand, SvgPoint, SvgStyle};
use crate::pdf::layout::PageTransform;

/// The checked bytes of the image files an export draws, by file and the
/// content hash an image names.
#[derive(Clone, Default)]
pub struct ExportImages(HashMap<(String, String), Arc<[u8]>>);

impl std::fmt::Debug for ExportImages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

impl ExportImages {
    /// Keep the bytes of `file` with content `hash`.
    pub fn insert(&mut self, file: impl Into<String>, hash: impl Into<String>, bytes: Arc<[u8]>) {
        self.0.insert((file.into(), hash.into()), bytes);
    }

    /// The bytes `image` shows, when the caller gave them.
    pub fn get(&self, image: &SchImage) -> Option<&Arc<[u8]>> {
        self.0.get(&(image.file.clone(), image.hash.clone()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// The pixels of a raster image an export embeds.
#[derive(Clone)]
pub struct SvgPicture {
    pub format: ImageFormat,
    pub bytes: Arc<[u8]>,
}

impl std::fmt::Debug for SvgPicture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgPicture")
            .field("format", &self.format)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// Push the picture of `image` in its box, under its border line.
pub(super) fn push_image_picture(
    out: &mut Vec<SvgElement>,
    image: &SchImage,
    xform: &PageTransform,
    images: &ExportImages,
) {
    let Some(bytes) = images.get(image) else {
        return;
    };
    let Some(format) = sniff_format(bytes) else {
        return;
    };
    let (lo, hi) = image.bounds();
    let x = xform.x(lo.x).min(xform.x(hi.x));
    let y = xform.px_y(lo.y).min(xform.px_y(hi.y));
    let width = (xform.x(hi.x) - xform.x(lo.x)).abs();
    let height = (xform.px_y(hi.y) - xform.px_y(lo.y)).abs();
    if format.is_vector() {
        push_vector(out, bytes, (x, y, width, height), image.alpha());
        return;
    }
    out.push(SvgElement::Image {
        x,
        y,
        width,
        height,
        opacity: image.alpha(),
        picture: Arc::new(SvgPicture {
            format,
            bytes: Arc::clone(bytes),
        }),
    });
}

/// An SVG picture as paths in the box `(x, y, width, height)`.
pub(super) fn push_vector(
    out: &mut Vec<SvgElement>,
    bytes: &[u8],
    (x, y, width, height): (f32, f32, f32, f32),
    opacity: f32,
) {
    // `from_data_nested` follows no `href` out of the picture.
    let Ok(tree) = usvg::Tree::from_data_nested(bytes, &usvg::Options::default()) else {
        return;
    };
    let size = tree.size();
    if !(size.width() > 0.0 && size.height() > 0.0) {
        return;
    }
    let frame = Frame {
        x,
        y,
        sx: width / size.width(),
        sy: height / size.height(),
    };
    walk(tree.root(), &frame, opacity, out);
}

/// The picture's user space mapped onto the image box.
struct Frame {
    x: f32,
    y: f32,
    sx: f32,
    sy: f32,
}

fn walk(group: &usvg::Group, frame: &Frame, opacity: f32, out: &mut Vec<SvgElement>) {
    let opacity = opacity * group.opacity().get();
    for node in group.children() {
        match node {
            usvg::Node::Group(inner) => walk(inner, frame, opacity, out),
            usvg::Node::Path(path) if path.is_visible() => push_path(path, frame, opacity, out),
            // A picture's text needs shaping, and a nested image is refused
            // when the file is checked.
            _ => {}
        }
    }
}

/// A paint as one colour: a gradient prints its first stop.
fn paint_rgb(paint: &usvg::Paint) -> Option<(f32, f32, f32)> {
    let colour = match paint {
        usvg::Paint::Color(colour) => *colour,
        usvg::Paint::LinearGradient(g) => g.stops().first()?.color(),
        usvg::Paint::RadialGradient(g) => g.stops().first()?.color(),
        usvg::Paint::Pattern(_) => return None,
    };
    Some((
        f32::from(colour.red) / 255.0,
        f32::from(colour.green) / 255.0,
        f32::from(colour.blue) / 255.0,
    ))
}

/// `rgb` laid at `alpha` over white paper: a fill has no opacity of its own
/// in the page elements.
fn over_white((r, g, b): (f32, f32, f32), alpha: f32) -> (f32, f32, f32) {
    let a = alpha.clamp(0.0, 1.0);
    (r * a + 1.0 - a, g * a + 1.0 - a, b * a + 1.0 - a)
}

fn push_path(path: &usvg::Path, frame: &Frame, opacity: f32, out: &mut Vec<SvgElement>) {
    let ts = path.abs_transform();
    let place = |p: usvg::tiny_skia_path::Point| {
        let ux = p.x * ts.sx + p.y * ts.kx + ts.tx;
        let uy = p.x * ts.ky + p.y * ts.sy + ts.ty;
        SvgPoint {
            x: frame.x + ux * frame.sx,
            y: frame.y + uy * frame.sy,
        }
    };
    let mut commands = Vec::new();
    let mut current = usvg::tiny_skia_path::Point::from_xy(0.0, 0.0);
    for segment in path.data().segments() {
        use usvg::tiny_skia_path::PathSegment;
        match segment {
            PathSegment::MoveTo(p) => {
                commands.push(SvgPathCommand::MoveTo(place(p)));
                current = p;
            }
            PathSegment::LineTo(p) => {
                commands.push(SvgPathCommand::LineTo(place(p)));
                current = p;
            }
            PathSegment::QuadTo(c, p) => {
                // The cubic that draws the same curve.
                let c1 = usvg::tiny_skia_path::Point::from_xy(
                    current.x + 2.0 / 3.0 * (c.x - current.x),
                    current.y + 2.0 / 3.0 * (c.y - current.y),
                );
                let c2 = usvg::tiny_skia_path::Point::from_xy(
                    p.x + 2.0 / 3.0 * (c.x - p.x),
                    p.y + 2.0 / 3.0 * (c.y - p.y),
                );
                commands.push(SvgPathCommand::CubicTo(place(c1), place(c2), place(p)));
                current = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                commands.push(SvgPathCommand::CubicTo(place(c1), place(c2), place(p)));
                current = p;
            }
            PathSegment::Close => commands.push(SvgPathCommand::Close),
        }
    }
    if commands.is_empty() {
        return;
    }
    let fill_rgb = path.fill().and_then(|fill| {
        paint_rgb(fill.paint()).map(|rgb| over_white(rgb, fill.opacity().get() * opacity))
    });
    let (stroke_rgb, stroke_alpha, stroke_width) = match path.stroke() {
        Some(stroke) => {
            let scale = (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt() * (frame.sx + frame.sy) / 2.0;
            (
                paint_rgb(stroke.paint()),
                stroke.opacity().get() * opacity,
                stroke.width().get() * scale,
            )
        }
        None => (None, 1.0, 0.0),
    };
    if fill_rgb.is_none() && stroke_rgb.is_none() {
        return;
    }
    out.push(SvgElement::Path {
        commands,
        style: SvgStyle {
            stroke_alpha,
            stroke_rgb,
            fill_rgb,
            stroke_width,
        },
    });
}

/// The `<image>` of the SVG document: the picture as a data URI, stretched
/// to the box as the image is on the sheet.
pub(super) fn image_tag(
    picture: &SvgPicture,
    (x, y, width, height): (f32, f32, f32, f32),
    opacity: f32,
) -> String {
    let alpha = if opacity < 1.0 {
        format!(" opacity=\"{opacity}\"")
    } else {
        String::new()
    };
    format!(
        "<image x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" preserveAspectRatio=\"none\"{alpha} href=\"data:{};base64,{}\" />",
        mime(picture.format),
        base64(&picture.bytes)
    )
}

/// A PNG's or GIF's (first frame) pixels as straight RGBA, with its size;
/// `None` for a JPEG (a PDF embeds it as it is), an SVG or a file that does
/// not decode.
pub(crate) fn decode_rgba(picture: &SvgPicture) -> Option<(u32, u32, Vec<u8>)> {
    match picture.format {
        ImageFormat::Png => {
            let pixmap = tiny_skia::Pixmap::decode_png(&picture.bytes).ok()?;
            let rgba = pixmap
                .pixels()
                .iter()
                .flat_map(|p| {
                    let c = p.demultiply();
                    [c.red(), c.green(), c.blue(), c.alpha()]
                })
                .collect();
            Some((pixmap.width(), pixmap.height(), rgba))
        }
        ImageFormat::Gif => decode_gif(&picture.bytes),
        ImageFormat::Jpeg | ImageFormat::Svg => None,
    }
}

/// A GIF's first frame on its logical screen, as straight RGBA.
fn decode_gif(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes).ok()?;
    let (w, h) = (usize::from(decoder.width()), usize::from(decoder.height()));
    let mut screen = vec![0_u8; w * h * 4];
    let frame = decoder.read_next_frame().ok()??;
    let (fw, fh) = (usize::from(frame.width), usize::from(frame.height));
    for row in 0..fh {
        for col in 0..fw {
            let (sx, sy) = (usize::from(frame.left) + col, usize::from(frame.top) + row);
            if sx >= w || sy >= h {
                continue;
            }
            let from = (row * fw + col) * 4;
            let to = (sy * w + sx) * 4;
            if let (Some(src), Some(dst)) =
                (frame.buffer.get(from..from + 4), screen.get_mut(to..to + 4))
            {
                dst.copy_from_slice(src);
            }
        }
    }
    Some((u32::try_from(w).ok()?, u32::try_from(h).ok()?, screen))
}

/// Draw a raster picture into the preview's pixmap, stretched to its box.
pub(super) fn draw_raster(
    pixmap: &mut tiny_skia::Pixmap,
    picture: &SvgPicture,
    (x, y, width, height): (f32, f32, f32, f32),
    opacity: f32,
) {
    let Some((pw, ph, mut rgba)) = decode_rgba(picture) else {
        return;
    };
    for px in rgba.chunks_exact_mut(4) {
        let a = u16::from(px[3]);
        for channel in &mut px[..3] {
            *channel = (u16::from(*channel) * a / 255) as u8;
        }
    }
    let Some(size) = tiny_skia::IntSize::from_wh(pw, ph) else {
        return;
    };
    let Some(source) = tiny_skia::Pixmap::from_vec(rgba, size) else {
        return;
    };
    let paint = tiny_skia::PixmapPaint {
        opacity,
        quality: tiny_skia::FilterQuality::Bilinear,
        ..tiny_skia::PixmapPaint::default()
    };
    let transform =
        tiny_skia::Transform::from_row(width / pw as f32, 0.0, 0.0, height / ph as f32, x, y);
    pixmap.draw_pixmap(0, 0, source.as_ref(), &paint, transform, None);
}

/// `bytes` as base64, for a data URI.
pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The MIME type of a picture's data URI.
pub(super) fn mime(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::Svg => "image/svg+xml",
    }
}
