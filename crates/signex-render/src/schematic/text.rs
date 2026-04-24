//! Text rendering -- TextNote and TextProp (reference / value fields).

use iced::Color;
use iced::widget::canvas;
use std::collections::HashMap;
use uuid::Uuid;

use signex_types::markup::{
    ExpressionEvalContext, RichSegment, evaluate_expressions, kicad_auto_net_name_from_pins,
    parse_markup,
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
    kind: RichRunKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RichRunKind {
    Normal,
    Overbar,
    Subscript,
    Superscript,
}

// Keep horizontal advance in sync with label metric tuning (`label_text_aabb`).
const GLYPH_ADVANCE_FACTOR: f32 = 0.55;

fn run_pair_kerning(prev: RichRunKind, next: RichRunKind, size: f32) -> f32 {
    match (prev, next) {
        // KiCad keeps suffix indices visually tight to the preceding glyph.
        (
            RichRunKind::Normal | RichRunKind::Overbar,
            RichRunKind::Subscript | RichRunKind::Superscript,
        ) => -size * 0.16,
        _ => 0.0,
    }
}

fn rich_runs(input: &str) -> Vec<RichRun> {
    let expanded = expand_backslash_escapes(&expand_char_escapes(input));
    let segments = parse_markup(&expanded);
    if segments.is_empty() {
        return vec![RichRun {
            text: expanded,
            scale: 1.0,
            baseline_offset: 0.0,
            kind: RichRunKind::Normal,
        }];
    }

    segments
        .into_iter()
        .map(|segment| match segment {
            RichSegment::Normal(text) => RichRun {
                text,
                scale: 1.0,
                baseline_offset: 0.0,
                kind: RichRunKind::Normal,
            },
            RichSegment::Overbar(text) => RichRun {
                text,
                scale: 1.0,
                baseline_offset: 0.0,
                kind: RichRunKind::Overbar,
            },
            RichSegment::Subscript(text) => RichRun {
                text,
                scale: 0.72,
                baseline_offset: 0.0,
                kind: RichRunKind::Subscript,
            },
            RichSegment::Superscript(text) => RichRun {
                text,
                scale: 0.72,
                baseline_offset: -0.34,
                kind: RichRunKind::Superscript,
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
    evaluate_symbol_text_with_context(content, sym, current_pin, None, None, None)
}

pub fn evaluate_symbol_text_with_context(
    content: &str,
    sym: &Symbol,
    current_pin: Option<&str>,
    cell: Option<&str>,
    global_refdes: Option<&HashMap<String, String>>,
    pin_net_names: Option<&HashMap<String, String>>,
) -> String {
    let at_vars = symbol_eval_variables(sym);
    let mut refdes_vars = HashMap::new();
    if !sym.uuid.is_nil() && !sym.reference.is_empty() {
        refdes_vars.insert(sym.uuid.to_string(), sym.reference.clone());
    }

    let ctx = ExpressionEvalContext {
        current_refdes: (!sym.reference.is_empty()).then_some(sym.reference.as_str()),
        current_value: (!sym.value.is_empty()).then_some(sym.value.as_str()),
        current_pin,
        cell,
        at_variables: Some(&at_vars),
        refdes_variables: global_refdes.or(Some(&refdes_vars)),
        net_name_by_pin: pin_net_names,
        ..ExpressionEvalContext::default()
    };
    evaluate_expressions(content, &ctx)
}

pub fn build_global_refdes_lookup(
    snapshot: &super::SchematicRenderSnapshot,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for sym in &snapshot.symbols {
        if sym.reference.is_empty() {
            continue;
        }
        out.entry(sym.uuid.to_string())
            .or_insert_with(|| sym.reference.clone());
        out.entry(sym.reference.clone())
            .or_insert_with(|| sym.reference.clone());

        for instance in &sym.instances {
            if instance.path.is_empty() {
                continue;
            }
            out.entry(instance.path.clone())
                .or_insert_with(|| sym.reference.clone());
            let trimmed = instance.path.trim_matches('/');
            if !trimmed.is_empty() {
                out.entry(trimmed.to_string())
                    .or_insert_with(|| sym.reference.clone());
            }
        }
    }
    out
}

pub fn build_symbol_pin_net_lookup(
    snapshot: &super::SchematicRenderSnapshot,
) -> HashMap<Uuid, HashMap<String, String>> {
    type Node = (i64, i64);

    fn q(p: signex_types::schematic::Point) -> Node {
        ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
    }

    fn find(parent: &mut HashMap<Node, Node>, x: Node) -> Node {
        let p = *parent.entry(x).or_insert(x);
        if p == x {
            x
        } else {
            let r = find(parent, p);
            parent.insert(x, r);
            r
        }
    }

    fn union(parent: &mut HashMap<Node, Node>, a: Node, b: Node) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }

    fn point_on_segment(
        p: signex_types::schematic::Point,
        a: signex_types::schematic::Point,
        b: signex_types::schematic::Point,
        tol: f64,
    ) -> bool {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq < tol * tol {
            return (p.x - a.x).abs() < tol && (p.y - a.y).abs() < tol;
        }
        let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq;
        if !(0.0..=1.0).contains(&t) {
            return false;
        }
        let proj_x = a.x + t * dx;
        let proj_y = a.y + t * dy;
        (p.x - proj_x).abs() < tol && (p.y - proj_y).abs() < tol
    }

    fn transform_pin_position(
        sym: &Symbol,
        local_pos: &signex_types::schematic::Point,
    ) -> signex_types::schematic::Point {
        let x = local_pos.x;
        let y = -local_pos.y;

        let rad = -sym.rotation.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let rx = x * cos - y * sin;
        let ry = x * sin + y * cos;

        let rx = if sym.mirror_y { -rx } else { rx };
        let ry = if sym.mirror_x { -ry } else { ry };

        signex_types::schematic::Point::new(rx + sym.position.x, ry + sym.position.y)
    }

    fn label_priority(kind: signex_types::schematic::LabelType) -> u8 {
        match kind {
            signex_types::schematic::LabelType::Global => 4,
            signex_types::schematic::LabelType::Power => 3,
            signex_types::schematic::LabelType::Hierarchical => 2,
            signex_types::schematic::LabelType::Net => 1,
        }
    }

    let mut parent: HashMap<Node, Node> = HashMap::new();
    let tolerance = 0.01;

    for wire in &snapshot.wires {
        union(&mut parent, q(wire.start), q(wire.end));
    }

    for junction in &snapshot.junctions {
        let j = q(junction.position);
        parent.entry(j).or_insert(j);
        for wire in &snapshot.wires {
            if wire.start == junction.position
                || wire.end == junction.position
                || point_on_segment(junction.position, wire.start, wire.end, tolerance)
            {
                union(&mut parent, j, q(wire.start));
                union(&mut parent, j, q(wire.end));
            }
        }
    }

    let mut label_root_by_name: HashMap<String, Node> = HashMap::new();
    let mut root_name: HashMap<Node, (u8, String)> = HashMap::new();

    for label in &snapshot.labels {
        let mut n = q(label.position);
        parent.entry(n).or_insert(n);
        for wire in &snapshot.wires {
            if point_on_segment(label.position, wire.start, wire.end, tolerance) {
                union(&mut parent, q(wire.start), q(wire.end));
                union(&mut parent, n, q(wire.start));
                n = q(wire.start);
                break;
            }
        }

        let mut root = find(&mut parent, n);
        if matches!(
            label.label_type,
            signex_types::schematic::LabelType::Global
                | signex_types::schematic::LabelType::Hierarchical
        ) && !label.text.is_empty()
        {
            if let Some(existing) = label_root_by_name.get(&label.text).copied() {
                union(&mut parent, root, existing);
                root = find(&mut parent, root);
            }
            label_root_by_name.insert(label.text.clone(), root);
        }

        if !label.text.is_empty() {
            let priority = label_priority(label.label_type);
            match root_name.get(&root) {
                Some((p, _)) if *p >= priority => {}
                _ => {
                    root_name.insert(root, (priority, label.text.clone()));
                }
            }
        }
    }

    let mut root_pins: HashMap<Node, Vec<(String, String)>> = HashMap::new();
    let mut pin_entries: Vec<(Uuid, String, Node)> = Vec::new();

    for sym in &snapshot.symbols {
        let Some(lib) = snapshot.lib_symbols.get(&sym.lib_id) else {
            continue;
        };
        for lp in &lib.pins {
            if !(lp.unit == 0 || lp.unit == sym.unit) {
                continue;
            }
            let world = transform_pin_position(sym, &lp.pin.position);
            let root = find(&mut parent, q(world));
            root_pins
                .entry(root)
                .or_default()
                .push((sym.reference.clone(), lp.pin.number.clone()));
            pin_entries.push((sym.uuid, lp.pin.number.clone(), root));
        }
    }

    let mut resolved_root_name: HashMap<Node, String> = HashMap::new();
    for root in root_pins.keys().copied() {
        let named = root_name
            .get(&root)
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        if !named.is_empty() {
            resolved_root_name.insert(root, named);
            continue;
        }
        let auto = root_pins
            .get(&root)
            .and_then(|pins| kicad_auto_net_name_from_pins(pins))
            .unwrap_or_default();
        resolved_root_name.insert(root, auto);
    }

    let mut out: HashMap<Uuid, HashMap<String, String>> = HashMap::new();
    for (sym_uuid, pin_number, root) in pin_entries {
        let net_name = resolved_root_name.get(&root).cloned().unwrap_or_default();
        if !net_name.is_empty() {
            out.entry(sym_uuid)
                .or_default()
                .insert(pin_number, net_name);
        }
    }

    out
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
        .map(|run| run.text.chars().count() as f32 * size * run.scale * GLYPH_ADVANCE_FACTOR)
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

    let mut prev_kind: Option<RichRunKind> = None;
    for run in runs {
        if let Some(prev) = prev_kind {
            cursor_x += run_pair_kerning(prev, run.kind, size);
        }

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

        cursor_x += run.text.chars().count() as f32 * run_size * GLYPH_ADVANCE_FACTOR;
        prev_kind = Some(run.kind);
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
    cell: Option<&str>,
    global_refdes: Option<&HashMap<String, String>>,
    pin_net_names: Option<&HashMap<String, String>>,
) {
    if content.is_empty() {
        return;
    }

    let evaluated =
        evaluate_symbol_text_with_context(content, sym, None, cell, global_refdes, pin_net_names);
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

    draw_rich_text(
        frame,
        &evaluated,
        sp,
        color,
        screen_font,
        h_align,
        v_align,
        rad,
    );
}

#[cfg(test)]
mod tests {
    use super::{RichRunKind, run_pair_kerning};

    #[test]
    fn kerning_tightens_normal_to_subscript_gap() {
        let k = run_pair_kerning(RichRunKind::Normal, RichRunKind::Subscript, 10.0);
        assert!(k < 0.0);
    }

    #[test]
    fn kerning_does_not_affect_normal_to_normal() {
        let k = run_pair_kerning(RichRunKind::Normal, RichRunKind::Normal, 10.0);
        assert_eq!(k, 0.0);
    }

    #[test]
    fn subscript_keeps_same_baseline_as_normal() {
        let runs = super::rich_runs("DIVIDED-S_{3}");
        let s_run = runs
            .iter()
            .find(|run| run.kind == RichRunKind::Normal && run.text.ends_with('S'))
            .expect("normal run with S should exist");
        let sub_run = runs
            .iter()
            .find(|run| run.kind == RichRunKind::Subscript && run.text == "3")
            .expect("subscript run should exist");

        assert_eq!(s_run.baseline_offset, sub_run.baseline_offset);
    }
}
