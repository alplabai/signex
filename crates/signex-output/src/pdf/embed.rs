//! Type0 / CIDFontType2 font embedding.
//!
//! While content streams are generated, [`FontUsage`] hands out a compact CID
//! per distinct (glyph, character) pair per font (CID 0 is `.notdef`), so a
//! glyph shared by several characters (Roboto's OHM SIGN and GREEK CAPITAL
//! OMEGA) still maps back to the exact character through `/ToUnicode`. After every page is built, [`write_fonts`] emits for each
//! used font a Type0 dict (`Identity-H`), a `CIDFontType2` descendant with a
//! `/W` array, a `/FontDescriptor`, a subset `/FontFile2` stream holding only
//! the used glyphs, and a `/ToUnicode` CMap so text stays searchable.

use std::collections::{BTreeSet, HashMap};

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, Str};
use ttf_parser::{Face, GlyphId};

use super::PdfError;
use super::font::PdfFont;
use super::subset::subset_truetype;

/// PDF objects written per embedded font: Type0, CIDFont, descriptor,
/// FontFile2 stream, ToUnicode CMap.
pub(super) const REFS_PER_FONT: i32 = 5;

/// Glyphs handed out so far for one font.
struct FaceUsage {
    face: Option<Face<'static>>,
    /// (Original glyph id, character) -> CID.
    cid_by_glyph_char: HashMap<(u16, char), u16>,
    /// `orig_gids[cid]` is the original glyph id; index 0 is `.notdef`.
    orig_gids: Vec<u16>,
    /// `chars[cid]` is the character the CID stands for.
    chars: Vec<char>,
    /// Set as soon as any text is encoded with this face, even text the face
    /// has no glyph for: the content stream names the face in `Tf` either way,
    /// so it must be defined in `/Font`.
    referenced: bool,
    /// Characters the face has no glyph for (drawn as `.notdef`).
    uncovered: BTreeSet<char>,
}

impl FaceUsage {
    fn new(font: PdfFont) -> Self {
        Self {
            face: font.face(),
            cid_by_glyph_char: HashMap::new(),
            orig_gids: vec![0],
            chars: vec!['\0'],
            referenced: false,
            uncovered: BTreeSet::new(),
        }
    }

    fn cid_for(&mut self, ch: char) -> u16 {
        let gid = self
            .face
            .as_ref()
            .and_then(|face| face.glyph_index(ch))
            .map_or(0, |g| g.0);
        if gid == 0 {
            if !ch.is_control() {
                self.uncovered.insert(ch);
            }
            return 0;
        }
        if let Some(&cid) = self.cid_by_glyph_char.get(&(gid, ch)) {
            return cid;
        }
        // Past 65535 distinct pairs a CID no longer fits; show `.notdef`.
        let Ok(cid) = u16::try_from(self.orig_gids.len()) else {
            self.uncovered.insert(ch);
            return 0;
        };
        self.cid_by_glyph_char.insert((gid, ch), cid);
        self.orig_gids.push(gid);
        self.chars.push(ch);
        cid
    }

    fn is_used(&self) -> bool {
        self.referenced
    }

    /// Six capitals derived from the kept glyph set and the font slot, so two
    /// documents with different subsets carry different names (PDF 32000-1
    /// 9.6.4). FNV-1a over the slot and glyph ids.
    fn subset_tag(&self, slot: usize) -> String {
        let mut hash: u32 = 0x811c_9dc5;
        let mut feed = |byte: u8| {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        };
        feed(slot as u8);
        for gid in &self.orig_gids {
            for byte in gid.to_be_bytes() {
                feed(byte);
            }
        }
        (0..6)
            .map(|i| {
                let letter = (hash >> (i * 5)) % 26;
                (b'A' + letter as u8) as char
            })
            .collect()
    }
}

/// Per-document record of which glyphs each bundled font must embed.
pub(super) struct FontUsage {
    faces: [FaceUsage; 4],
}

impl FontUsage {
    pub(super) fn new() -> Self {
        Self {
            faces: PdfFont::ALL.map(FaceUsage::new),
        }
    }

    fn slot(font: PdfFont) -> usize {
        PdfFont::ALL.iter().position(|&f| f == font).unwrap_or(0)
    }

    /// Characters no bundled face covers, across all faces, sorted and
    /// de-duplicated. They were drawn as `.notdef`.
    pub(super) fn uncovered_chars(&self) -> Vec<char> {
        let all: BTreeSet<char> = self
            .faces
            .iter()
            .flat_map(|f| f.uncovered.iter().copied())
            .collect();
        all.into_iter().collect()
    }

    /// Encode `text` as a PDF hex string of 2-byte CIDs (`<00010002>`),
    /// registering every glyph for embedding. Glyphs the font lacks encode as
    /// `.notdef` (CID 0).
    pub(super) fn encode(&mut self, font: PdfFont, text: &str) -> String {
        let face = &mut self.faces[Self::slot(font)];
        face.referenced = true;
        let mut out = String::with_capacity(text.len() * 4 + 2);
        out.push('<');
        for ch in text.chars() {
            out.push_str(&format!("{:04X}", face.cid_for(ch)));
        }
        out.push('>');
        out
    }
}

/// Font that was written to the PDF, with the ref its resources entry needs.
pub(super) struct EmbeddedFont {
    pub font: PdfFont,
    pub type0: Ref,
}

/// Write every used font's objects, starting at ref id `base`. Font `i` of
/// [`PdfFont::ALL`] owns `base + i * REFS_PER_FONT ..`.
pub(super) fn write_fonts(
    pdf: &mut Pdf,
    usage: &FontUsage,
    base: i32,
) -> Result<Vec<EmbeddedFont>, PdfError> {
    let mut written = Vec::new();
    for (slot, &font) in PdfFont::ALL.iter().enumerate() {
        let face_usage = &usage.faces[slot];
        if !face_usage.is_used() {
            continue;
        }
        let first = base + slot as i32 * REFS_PER_FONT;
        let refs = FontRefs {
            type0: Ref::new(first),
            cid: Ref::new(first + 1),
            descriptor: Ref::new(first + 2),
            file: Ref::new(first + 3),
            to_unicode: Ref::new(first + 4),
        };
        write_font(pdf, font, slot, face_usage, &refs)?;
        written.push(EmbeddedFont {
            font,
            type0: refs.type0,
        });
    }
    Ok(written)
}

struct FontRefs {
    type0: Ref,
    cid: Ref,
    descriptor: Ref,
    file: Ref,
    to_unicode: Ref,
}

fn write_font(
    pdf: &mut Pdf,
    font: PdfFont,
    slot: usize,
    usage: &FaceUsage,
    refs: &FontRefs,
) -> Result<(), PdfError> {
    let face = usage.face.as_ref().ok_or_else(|| {
        PdfError::Font(format!("bundled font {} does not parse", font.base_name()))
    })?;
    let mut keep = usage.orig_gids.clone();
    let subset = subset_truetype(font.font_bytes(), &mut keep)
        .map_err(|e| PdfError::Font(format!("{}: {e}", font.base_name())))?;

    let base_name = format!("{}+{}", usage.subset_tag(slot), font.base_name());
    let base = Name(base_name.as_bytes());
    let info = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };

    pdf.type0_font(refs.type0)
        .base_font(base)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(refs.cid)
        .to_unicode(refs.to_unicode);

    let upm = f32::from(face.units_per_em()).max(1.0);
    let to_1000 = |v: f32| v * 1000.0 / upm;
    let widths = keep
        .iter()
        .map(|&gid| to_1000(face.glyph_hor_advance(GlyphId(gid)).map_or(0.0, f32::from)));
    let mut cid = pdf.cid_font(refs.cid);
    cid.subtype(CidFontType::Type2)
        .base_font(base)
        .system_info(info)
        .font_descriptor(refs.descriptor)
        .cid_to_gid_map_predefined(Name(b"Identity"));
    cid.widths().consecutive(0, widths);
    cid.finish();

    let bbox = face.global_bounding_box();
    let mut flags = FontFlags::NON_SYMBOLIC;
    if face.is_monospaced() {
        flags |= FontFlags::FIXED_PITCH;
    }
    let mut descriptor = pdf.font_descriptor(refs.descriptor);
    descriptor
        .name(base)
        .flags(flags)
        .bbox(Rect::new(
            to_1000(f32::from(bbox.x_min)),
            to_1000(f32::from(bbox.y_min)),
            to_1000(f32::from(bbox.x_max)),
            to_1000(f32::from(bbox.y_max)),
        ))
        .italic_angle(face.italic_angle())
        .ascent(to_1000(f32::from(face.ascender())))
        .descent(to_1000(f32::from(face.descender())))
        .cap_height(to_1000(f32::from(
            face.capital_height().unwrap_or(face.ascender()),
        )))
        .stem_v(if face.is_bold() { 120.0 } else { 80.0 })
        .font_file2(refs.file);
    descriptor.finish();

    let mut file = pdf.stream(refs.file, &subset);
    file.pair(Name(b"Length1"), subset.len() as i32);
    file.finish();

    let mut cmap = UnicodeCmap::new(Name(b"Adobe-Identity-UCS"), info);
    for (cid, &ch) in usage.chars.iter().enumerate().skip(1) {
        cmap.pair(cid as u16, ch);
    }
    pdf.cmap(refs.to_unicode, &cmap.finish());
    Ok(())
}
