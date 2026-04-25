//! `.snxsht` S-expression parser + emitter.
//!
//! Format mirrors KiCad's `.kicad_*` style — flat S-expression tree —
//! so we can reuse `kicad-parser::sexpr` for tokenisation and tree
//! shape, and write the emitter as plain string formatting (the
//! template tree is small enough that the AST helper isn't worth it).
//!
//! Grammar:
//!
//! ```text
//! (signex_template
//!   (id "iso_a4_landscape")
//!   (display "ISO A4 landscape")
//!   (page "A4")           ; one of: A0..A5, A..E, USLetter, USLegal,
//!                         ;         or (page custom <w_mm> <h_mm>)
//!   (orientation portrait | landscape)
//!   (frame
//!     (border_margin_mm 10.0)
//!     (zone_markers yes | no)
//!     (horizontal_zones 8)
//!     (vertical_zones 6))
//!   (title_block
//!     (size 180.0 32.0)
//!     (field "Title" 30.0 4.0 "Roboto" 4.5 bold "${TITLE}")
//!     ...))
//! ```
//!
//! `field` positional args: `name x_mm y_mm font_family font_size_mm
//! font_style default_text`. `font_style` is one of
//! `normal | bold | italic | bold_italic`.

use kicad_parser::sexpr::{SExpr, parse as parse_sexpr};

use super::{FontStyle, Frame, Template, TemplateId, TitleBlock, TitleBlockField};
use crate::pdf::{Orientation, PageSize};

#[derive(Debug, thiserror::Error)]
pub enum SnxshtError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid template: {0}")]
    Invalid(String),
}

/// Parse a `.snxsht` source string into a `Template`. The id supplied
/// here is the project-relative path (or any opaque key the loader
/// uses to refer back to this template); the file's own `(id ...)`
/// node is honoured when present and overrides this argument.
pub fn parse_template(source: &str, fallback_id: &str) -> Result<Template, SnxshtError> {
    let root = parse_sexpr(source).map_err(SnxshtError::Parse)?;
    if root.keyword() != Some("signex_template") {
        return Err(SnxshtError::Invalid(format!(
            "expected (signex_template ...), got {:?}",
            root.keyword()
        )));
    }

    let id = root
        .find("id")
        .and_then(|n| n.first_arg())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback_id.to_string());
    let display_name = root
        .find("display")
        .and_then(|n| n.first_arg())
        .map(|s| s.to_string())
        .unwrap_or_else(|| id.clone());

    let page_size = parse_page_size(&root)?;
    let orientation = parse_orientation(&root)?;
    let frame = parse_frame(&root)?;
    let title_block = parse_title_block(&root)?;

    Ok(Template {
        id: TemplateId(id),
        display_name,
        page_size,
        orientation,
        frame,
        title_block,
    })
}

fn parse_page_size(root: &SExpr) -> Result<PageSize, SnxshtError> {
    let node = root
        .find("page")
        .ok_or_else(|| SnxshtError::Invalid("missing (page ...)".to_string()))?;
    // Two shapes:
    //   (page "A4")
    //   (page custom 210.0 297.0)
    if let Some(first) = node.first_arg() {
        if first.eq_ignore_ascii_case("custom") {
            let w = node.arg_f64(1).ok_or_else(|| {
                SnxshtError::Invalid("(page custom ...) missing width".to_string())
            })?;
            let h = node.arg_f64(2).ok_or_else(|| {
                SnxshtError::Invalid("(page custom ...) missing height".to_string())
            })?;
            return Ok(PageSize::Custom {
                width_mm: w,
                height_mm: h,
            });
        }
        Ok(PageSize::from_kicad_str(first))
    } else {
        Err(SnxshtError::Invalid("(page ...) missing argument".to_string()))
    }
}

fn parse_orientation(root: &SExpr) -> Result<Orientation, SnxshtError> {
    match root.find("orientation").and_then(|n| n.first_arg()) {
        Some(s) if s.eq_ignore_ascii_case("portrait") => Ok(Orientation::Portrait),
        Some(s) if s.eq_ignore_ascii_case("landscape") => Ok(Orientation::Landscape),
        Some(other) => Err(SnxshtError::Invalid(format!(
            "(orientation ...) must be portrait or landscape, got {other}"
        ))),
        None => Ok(Orientation::Landscape),
    }
}

fn parse_frame(root: &SExpr) -> Result<Frame, SnxshtError> {
    let Some(node) = root.find("frame") else {
        return Ok(Frame::default());
    };
    let mut frame = Frame::default();
    if let Some(v) = node.find("border_margin_mm").and_then(|n| n.arg_f64(0)) {
        frame.border_margin_mm = v;
    }
    if let Some(v) = node.find("zone_markers").and_then(|n| n.first_arg()) {
        frame.show_zone_markers = parse_yes_no(v)
            .ok_or_else(|| SnxshtError::Invalid(format!("zone_markers must be yes/no, got {v}")))?;
    }
    if let Some(v) = node
        .find("horizontal_zones")
        .and_then(|n| n.first_arg())
        .and_then(|s| s.parse::<u8>().ok())
    {
        frame.horizontal_zones = v;
    }
    if let Some(v) = node
        .find("vertical_zones")
        .and_then(|n| n.first_arg())
        .and_then(|s| s.parse::<u8>().ok())
    {
        frame.vertical_zones = v;
    }
    Ok(frame)
}

fn parse_title_block(root: &SExpr) -> Result<TitleBlock, SnxshtError> {
    let Some(node) = root.find("title_block") else {
        return Ok(TitleBlock {
            width_mm: 0.0,
            height_mm: 0.0,
            fields: Vec::new(),
        });
    };
    let (width_mm, height_mm) = match node.find("size") {
        Some(size) => (
            size.arg_f64(0).ok_or_else(|| {
                SnxshtError::Invalid("(size ...) missing width".to_string())
            })?,
            size.arg_f64(1).ok_or_else(|| {
                SnxshtError::Invalid("(size ...) missing height".to_string())
            })?,
        ),
        None => (0.0, 0.0),
    };

    let mut fields = Vec::new();
    for field_node in node.find_all("field") {
        fields.push(parse_field(field_node)?);
    }

    Ok(TitleBlock {
        width_mm,
        height_mm,
        fields,
    })
}

fn parse_field(node: &SExpr) -> Result<TitleBlockField, SnxshtError> {
    // (field "Title" 30.0 4.0 "Roboto" 4.5 bold "${TITLE}")
    let name = node
        .arg(0)
        .ok_or_else(|| SnxshtError::Invalid("(field ...) missing name".to_string()))?
        .to_string();
    let x_mm = node
        .arg_f64(1)
        .ok_or_else(|| SnxshtError::Invalid(format!("field {name}: missing x_mm")))?;
    let y_mm = node
        .arg_f64(2)
        .ok_or_else(|| SnxshtError::Invalid(format!("field {name}: missing y_mm")))?;
    let font_family = node
        .arg(3)
        .ok_or_else(|| SnxshtError::Invalid(format!("field {name}: missing font_family")))?
        .to_string();
    let font_size_mm = node
        .arg_f64(4)
        .ok_or_else(|| SnxshtError::Invalid(format!("field {name}: missing font_size_mm")))?;
    let font_style = parse_font_style(
        node.arg(5)
            .ok_or_else(|| SnxshtError::Invalid(format!("field {name}: missing font_style")))?,
    )?;
    let default_text = node.arg(6).unwrap_or("").to_string();

    Ok(TitleBlockField {
        name,
        x_mm,
        y_mm,
        font_family,
        font_size_mm,
        font_style,
        default_text,
    })
}

fn parse_font_style(s: &str) -> Result<FontStyle, SnxshtError> {
    match s.to_ascii_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "bold" => Ok(FontStyle::Bold),
        "italic" => Ok(FontStyle::Italic),
        "bold_italic" | "bolditalic" => Ok(FontStyle::BoldItalic),
        other => Err(SnxshtError::Invalid(format!(
            "unknown font_style {other}"
        ))),
    }
}

fn parse_yes_no(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "yes" | "true" => Some(true),
        "no" | "false" => Some(false),
        _ => None,
    }
}

/// Render a `Template` to its `.snxsht` string form. The output is
/// pretty-printed (one node per line for readability); round-trips
/// through `parse_template` cleanly.
pub fn emit_template(template: &Template) -> String {
    let mut out = String::new();
    out.push_str("(signex_template\n");
    out.push_str(&format!("  (id {})\n", quote(&template.id.0)));
    out.push_str(&format!(
        "  (display {})\n",
        quote(&template.display_name)
    ));
    out.push_str(&emit_page(&template.page_size));
    out.push_str(&format!(
        "  (orientation {})\n",
        match template.orientation {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    ));
    out.push_str(&emit_frame(&template.frame));
    out.push_str(&emit_title_block(&template.title_block));
    out.push_str(")\n");
    out
}

fn emit_page(page: &PageSize) -> String {
    match page {
        PageSize::IsoA0 => "  (page \"A0\")\n".to_string(),
        PageSize::IsoA1 => "  (page \"A1\")\n".to_string(),
        PageSize::IsoA2 => "  (page \"A2\")\n".to_string(),
        PageSize::IsoA3 => "  (page \"A3\")\n".to_string(),
        PageSize::IsoA4 => "  (page \"A4\")\n".to_string(),
        PageSize::IsoA5 => "  (page \"A5\")\n".to_string(),
        PageSize::AnsiA => "  (page \"A\")\n".to_string(),
        PageSize::AnsiB => "  (page \"B\")\n".to_string(),
        PageSize::AnsiC => "  (page \"C\")\n".to_string(),
        PageSize::AnsiD => "  (page \"D\")\n".to_string(),
        PageSize::AnsiE => "  (page \"E\")\n".to_string(),
        PageSize::UsLetter => "  (page \"USLetter\")\n".to_string(),
        PageSize::UsLegal => "  (page \"USLegal\")\n".to_string(),
        PageSize::Custom {
            width_mm,
            height_mm,
        } => format!("  (page custom {} {})\n", fnum(*width_mm), fnum(*height_mm)),
    }
}

fn emit_frame(frame: &Frame) -> String {
    let mut s = String::from("  (frame\n");
    s.push_str(&format!(
        "    (border_margin_mm {})\n",
        fnum(frame.border_margin_mm),
    ));
    s.push_str(&format!(
        "    (zone_markers {})\n",
        if frame.show_zone_markers { "yes" } else { "no" },
    ));
    s.push_str(&format!(
        "    (horizontal_zones {})\n",
        frame.horizontal_zones,
    ));
    s.push_str(&format!(
        "    (vertical_zones {}))\n",
        frame.vertical_zones,
    ));
    s
}

fn emit_title_block(tb: &TitleBlock) -> String {
    if tb.width_mm == 0.0 && tb.height_mm == 0.0 && tb.fields.is_empty() {
        return String::new();
    }
    let mut s = String::from("  (title_block\n");
    s.push_str(&format!(
        "    (size {} {})\n",
        fnum(tb.width_mm),
        fnum(tb.height_mm),
    ));
    for f in &tb.fields {
        s.push_str(&format!(
            "    (field {} {} {} {} {} {} {})\n",
            quote(&f.name),
            fnum(f.x_mm),
            fnum(f.y_mm),
            quote(&f.font_family),
            fnum(f.font_size_mm),
            match f.font_style {
                FontStyle::Normal => "normal",
                FontStyle::Bold => "bold",
                FontStyle::Italic => "italic",
                FontStyle::BoldItalic => "bold_italic",
            },
            quote(&f.default_text),
        ));
    }
    s.push_str("  )\n");
    s
}

fn quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Format an f64 with the minimum decimals that round-trips. KiCad
/// uses up to 6 decimals; we match. Trailing zeroes are trimmed so
/// integers render without a dot.
fn fnum(v: f64) -> String {
    let s = format!("{v:.6}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

// Suppress unused-warning when the parser's not yet wired into the
// template loader. Re-exported via `template::mod` so callers can
// pick the parser up immediately.
#[allow(dead_code)]
fn _force_used() {
    let _ = parse_template;
    let _ = emit_template;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_a4_landscape() {
        let original = super::super::builtin::load_builtin(&TemplateId::from(
            "iso_a4_landscape",
        ))
        .expect("a4 landscape builtin");
        let s = emit_template(&original);
        let parsed = parse_template(&s, "iso_a4_landscape").expect("parse round-trip");
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.page_size, original.page_size);
        assert_eq!(parsed.orientation, original.orientation);
        assert_eq!(parsed.frame, original.frame);
        assert_eq!(parsed.title_block.fields.len(), original.title_block.fields.len());
    }

    #[test]
    fn parses_minimal_template() {
        let s = r#"
        (signex_template
          (id "test")
          (page "A4")
          (orientation landscape))
        "#;
        let t = parse_template(s, "test").expect("minimal parse");
        assert_eq!(t.id.0, "test");
        assert_eq!(t.page_size, PageSize::IsoA4);
        assert_eq!(t.orientation, Orientation::Landscape);
    }

    #[test]
    fn parses_custom_page_size() {
        let s = r#"
        (signex_template
          (id "tabloid")
          (page custom 431.8 279.4)
          (orientation landscape))
        "#;
        let t = parse_template(s, "tabloid").unwrap();
        match t.page_size {
            PageSize::Custom {
                width_mm,
                height_mm,
            } => {
                assert!((width_mm - 431.8).abs() < 1e-6);
                assert!((height_mm - 279.4).abs() < 1e-6);
            }
            _ => panic!("expected custom page size"),
        }
    }

    #[test]
    fn rejects_wrong_root() {
        let err = parse_template("(some_other_root)", "x").unwrap_err();
        assert!(matches!(err, SnxshtError::Invalid(_)));
    }
}
