//! TrueType (`glyf`) subsetting for PDF `/FontFile2` streams.
//!
//! The PDF writer assigns each used glyph a compact id (CID = new glyph id)
//! while it emits content streams, so the subset renumbers glyphs: new glyph
//! `i` is original glyph `keep[i]`. Composite glyphs pull in their components,
//! which are appended to `keep` and renumbered too. Only the tables a PDF
//! reader needs to rasterise outlines are written (`head`, `hhea`, `maxp`,
//! `hmtx`, `loca`, `glyf`, plus the hinting tables, `OS/2` and `name` when
//! present, and a version 3.0 `post`); there is no `cmap` because Identity-H
//! CIDs address glyphs directly (`/CIDToGIDMap /Identity`: CID `i` is new
//! glyph `i`). `keep` may list one original glyph under several CIDs; each
//! gets its own copy so the identity mapping stays exact.

use ttf_parser::{RawFace, Tag};

const HEAD: Tag = Tag::from_bytes(b"head");
const HHEA: Tag = Tag::from_bytes(b"hhea");
const MAXP: Tag = Tag::from_bytes(b"maxp");
const HMTX: Tag = Tag::from_bytes(b"hmtx");
const LOCA: Tag = Tag::from_bytes(b"loca");
const GLYF: Tag = Tag::from_bytes(b"glyf");
const POST: Tag = Tag::from_bytes(b"post");
const OPTIONAL_TABLES: [Tag; 5] = [
    Tag::from_bytes(b"OS/2"),
    Tag::from_bytes(b"name"),
    Tag::from_bytes(b"cvt "),
    Tag::from_bytes(b"fpgm"),
    Tag::from_bytes(b"prep"),
];

const ARG_WORDS: u16 = 0x0001;
const HAVE_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const HAVE_XY_SCALE: u16 = 0x0040;
const HAVE_2X2: u16 = 0x0080;

/// Source tables borrowed from the parsed face.
struct Source<'a> {
    head: &'a [u8],
    hhea: &'a [u8],
    maxp: &'a [u8],
    hmtx: &'a [u8],
    loca: &'a [u8],
    glyf: &'a [u8],
    long_loca: bool,
    num_h_metrics: usize,
    num_glyphs: usize,
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

impl<'a> Source<'a> {
    fn parse(raw: &RawFace<'a>) -> Result<Self, String> {
        let table = |tag: Tag| {
            raw.table(tag)
                .ok_or_else(|| format!("font has no `{tag}` table (not TrueType outlines)"))
        };
        let head = table(HEAD)?;
        let hhea = table(HHEA)?;
        let maxp = table(MAXP)?;
        let bad = || "truncated font table".to_string();
        Ok(Self {
            long_loca: be16(head, 50).ok_or_else(bad)? != 0,
            num_h_metrics: be16(hhea, 34).ok_or_else(bad)? as usize,
            num_glyphs: be16(maxp, 4).ok_or_else(bad)? as usize,
            hmtx: table(HMTX)?,
            loca: table(LOCA)?,
            glyf: table(GLYF)?,
            head,
            hhea,
            maxp,
        })
    }

    /// Outline bytes of an original glyph (empty for blank glyphs).
    fn glyph(&self, gid: u16) -> Result<&'a [u8], String> {
        let gid = gid as usize;
        if gid >= self.num_glyphs {
            return Err(format!("glyph {gid} out of range"));
        }
        let bad = || "truncated loca table".to_string();
        let (start, end) = if self.long_loca {
            (
                be32(self.loca, gid * 4).ok_or_else(bad)? as usize,
                be32(self.loca, gid * 4 + 4).ok_or_else(bad)? as usize,
            )
        } else {
            (
                be16(self.loca, gid * 2).ok_or_else(bad)? as usize * 2,
                be16(self.loca, gid * 2 + 2).ok_or_else(bad)? as usize * 2,
            )
        };
        if start >= end {
            return Ok(&[]);
        }
        self.glyf
            .get(start..end)
            .ok_or_else(|| "glyph outside glyf table".to_string())
    }

    /// `(advance, left side bearing)` of an original glyph.
    fn metrics(&self, gid: u16) -> Result<(u16, i16), String> {
        let gid = gid as usize;
        let bad = || "truncated hmtx table".to_string();
        if self.num_h_metrics == 0 {
            return Err(bad());
        }
        if gid < self.num_h_metrics {
            let adv = be16(self.hmtx, gid * 4).ok_or_else(bad)?;
            let lsb = be16(self.hmtx, gid * 4 + 2).ok_or_else(bad)? as i16;
            return Ok((adv, lsb));
        }
        let adv = be16(self.hmtx, (self.num_h_metrics - 1) * 4).ok_or_else(bad)?;
        let at = self.num_h_metrics * 4 + (gid - self.num_h_metrics) * 2;
        Ok((adv, be16(self.hmtx, at).ok_or_else(bad)? as i16))
    }
}

/// Byte offsets of the component glyph ids inside a composite outline.
fn component_offsets(glyph: &[u8]) -> Result<Vec<usize>, String> {
    let bad = || "truncated composite glyph".to_string();
    let mut offsets = Vec::new();
    if glyph.len() < 10 || (be16(glyph, 0).ok_or_else(bad)? as i16) >= 0 {
        return Ok(offsets);
    }
    let mut at = 10;
    loop {
        let flags = be16(glyph, at).ok_or_else(bad)?;
        offsets.push(at + 2);
        at += 4 + if flags & ARG_WORDS != 0 { 4 } else { 2 };
        at += if flags & HAVE_SCALE != 0 {
            2
        } else if flags & HAVE_XY_SCALE != 0 {
            4
        } else if flags & HAVE_2X2 != 0 {
            8
        } else {
            0
        };
        if flags & MORE_COMPONENTS == 0 {
            return Ok(offsets);
        }
    }
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// Build a subset font. `keep[i]` is the original glyph for new glyph `i`
/// (`keep[0]` must be `.notdef`). Component glyphs of composites are appended
/// to `keep`, so on success `keep.len()` is the subset's glyph count.
pub(super) fn subset_truetype(data: &[u8], keep: &mut Vec<u16>) -> Result<Vec<u8>, String> {
    let raw = RawFace::parse(data, 0).map_err(|e| format!("unparseable font: {e}"))?;
    let src = Source::parse(&raw)?;
    let truncated = || "truncated composite glyph".to_string();

    // Pass 1: close over composite components, appending as found.
    let mut i = 0;
    while i < keep.len() {
        let glyph = src.glyph(keep[i])?;
        for at in component_offsets(glyph)? {
            let component = be16(glyph, at).ok_or_else(truncated)?;
            if !keep.contains(&component) {
                keep.push(component);
            }
        }
        i += 1;
    }
    if keep.len() > u16::MAX as usize {
        return Err("subset exceeds 65535 glyphs".into());
    }

    // Pass 2: emit renumbered glyf/loca/hmtx.
    let (mut glyf, mut loca, mut hmtx) = (Vec::new(), Vec::new(), Vec::new());
    for &orig in keep.iter() {
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let mut glyph = src.glyph(orig)?.to_vec();
        for at in component_offsets(&glyph)? {
            let component = be16(&glyph, at).ok_or_else(truncated)?;
            let renumbered = keep.iter().position(|&g| g == component).unwrap_or(0) as u16;
            glyph[at..at + 2].copy_from_slice(&renumbered.to_be_bytes());
        }
        glyf.extend_from_slice(&glyph);
        glyf.resize(glyf.len().next_multiple_of(4), 0);
        let (adv, lsb) = src.metrics(orig)?;
        hmtx.extend_from_slice(&adv.to_be_bytes());
        hmtx.extend_from_slice(&lsb.to_be_bytes());
    }
    loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());

    let count = keep.len() as u16;
    let mut head = src.head.to_vec();
    head[8..12].copy_from_slice(&0u32.to_be_bytes());
    head[50..52].copy_from_slice(&1i16.to_be_bytes());
    let mut hhea = src.hhea.to_vec();
    hhea[34..36].copy_from_slice(&count.to_be_bytes());
    let mut maxp = src.maxp.to_vec();
    maxp[4..6].copy_from_slice(&count.to_be_bytes());

    let mut tables: Vec<(Tag, Vec<u8>)> = vec![
        (GLYF, glyf),
        (HEAD, head),
        (HHEA, hhea),
        (HMTX, hmtx),
        (LOCA, loca),
        (MAXP, maxp),
    ];
    for tag in OPTIONAL_TABLES {
        if let Some(table) = raw.table(tag) {
            tables.push((tag, table.to_vec()));
        }
    }
    // `post` keeps the source's metrics but drops glyph names (version 3.0),
    // which would otherwise describe the pre-renumbering glyph order.
    if let Some(post) = raw.table(POST)
        && post.len() >= 32
    {
        let mut post = post[..32].to_vec();
        post[..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
        tables.push((POST, post));
    }
    tables.sort_by_key(|(tag, _)| tag.0);
    let mut font = assemble(&tables);
    fix_checksum_adjustment(&mut font);
    Ok(font)
}

/// Set `head.checkSumAdjustment` to `0xB1B0AFBA` minus the whole-font checksum.
/// The field is 0 while the checksum is taken, as the spec requires.
fn fix_checksum_adjustment(font: &mut [u8]) {
    let head_offset = (0..be16(font, 4).unwrap_or(0) as usize).find_map(|i| {
        let entry = 12 + i * 16;
        (font.get(entry..entry + 4)? == HEAD.0.to_be_bytes()).then(|| be32(font, entry + 8))?
    });
    let Some(head_offset) = head_offset else {
        return;
    };
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(font));
    if let Some(slot) = font.get_mut(head_offset as usize + 8..head_offset as usize + 12) {
        slot.copy_from_slice(&adjustment.to_be_bytes());
    }
}

/// Lay out an sfnt container: directory followed by 4-byte-aligned tables.
fn assemble(tables: &[(Tag, Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let entry_selector = n.ilog2() as u16;
    let search_range = (1u16 << entry_selector) * 16;
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&(n * 16 - search_range).to_be_bytes());

    let mut offset = 12 + tables.len() * 16;
    for (tag, table) in tables {
        out.extend_from_slice(&tag.0.to_be_bytes());
        out.extend_from_slice(&checksum(table).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(table.len() as u32).to_be_bytes());
        offset += table.len().next_multiple_of(4);
    }
    for (_, table) in tables {
        out.extend_from_slice(table);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    out
}
