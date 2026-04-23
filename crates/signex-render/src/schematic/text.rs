//! Text rendering -- TextNote and TextProp (reference / value fields).

use iced::Color;
use iced::widget::canvas;
use std::collections::HashMap;

use signex_types::markup::{
    ExpressionEvalContext, RichSegment, evaluate_expressions, parse_markup,
};
use signex_types::schematic::{HAlign, Symbol, TextNote, TextProp, VAlign};

use super::{ScreenTransform, field_effective_style};

pub fn display_text_content(input: &str) -> String {
    fn overbar_text(text: &str) -> String {
        let mut out = String::new();
        for ch in text.chars() {
            out.push(ch);
            out.push('\u{0305}');
        }
        out
    }

    // KiCad escapes characters with path/markup significance as {name} tokens
    // (e.g. `{slash}` for `/`). Expand before parsing markup so the literal
    // characters appear in the rendered glyphs instead of the escape source.
    // Also fold backslash-escapes (`\n` → newline, `\\` → backslash) that
    // KiCad uses inside multi-line text notes.
    let expanded = expand_backslash_escapes(&expand_char_escapes(input));

    let segments = parse_markup(&expanded);
    if segments.is_empty() {
        return expanded;
    }

    let mut out = String::new();
    for segment in segments {
        match segment {
            RichSegment::Normal(text)
            | RichSegment::Subscript(text)
            | RichSegment::Superscript(text) => out.push_str(&text),
            RichSegment::Overbar(text) => out.push_str(&overbar_text(&text)),
        }
    }
    out
}

#[derive(Clone)]
struct RichRun {
    text: String,
    scale: f32,
    baseline_offset: f32,
}

fn rich_runs(input: &str) -> Vec<RichRun> {
    let expanded = expand_backslash_escapes(&expand_char_escapes(input));
    let segments = parse_markup(&expanded);
    if segments.is_empty() {
        return vec![RichRun {
            text: expanded,
            scale: 1.0,
            baseline_offset: 0.0,
        }];
    }

    segments
        .into_iter()
        .map(|segment| match segment {
            RichSegment::Normal(text) | RichSegment::Overbar(text) => RichRun {
                text,
                scale: 1.0,
                baseline_offset: 0.0,
            },
            RichSegment::Subscript(text) => RichRun {
                text,
                scale: 0.72,
                baseline_offset: 0.26,
            },
            RichSegment::Superscript(text) => RichRun {
                text,
                scale: 0.72,
                baseline_offset: -0.34,
            },
        })
        .filter(|run| !run.text.is_empty())
        .collect()
}

fn symbol_eval_variables(sym: &Symbol) -> HashMap<String, String> {
    let mut vars = sym.fields.clone();
    for prop in &sym.custom_properties {
        if !prop.key.is_empty() {
            vars.insert(prop.key.clone(), prop.value.clone());
        }
    }
    vars.entry("refdes".to_string())
        .or_insert_with(|| sym.reference.clone());
    vars.entry("reference".to_string())
        .or_insert_with(|| sym.reference.clone());
    vars.entry("value".to_string())
        .or_insert_with(|| sym.value.clone());
    vars
}

pub fn evaluate_symbol_text(content: &str, sym: &Symbol, current_pin: Option<&str>) -> String {
    let at_vars = symbol_eval_variables(sym);
    let mut refdes_vars = HashMap::new();
    if !sym.uuid.is_nil() && !sym.reference.is_empty() {
        refdes_vars.insert(sym.uuid.to_string(), sym.reference.clone());
    }

    let ctx = ExpressionEvalContext {
        current_refdes: (!sym.reference.is_empty()).then_some(sym.reference.as_str()),
        current_value: (!sym.value.is_empty()).then_some(sym.value.as_str()),
        current_pin,
        at_variables: Some(&at_vars),
        refdes_variables: Some(&refdes_vars),
        ..ExpressionEvalContext::default()
    };
    evaluate_expressions(content, &ctx)
}

pub fn draw_rich_text(
    frame: &mut canvas::Frame,
    input: &str,
    anchor: iced::Point,
    color: Color,
    size: f32,
    h_align: iced::alignment::Horizontal,
    v_align: iced::alignment::Vertical,
    rotation_rad: f32,
) {
    if input.is_empty() || size < 1.0 {
        return;
    }

    let runs = rich_runs(input);
    let total_w: f32 = runs
        .iter()
        .map(|run| run.text.chars().count() as f32 * size * run.scale * 0.6)
        .sum();

    let mut cursor_x = match h_align {
        iced::alignment::Horizontal::Left => anchor.x,
        iced::alignment::Horizontal::Center => anchor.x - total_w * 0.5,
        iced::alignment::Horizontal::Right => anchor.x - total_w,
    };

    let base_y = match v_align {
        iced::alignment::Vertical::Top => anchor.y + size * 0.8,
        iced::alignment::Vertical::Center => anchor.y + size * 0.3,
        iced::alignment::Vertical::Bottom => anchor.y - size * 0.2,
    };

    for run in runs {
        let run_size = size * run.scale;
        let run_y = base_y + size * run.baseline_offset;
        let text = canvas::Text {
            content: run.text.clone(),
            position: iced::Point::new(cursor_x, run_y),
            color,
            size: iced::Pixels(run_size),
            font: crate::canvas_font(),
            align_x: iced::alignment::Horizontal::Left.into(),
            align_y: iced::alignment::Vertical::Bottom,
            ..canvas::Text::default()
        };

        if rotation_rad.abs() > 0.001 {
            use iced::widget::canvas::path::lyon_path::math as lyon_math;
            let t = lyon_math::Transform::identity()
                .then_translate(lyon_math::Vector::new(-anchor.x, -anchor.y))
                .then_rotate(lyon_math::Angle::radians(rotation_rad))
                .then_translate(lyon_math::Vector::new(anchor.x, anchor.y));
            text.draw_with(|path, fill| {
                let rotated = path.transform(&t);
                frame.fill(&rotated, fill);
            });
        } else {
            frame.fill_text(text);
        }

        cursor_x += run.text.chars().count() as f32 * run_size * 0.6;
    }
}

/// Plain display string + ordered list of `(start_char_idx, char_count)` pairs
/// identifying overbar regions. Used by renderers that draw the overbar as a
/// separate stroke (with a visible gap above the glyphs) instead of relying on
/// the combining-overline U+0305 which sits flush to the cap-height.
pub fn display_text_with_overbars(input: &str) -> (String, Vec<(usize, usize)>) {
    let expanded = expand_char_escapes(input);
    let segments = parse_markup(&expanded);
    if segments.is_empty() {
        return (expanded, Vec::new());
    }

    let mut plain = String::new();
    let mut overbars: Vec<(usize, usize)> = Vec::new();
    let mut char_cursor: usize = 0;
    for segment in segments {
        match segment {
            RichSegment::Normal(text)
            | RichSegment::Subscript(text)
            | RichSegment::Superscript(text) => {
                let n = text.chars().count();
                plain.push_str(&text);
                char_cursor += n;
            }
            RichSegment::Overbar(text) => {
                let n = text.chars().count();
                overbars.push((char_cursor, n));
                plain.push_str(&text);
                char_cursor += n;
            }
        }
    }
    (plain, overbars)
}

/// Count the number of glyphs that will actually render for `input` — char
/// escapes resolved, markup braces stripped. Used for width estimation in
/// label/port geometry so the body rectangle matches the visible text.
pub fn visible_char_count(input: &str) -> usize {
    let expanded = expand_char_escapes(input);
    let segments = parse_markup(&expanded);
    if segments.is_empty() {
        return expanded.chars().count();
    }
    segments
        .iter()
        .map(|s| match s {
            RichSegment::Normal(t)
            | RichSegment::Subscript(t)
            | RichSegment::Superscript(t)
            | RichSegment::Overbar(t) => t.chars().count(),
        })
        .sum()
}

/// Expand KiCad backslash escapes used inside text-note / multi-line fields:
/// `\n` → newline, `\r` → CR (collapsed), `\t` → tab, `\\` → literal `\`.
/// Unrecognised `\x` sequences are passed through unchanged.
pub fn expand_backslash_escapes(input: &str) -> String {
    if !input.contains('\\') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('r') => {
                    chars.next();
                    // Collapse `\r\n` (backslash-escape form: four chars
                    // `\`, `r`, `\`, `n`) into a single newline so CRLF
                    // doesn't produce blank double-spaced lines.
                    let mut la = chars.clone();
                    if la.next() == Some('\\') && la.next() == Some('n') {
                        chars.next();
                        chars.next();
                    }
                    out.push('\n');
                }
                Some('t') => {
                    chars.next();
                    out.push('\t');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Replace KiCad `{name}` escape tokens with their literal character.
///
/// KiCad uses these so the raw `/` (hierarchical path separator) and a few
/// other reserved characters don't have to appear in label/pin text streams.
pub fn expand_char_escapes(input: &str) -> String {
    if !input.contains('{') {
        return input.to_string();
    }
    let mut out = input.to_string();
    for (tok, ch) in ESCAPE_TABLE {
        if out.contains(tok) {
            out = out.replace(tok, ch);
        }
    }
    out
}

/// Inverse of `expand_char_escapes` — replace literal reserved characters with
/// their `{name}` KiCad escape tokens so the text round-trips through the
/// S-expression writer unambiguously.
pub fn escape_for_kicad(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '/' => out.push_str("{slash}"),
            '\\' => out.push_str("{backslash}"),
            _ => out.push(ch),
        }
    }
    out
}

const ESCAPE_TABLE: &[(&str, &str)] = &[
    ("{slash}", "/"),
    ("{backslash}", "\\"),
    ("{tilde}", "~"),
    ("{colon}", ":"),
    ("{dollar}", "$"),
    ("{space}", " "),
];

/// Draw a text note on the schematic.
pub fn draw_text_note(
    frame: &mut canvas::Frame,
    note: &TextNote,
    transform: &ScreenTransform,
    color: Color,
) {
    // Fixed 10 pt (1.8 mm) for all canvas text — matches Altium default.
    let font_size_mm = crate::SCHEMATIC_TEXT_MM;
    let _stored = note.font_size;
    let screen_font = transform.world_len(font_size_mm).abs() * crate::STROKE_FONT_SCALE;
    if screen_font < 1.0 {
        return;
    }

    let sp = transform.to_screen_point(note.position.x, note.position.y);

    let h_align = match note.justify_h {
        HAlign::Left => iced::alignment::Horizontal::Left,
        HAlign::Center => iced::alignment::Horizontal::Center,
        HAlign::Right => iced::alignment::Horizontal::Right,
    };

    let v_align = match note.justify_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    let rad = -(note.rotation.to_radians() as f32);

    draw_rich_text(
        frame,
        &note.text,
        sp,
        color,
        screen_font,
        h_align,
        v_align,
        rad,
    );
}

/// Draw a property text (reference, value, or other field).
///
/// `display_pos`: resolved display position for the field text, computed by
/// caller via [`field_display_pos`].
///
/// `mirror_x`: true when the parent symbol has `mirror x` (flips Y axis),
/// which causes KiCad to flip the horizontal justification of the field text
/// (SCH_FIELD::GetEffectiveJustify). Pass `sym.mirror_x`.
///
/// Rotation: KiCad field angles are CCW-positive in their Y-down screen
/// space. Iced `frame.rotate()` is CW-positive, so we negate the angle.
pub fn draw_text_prop(
    frame: &mut canvas::Frame,
    content: &str,
    prop: &TextProp,
    sym: &Symbol,
    display_pos: (f64, f64),
    transform: &ScreenTransform,
    color: Color,
) {
    if content.is_empty() {
        return;
    }

    let evaluated = evaluate_symbol_text(content, sym, None);
    if evaluated.is_empty() {
        return;
    }

    // All symbol ref/val text renders at 10 pt (1.8 mm).
    let font_size_mm = crate::SCHEMATIC_TEXT_MM;
    let _stored = prop.font_size;
    let screen_font = transform.world_len(font_size_mm).abs() * crate::STROKE_FONT_SCALE;
    if screen_font < 1.0 {
        return;
    }

    let sp = transform.to_screen_point(display_pos.0, display_pos.1);

    let (draw_rotation, effective_h, effective_v) = field_effective_style(prop, sym);

    let h_align = match effective_h {
        HAlign::Left => iced::alignment::Horizontal::Left,
        HAlign::Center => iced::alignment::Horizontal::Center,
        HAlign::Right => iced::alignment::Horizontal::Right,
    };

    let v_align = match effective_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    // Iced CW-positive, Y-down; KiCad field angles are CCW.
    let rad = -(draw_rotation.to_radians() as f32);

    draw_rich_text(frame, &evaluated, sp, color, screen_font, h_align, v_align, rad);
}
