//! Images placed on a sheet are project assets (#1116, mockup row 204): the
//! sheet stores a project-relative path and the content hash, never the
//! picture. This module is the one rule set for those files: which paths a
//! sheet may name, what a file must be to be placed (PNG, JPEG, GIF or SVG,
//! within size limits, an SVG with no outside reference), the hash, and the
//! name a file picked from outside the project gets under `assets/images/`.
//! Pure: the caller reads the bytes. Rules: docs/SCHEMATIC_IMAGES.md.

use sha2::{Digest, Sha256};

/// The project folder images are copied into, relative to the project root.
pub const IMAGE_ASSET_DIR: &str = "assets/images";

/// Largest image file placed or loaded.
pub const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

/// Largest edge, in pixels, of a raster image or an SVG's own size.
pub const MAX_IMAGE_SIDE_PX: u32 = 16_384;

/// Most pixels of a raster image: 64 Mpx is 256 MiB once decoded to RGBA.
pub const MAX_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;

/// The resolution a file's own size is read at when it names none.
pub const IMAGE_FILE_DPI: f64 = 96.0;

/// The kinds of image a sheet places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Svg,
}

impl ImageFormat {
    /// The file extension a copied asset is given.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Svg => "svg",
        }
    }

    /// Every extension a file of this kind may already carry.
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Png => &["png"],
            Self::Jpeg => &["jpg", "jpeg"],
            Self::Gif => &["gif"],
            Self::Svg => &["svg"],
        }
    }

    /// An SVG draws as vectors; the others are pixels.
    pub fn is_vector(self) -> bool {
        self == Self::Svg
    }

    /// The format's own name, as a person reads it: the same in every
    /// language.
    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Svg => "SVG",
        }
    }

    /// The name in a JSON answer.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Svg => "svg",
        }
    }
}

/// Why an image path or file is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageRefusal {
    #[error("the image path is empty")]
    EmptyPath,
    #[error("the image path `{0}` is absolute; a sheet names images inside its project")]
    AbsolutePath(String),
    #[error("the image path `{0}` leaves the project folder")]
    LeavesProject(String),
    #[error("the file is {bytes} bytes; an image may be up to {limit} bytes")]
    TooLarge { bytes: u64, limit: u64 },
    #[error("the file is not a PNG, JPEG, GIF or SVG image")]
    UnknownFormat,
    #[error("the size of the image could not be read")]
    NoSize,
    #[error("the image is {width} × {height} pixels, over the limit")]
    TooManyPixels { width: u32, height: u32 },
    #[error("the SVG refers to something outside itself (`{0}`)")]
    ExternalReference(String),
}

/// What a checked image file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFacts {
    pub format: ImageFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub bytes: u64,
    /// [`content_hash`] of the file.
    pub hash: String,
}

impl ImageFacts {
    /// The file's own size in mm, at [`IMAGE_FILE_DPI`].
    pub fn natural_size_mm(&self) -> (f64, f64) {
        (px_to_mm(self.width_px), px_to_mm(self.height_px))
    }
}

/// Pixels to mm at [`IMAGE_FILE_DPI`].
pub fn px_to_mm(px: u32) -> f64 {
    f64::from(px) * 25.4 / IMAGE_FILE_DPI
}

/// The SHA-256 of `bytes`, lower-case hex.
pub fn content_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `path` as a sheet stores it: relative to the project root, `/`
/// separated, without `.` parts. Refused when it is empty, absolute (a
/// root, a drive or a share) or steps out with `..` (the class of #112).
pub fn checked_asset_path(path: &str) -> Result<String, ImageRefusal> {
    let unified = path.trim().replace('\\', "/");
    if unified.is_empty() {
        return Err(ImageRefusal::EmptyPath);
    }
    let bytes = unified.as_bytes();
    let has_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    if unified.starts_with('/') || unified.starts_with('~') || has_drive {
        return Err(ImageRefusal::AbsolutePath(path.to_string()));
    }
    let mut parts = Vec::new();
    for part in unified.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(ImageRefusal::LeavesProject(path.to_string())),
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err(ImageRefusal::EmptyPath);
    }
    Ok(parts.join("/"))
}

/// The image kind `bytes` hold, from their first bytes; `None` for any other
/// file.
pub fn sniff_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(ImageFormat::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageFormat::Gif);
    }
    let head = &bytes[..bytes.len().min(4096)];
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    (text.starts_with('<') && text.contains("<svg")).then_some(ImageFormat::Svg)
}

/// Check `bytes` as an image to place or draw: the size limit, the kind, the
/// pixel size and, for an SVG, that it refers to nothing outside itself.
pub fn check_image(bytes: &[u8]) -> Result<ImageFacts, ImageRefusal> {
    let size = bytes.len() as u64;
    if size > MAX_IMAGE_BYTES {
        return Err(ImageRefusal::TooLarge {
            bytes: size,
            limit: MAX_IMAGE_BYTES,
        });
    }
    let format = sniff_format(bytes).ok_or(ImageRefusal::UnknownFormat)?;
    if format == ImageFormat::Svg {
        let text = String::from_utf8_lossy(bytes);
        if let Some(reference) = external_svg_reference(&text) {
            return Err(ImageRefusal::ExternalReference(reference));
        }
    }
    let (width_px, height_px) = pixel_size(format, bytes).ok_or(ImageRefusal::NoSize)?;
    let pixels = u64::from(width_px) * u64::from(height_px);
    if width_px == 0
        || height_px == 0
        || width_px > MAX_IMAGE_SIDE_PX
        || height_px > MAX_IMAGE_SIDE_PX
        || (!format.is_vector() && pixels > MAX_IMAGE_PIXELS)
    {
        return Err(ImageRefusal::TooManyPixels {
            width: width_px,
            height: height_px,
        });
    }
    Ok(ImageFacts {
        format,
        width_px,
        height_px,
        bytes: size,
        hash: content_hash(bytes),
    })
}

/// The pixel size of an image, read from its header; an SVG's from its
/// `width` / `height` attributes, else its `viewBox`, at [`IMAGE_FILE_DPI`].
pub fn pixel_size(format: ImageFormat, bytes: &[u8]) -> Option<(u32, u32)> {
    match format {
        ImageFormat::Png => {
            if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
                return None;
            }
            Some((be_u32(&bytes[16..20]), be_u32(&bytes[20..24])))
        }
        ImageFormat::Gif => {
            if bytes.len() < 10 {
                return None;
            }
            let w = u16::from_le_bytes([bytes[6], bytes[7]]);
            let h = u16::from_le_bytes([bytes[8], bytes[9]]);
            Some((u32::from(w), u32::from(h)))
        }
        ImageFormat::Jpeg => jpeg_size(bytes),
        ImageFormat::Svg => svg_size(&String::from_utf8_lossy(bytes)),
    }
}

fn be_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// The size in a JPEG's start-of-frame segment.
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        i += 2;
        match marker {
            // Fill bytes and markers without a length.
            0xFF => i -= 1,
            0x01 | 0xD0..=0xD8 => {}
            _ => {
                let len = usize::from(u16::from_be_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]));
                let is_frame =
                    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
                if is_frame {
                    let h = u16::from_be_bytes([*bytes.get(i + 3)?, *bytes.get(i + 4)?]);
                    let w = u16::from_be_bytes([*bytes.get(i + 5)?, *bytes.get(i + 6)?]);
                    return Some((u32::from(w), u32::from(h)));
                }
                if len < 2 {
                    return None;
                }
                i += len;
            }
        }
    }
    None
}

/// The text of the root `<svg …>` start tag.
fn svg_root_tag(text: &str) -> Option<&str> {
    let start = text.find("<svg")?;
    let end = text[start..].find('>')?;
    Some(&text[start..start + end])
}

/// The value of attribute `name` in the start tag `tag`.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().next_back();
        let after = &rest[at + name.len()..];
        let after = after.trim_start();
        if before.is_some_and(char::is_whitespace)
            && let Some(value) = after.strip_prefix('=')
        {
            let value = value.trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                let body = &value[1..];
                return body.find(quote).map(|end| &body[..end]);
            }
        }
        rest = &rest[at + name.len()..];
    }
    None
}

/// A length attribute in px: a bare number or px, else mm, cm, in or pt; a
/// percentage has no size of its own.
fn svg_length_px(value: &str) -> Option<f64> {
    let value = value.trim();
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: f64 = number.parse().ok()?;
    let per_unit = match unit.trim() {
        "" | "px" => 1.0,
        "mm" => IMAGE_FILE_DPI / 25.4,
        "cm" => IMAGE_FILE_DPI / 2.54,
        "in" => IMAGE_FILE_DPI,
        "pt" => IMAGE_FILE_DPI / 72.0,
        _ => return None,
    };
    let px = number * per_unit;
    (px.is_finite() && px > 0.0).then_some(px)
}

fn svg_size(text: &str) -> Option<(u32, u32)> {
    let tag = svg_root_tag(text)?;
    let from_box = || {
        let numbers: Vec<f64> = attribute(tag, "viewBox")?
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        match numbers[..] {
            [_, _, w, h] if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 => Some((w, h)),
            _ => None,
        }
    };
    let width = attribute(tag, "width").and_then(svg_length_px);
    let height = attribute(tag, "height").and_then(svg_length_px);
    let (w, h) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (w, h) => {
            let (bw, bh) = from_box()?;
            match (w, h) {
                (Some(w), None) => (w, w * bh / bw),
                (None, Some(h)) => (h * bw / bh, h),
                _ => (bw, bh),
            }
        }
    };
    // A vector has no pixels to decode: a huge declared size is only clamped.
    let side = |v: f64| -> Option<u32> {
        (v.is_finite() && v >= 0.5)
            .then(|| v.round().clamp(1.0, f64::from(MAX_IMAGE_SIDE_PX)) as u32)
    };
    Some((side(w)?, side(h)?))
}

/// The first reference in an SVG to anything outside the file: an `href`
/// that is not a `#fragment` or a `data:` URI, a CSS `url(…)` or `@import`
/// of the same, or an external entity. `None` for a self-contained file.
pub fn external_svg_reference(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if let Some(at) = lower.find("<!entity") {
        let decl = &lower[at..lower[at..].find('>').map_or(lower.len(), |end| at + end)];
        if decl.contains("system") || decl.contains("public") {
            return Some(clip(&text[at..at + decl.len()]));
        }
    }
    if let Some(at) = lower.find("@import") {
        return Some(clip(&text[at..]));
    }
    let mut from = 0;
    while let Some(found) = lower[from..].find("href") {
        let at = from + found;
        from = at + 4;
        let after = lower[from..].trim_start();
        let Some(value) = after.strip_prefix('=') else {
            continue;
        };
        let value = value.trim_start();
        let Some(quote) = value.chars().next().filter(|q| *q == '"' || *q == '\'') else {
            continue;
        };
        let body = &value[1..];
        let reference = &body[..body.find(quote).unwrap_or(body.len())];
        if !is_inside(reference) {
            return Some(clip(reference));
        }
    }
    let mut from = 0;
    while let Some(found) = lower[from..].find("url(") {
        let at = from + found + 4;
        from = at;
        let body = &lower[at..];
        let reference = body[..body.find(')').unwrap_or(body.len())]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'');
        if !is_inside(reference) {
            return Some(clip(reference));
        }
    }
    None
}

/// A reference that stays in the file: a fragment or inline data.
fn is_inside(reference: &str) -> bool {
    let reference = reference.trim();
    reference.starts_with('#') || reference.to_ascii_lowercase().starts_with("data:")
}

fn clip(text: &str) -> String {
    text.chars().take(80).collect()
}

/// The file name a picked file is copied in as: its own name, with only
/// letters, digits, `.`, `-` and `_` kept, and the kind's extension when it
/// carries another.
pub fn asset_file_name(picked_name: &str, format: ImageFormat) -> String {
    let base = picked_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(picked_name);
    let mut name: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    name = name.trim_matches(|c| c == '.' || c == '-').to_string();
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_ascii_lowercase()),
        _ => (name.clone(), String::new()),
    };
    let stem = if stem.is_empty() {
        "image".to_string()
    } else {
        stem
    };
    let ext = if format.extensions().contains(&ext.as_str()) {
        ext
    } else {
        format.extension().to_string()
    };
    format!("{stem}.{ext}")
}

/// Where a file with `hash` goes under [`IMAGE_ASSET_DIR`], given the files
/// already there as `(name, hash)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetChoice {
    /// The project-relative path the sheet stores.
    pub file: String,
    /// A file with the same content is already there: nothing is copied.
    pub reuses_existing: bool,
}

/// The asset path for content `hash` wanted as `wanted` (an
/// [`asset_file_name`]): a file already holding the same content is reused,
/// else the wanted name, else the name with the hash's first 8, then 16, then
/// all of its digits added.
pub fn pick_asset_name(wanted: &str, hash: &str, existing: &[(String, String)]) -> AssetChoice {
    let path = |name: &str| format!("{IMAGE_ASSET_DIR}/{name}");
    let same = existing
        .iter()
        .filter(|(_, h)| h == hash)
        .map(|(n, _)| n.as_str());
    let mut same: Vec<&str> = same.collect();
    same.sort_unstable();
    if let Some(name) = same.iter().find(|n| **n == wanted).or(same.first()) {
        return AssetChoice {
            file: path(name),
            reuses_existing: true,
        };
    }
    let taken = |name: &str| existing.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    let (stem, ext) = wanted.rsplit_once('.').unwrap_or((wanted, ""));
    let candidates = std::iter::once(wanted.to_string()).chain(
        [8, 16, hash.len()]
            .into_iter()
            .map(|n| format!("{stem}-{}.{ext}", &hash[..n.min(hash.len())])),
    );
    let mut name = wanted.to_string();
    for candidate in candidates {
        name = candidate;
        if !taken(name.as_str()) {
            break;
        }
    }
    AssetChoice {
        file: path(name.as_str()),
        reuses_existing: false,
    }
}

/// Why a picked or placed image file could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageReadError {
    /// The file breaks a rule of this module.
    Refused(ImageRefusal),
    /// The file could not be read: the system's reason.
    Unreadable(String),
    /// The file is there but is not the content the sheet placed (its hash
    /// differs): drawn as missing, never as the other picture.
    Changed,
}

/// A picked file, checked and read, and where it lives in the project.
#[derive(Clone, PartialEq, Eq)]
pub struct PickedImage {
    pub facts: ImageFacts,
    /// The checked content.
    pub bytes: std::sync::Arc<[u8]>,
    /// The project-relative path the sheet stores.
    pub file: String,
    /// The project root `file` is relative to.
    pub root: std::path::PathBuf,
    /// The file is not in the project yet: the save copies it there.
    pub needs_copy: bool,
    /// The picked file's own name, for messages.
    pub picked_name: String,
}

impl std::fmt::Debug for PickedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PickedImage")
            .field("facts", &self.facts)
            .field("file", &self.file)
            .field("needs_copy", &self.needs_copy)
            .finish()
    }
}

/// A picked file waiting to be copied into the project when the sheet that
/// places it is saved (mockup row 204: nothing is written before the save).
#[derive(Clone, PartialEq, Eq)]
pub struct PendingImageCopy {
    /// The sheet whose save writes it.
    pub sheet: std::path::PathBuf,
    /// The project root `file` is relative to.
    pub root: std::path::PathBuf,
    /// The project-relative path it is written to.
    pub file: String,
    pub hash: String,
    /// The checked content, as hashed: the copy writes these bytes, not the
    /// picked file again, which may have changed since.
    pub bytes: std::sync::Arc<[u8]>,
}

impl std::fmt::Debug for PendingImageCopy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingImageCopy")
            .field("sheet", &self.sheet)
            .field("file", &self.file)
            .field("hash", &self.hash)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "image_asset_tests.rs"]
mod tests;
