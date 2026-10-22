//! Type0 font embedding: Unicode text must survive export, stay extractable
//! through `/ToUnicode`, and the embedded faces must be real subsets.

use signex_types::schematic::{HAlign, Point, TextNote, VAlign};
use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag};

use super::font::PdfFont;
use super::subset::subset_truetype;
use super::tests::{empty_sheet, sample_ctx};
use super::*;

const SAMPLE: &str = "10µF 100°C ±5% Ğüşİıç Ω";

fn note(text: &str, y: f64) -> TextNote {
    TextNote {
        uuid: uuid::Uuid::nil(),
        text: text.to_string(),
        position: Point::new(20.0, y),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn export_notes(texts: &[&str]) -> Vec<u8> {
    let mut sheet = empty_sheet();
    for (i, text) in texts.iter().enumerate() {
        sheet.text_notes.push(note(text, 20.0 + 10.0 * i as f64));
    }
    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;
    let opts = PdfOptions {
        include_title_block: false,
        ..Default::default()
    };
    PdfExporter.export(&ctx, &opts).expect("export").bytes
}

/// PDF bytes as a 1:1 char-per-byte string so offsets survive binary streams.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn leading_number(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Object number a `/key N 0 R` entry points at.
fn ref_after(text: &str, key: &str) -> Option<u32> {
    text.match_indices(key).find_map(|(at, _)| {
        let rest = &text[at + key.len()..];
        let n = leading_number(rest)?;
        rest[n.to_string().len()..].starts_with(" 0 R").then_some(n)
    })
}

fn object_body(text: &str, n: u32) -> &str {
    let start = text
        .find(&format!("\n{n} 0 obj"))
        .unwrap_or_else(|| panic!("object {n} missing"));
    let end = text[start..].find("endobj").expect("endobj") + start;
    &text[start..end]
}

/// CID -> char table parsed from a `/ToUnicode` CMap body.
fn parse_cmap(body: &str) -> std::collections::HashMap<u16, char> {
    let mut map = std::collections::HashMap::new();
    let mut in_bfchar = false;
    for line in body.lines() {
        match line.trim() {
            "endbfchar" => in_bfchar = false,
            l if l.ends_with("beginbfchar") => in_bfchar = true,
            _ if !in_bfchar => continue,
            _ => {}
        }
        let mut parts = line.split(['<', '>']).filter(|p| !p.trim().is_empty());
        if let (Some(cid), Some(uni), None) = (parts.next(), parts.next(), parts.next())
            && let (Ok(cid), Ok(uni)) = (u16::from_str_radix(cid, 16), u32::from_str_radix(uni, 16))
            && let Some(ch) = char::from_u32(uni)
        {
            map.insert(cid, ch);
        }
    }
    map
}

/// Every `Tj` string in the PDF, decoded through its font's `/ToUnicode`.
/// CID 0 (`.notdef`) decodes to nothing, as a reader would extract it.
fn extract_text(bytes: &[u8]) -> Vec<String> {
    let text = latin1(bytes);
    let mut shown = Vec::new();
    let mut alias = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix('/')
            && rest.ends_with(" Tf")
        {
            alias = format!("/{}", rest.split(' ').next().unwrap_or_default());
        }
        let Some(hex) = line.strip_suffix("> Tj").and_then(|l| l.strip_prefix('<')) else {
            continue;
        };
        let type0 = ref_after(&text, &format!("{alias} ")).expect("font in resources");
        let cmap_ref = ref_after(object_body(&text, type0), "/ToUnicode ").expect("ToUnicode");
        let cmap = parse_cmap(object_body(&text, cmap_ref));
        let decoded: String = (0..hex.len() / 4)
            .filter_map(|i| u16::from_str_radix(&hex[i * 4..i * 4 + 4], 16).ok())
            .filter_map(|cid| cmap.get(&cid).copied())
            .collect();
        shown.push(decoded);
    }
    shown
}

#[test]
fn fonts_are_embedded_as_type0_with_font_file2_and_to_unicode() {
    let text = latin1(&export_notes(&[SAMPLE]));
    for needle in [
        "/Type0",
        "/CIDFontType2",
        "/Identity-H",
        "/FontFile2",
        "/ToUnicode",
        "/FontDescriptor",
        "/W [",
    ] {
        assert!(text.contains(needle), "PDF lacks {needle}");
    }
    assert!(!text.contains("Helvetica") && !text.contains("Courier"));
    assert!(text.contains("+Roboto"), "Roboto must be the base font");
}

#[test]
fn non_ascii_text_round_trips_through_to_unicode() {
    let shown = extract_text(&export_notes(&[SAMPLE, "Ω µ ü"]));
    assert!(shown.iter().any(|s| s == SAMPLE), "got {shown:?}");
    assert!(shown.iter().any(|s| s == "Ω µ ü"), "got {shown:?}");
}

#[test]
fn glyphs_present_in_the_font_never_resolve_to_notdef() {
    let bytes = export_notes(&[SAMPLE]);
    let text = latin1(&bytes);
    let tj = text
        .lines()
        .find_map(|l| l.strip_suffix("> Tj").and_then(|l| l.strip_prefix('<')))
        .expect("a Tj line");
    assert_eq!(tj.len() / 4, SAMPLE.chars().count());
    assert!(
        (0..tj.len() / 4).all(|i| &tj[i * 4..i * 4 + 4] != "0000"),
        "a glyph resolved to .notdef: {tj}"
    );
}

#[test]
fn glyph_the_font_lacks_exports_without_panicking() {
    let shown = extract_text(&export_notes(&["Ω µ ü 中"]));
    assert!(shown.iter().any(|s| s == "Ω µ ü "), "got {shown:?}");
}

#[test]
fn single_glyph_document_embeds_a_subset_not_the_whole_face() {
    let bytes = export_notes(&["A"]);
    let full = PdfFont::RobotoRegular.font_bytes().len();
    assert!(
        bytes.len() < full / 20,
        "PDF is {} bytes, face is {full}",
        bytes.len()
    );
    let text = latin1(&bytes);
    assert!(
        !text.contains("+Iosevka"),
        "unused faces must not be embedded"
    );
}

/// Resolved outline commands of a glyph (composites already flattened into
/// their components), so renumbered component references are really checked.
#[derive(Default, PartialEq, Debug)]
struct Outline(Vec<String>);

impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(format!("M{x},{y}"));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(format!("L{x},{y}"));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(format!("Q{x1},{y1},{x},{y}"));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(format!("C{x1},{y1},{x2},{y2},{x},{y}"));
    }
    fn close(&mut self) {
        self.0.push("Z".into());
    }
}

fn outline(face: &Face, gid: u16) -> (Outline, Option<ttf_parser::Rect>) {
    let mut out = Outline::default();
    let bbox = face.outline_glyph(GlyphId(gid), &mut out);
    (out, bbox)
}

#[test]
fn subset_keeps_outlines_for_simple_and_composite_glyphs() {
    let data = PdfFont::RobotoRegular.font_bytes();
    let face = Face::parse(data, 0).expect("roboto");
    let originals: Vec<u16> = "Aü°µΩ"
        .chars()
        .map(|c| face.glyph_index(c).expect("glyph").0)
        .collect();
    let mut keep = vec![0];
    keep.extend(&originals);
    let requested = keep.len();

    let subset = subset_truetype(data, &mut keep).expect("subset");
    assert!(subset.len() < data.len() / 20);
    let sub = Face::parse(&subset, 0).expect("subset parses");
    assert_eq!(usize::from(sub.number_of_glyphs()), keep.len());
    assert!(keep.len() >= requested);
    for (new, &orig) in keep.iter().enumerate() {
        assert_eq!(
            outline(&sub, new as u16),
            outline(&face, orig),
            "outline of new glyph {new} (orig {orig}) differs"
        );
        assert_eq!(
            sub.glyph_hor_advance(GlyphId(new as u16)),
            face.glyph_hor_advance(GlyphId(orig))
        );
    }
}

#[test]
fn every_bundled_face_subsets() {
    for font in PdfFont::ALL {
        let face = font.face().expect("parses");
        let gid = face.glyph_index('A').expect("A").0;
        let mut keep = vec![0, gid];
        let subset = subset_truetype(font.font_bytes(), &mut keep)
            .unwrap_or_else(|e| panic!("{}: {e}", font.base_name()));
        assert!(Face::parse(&subset, 0).is_ok(), "{}", font.base_name());
    }
}

#[test]
fn glyph_shared_by_two_characters_keeps_each_character_in_to_unicode() {
    let face = PdfFont::RobotoRegular.face().expect("roboto");
    let (ohm, omega) = ('\u{2126}', '\u{03A9}');
    assert_eq!(
        face.glyph_index(ohm),
        face.glyph_index(omega),
        "premise: Roboto draws OHM SIGN and GREEK CAPITAL OMEGA with one glyph"
    );
    let shown = extract_text(&export_notes(&["\u{2126}", "\u{03A9}"]));
    assert!(shown.iter().any(|s| s == "\u{2126}"), "got {shown:?}");
    assert!(shown.iter().any(|s| s == "\u{03A9}"), "got {shown:?}");
}

#[test]
fn subset_carries_the_tables_strict_readers_expect() {
    let data = PdfFont::RobotoRegular.font_bytes();
    let gid = Face::parse(data, 0)
        .expect("roboto")
        .glyph_index('A')
        .expect("A")
        .0;
    let mut keep = vec![0, gid, gid];
    let subset = subset_truetype(data, &mut keep).expect("subset");
    let raw = ttf_parser::RawFace::parse(&subset, 0).expect("raw");
    for tag in [b"OS/2", b"name", b"post"] {
        assert!(raw.table(Tag::from_bytes(tag)).is_some(), "missing {tag:?}");
    }
    let post = raw.table(Tag::from_bytes(b"post")).expect("post");
    assert_eq!(&post[..4], &0x0003_0000u32.to_be_bytes());
    let sub = Face::parse(&subset, 0).expect("parses");
    assert_eq!(
        usize::from(sub.number_of_glyphs()),
        3,
        "duplicate CIDs keep their own glyph"
    );
}

/// Every `/Fn size Tf` in the file must have a `/Fn N 0 R` entry in some
/// `/Font` resources dictionary, or readers report an undefined font.
fn assert_every_used_font_is_defined(bytes: &[u8]) {
    let text = latin1(bytes);
    let mut used = Vec::new();
    for (at, _) in text.match_indices(" Tf\n") {
        let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        let alias = text[line_start..at].split_whitespace().next().unwrap_or("");
        used.push(alias.trim_start_matches('/').to_string());
    }
    assert!(!used.is_empty(), "premise: the page shows text");
    for alias in used {
        let defined = text.match_indices(&format!("/{alias} ")).any(|(at, pat)| {
            let rest = &text[at + pat.len()..];
            leading_number(rest).is_some_and(|n| rest[n.to_string().len()..].starts_with(" 0 R"))
        });
        assert!(defined, "/{alias} is used by Tf but not defined in /Font");
    }
}

#[test]
fn a_cjk_only_note_still_defines_the_font_it_selects() {
    assert_every_used_font_is_defined(&export_notes(&["中文"]));
}

#[test]
fn every_builtin_template_with_an_empty_title_defines_all_used_fonts() {
    for id in crate::template::builtin::all_builtin_ids() {
        let ctx = sample_ctx(1);
        assert!(ctx.metadata.title.is_empty(), "premise: empty title");
        let opts = PdfOptions {
            include_title_block: true,
            sheet_template: Some(id.clone()),
            ..Default::default()
        };
        let out = PdfExporter.export(&ctx, &opts).expect("export");
        if latin1(&out.bytes).contains(" Tf\n") {
            assert_every_used_font_is_defined(&out.bytes);
        }
    }
}

#[test]
fn a_font_with_only_notdef_glyphs_is_embedded() {
    let bytes = export_notes(&["中文"]);
    let text = latin1(&bytes);
    assert!(
        text.contains("/Type0"),
        "the referenced face must be written"
    );
}

#[test]
fn subset_tags_are_six_capitals_derived_from_the_glyph_set() {
    fn tags(bytes: &[u8]) -> Vec<String> {
        let text = latin1(bytes);
        text.match_indices("/BaseFont /")
            .map(|(at, pat)| {
                text[at + pat.len()..]
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    }
    let one = tags(&export_notes(&["abc"]));
    let other = tags(&export_notes(&["abc", "xyz"]));
    assert!(!one.is_empty());
    for tag in one.iter().chain(&other) {
        let (prefix, _) = tag.split_once('+').expect("subset tag has a plus");
        assert!(
            prefix.len() == 6 && prefix.chars().all(|c| c.is_ascii_uppercase()),
            "bad subset tag {tag}"
        );
    }
    assert_ne!(one, other, "different glyph sets must carry different tags");
}

#[test]
fn uncovered_characters_are_reported_to_the_caller() {
    let mut sheet = empty_sheet();
    sheet.text_notes.push(note("a中文中", 20.0));
    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;
    let opts = PdfOptions {
        include_title_block: false,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    assert_eq!(out.uncovered_chars, vec!['中', '文']);

    let clean = PdfExporter.export(&sample_ctx(1), &opts).expect("export");
    assert!(clean.uncovered_chars.is_empty());
}

#[test]
fn layout_uses_the_notdef_advance_for_uncovered_characters() {
    let face = PdfFont::RobotoRegular.face().expect("roboto");
    let upm = f32::from(face.units_per_em());
    let notdef = f32::from(face.glyph_hor_advance(GlyphId(0)).expect("notdef"));
    let got = font::text_advance_pt("F1", "中", 10.0);
    assert!((got - notdef * 10.0 / upm).abs() < 1e-3, "got {got}");
}

#[test]
fn subset_fixes_the_whole_font_checksum() {
    let data = PdfFont::RobotoRegular.font_bytes();
    let mut keep = vec![0];
    let subset = subset_truetype(data, &mut keep).expect("subset");
    let sum = subset.chunks(4).fold(0u32, |s, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(w))
    });
    assert_eq!(sum, 0xB1B0_AFBA);
}
