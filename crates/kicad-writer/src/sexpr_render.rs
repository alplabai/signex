use std::fmt::Write;

pub(crate) use kicad_parser::sexpr::SExpr;
pub(crate) use kicad_parser::sexpr_builder::{atom, list, raw};

pub(crate) fn write_rendered_sexpr(out: &mut String, indent: usize, expr: SExpr) {
    let indent = " ".repeat(indent);
    for line in expr.pretty(0).lines() {
        let _ = writeln!(out, "{indent}{line}");
    }
}

pub(crate) fn node(keyword: &str, children: impl IntoIterator<Item = SExpr>) -> SExpr {
    let mut items = vec![raw(keyword)];
    items.extend(children);
    list(items)
}

pub(crate) fn at_node(x: f64, y: f64, rotation: Option<f64>) -> SExpr {
    let mut items = vec![atom(x), atom(y)];
    if let Some(rotation) = rotation {
        items.push(atom(rotation));
    }
    node("at", items)
}

pub(crate) fn yes_no_node(keyword: &str, value: bool) -> SExpr {
    node(keyword, vec![raw(if value { "yes" } else { "no" })])
}

pub(crate) fn hide_yes_node() -> SExpr {
    yes_no_node("hide", true)
}

pub(crate) fn font_node(
    size: f64,
    thickness: Option<f64>,
    bold: bool,
    italic: bool,
) -> SExpr {
    let mut items = vec![node("size", vec![atom(size), atom(size)])];
    if let Some(thickness) = thickness {
        items.push(node("thickness", vec![atom(thickness)]));
    }
    if bold {
        items.push(raw("bold"));
    }
    if italic {
        items.push(raw("italic"));
    }
    node("font", items)
}

pub(crate) fn effects_node(
    font_size: f64,
    thickness: Option<f64>,
    bold: bool,
    italic: bool,
    extras: impl IntoIterator<Item = SExpr>,
) -> SExpr {
    let mut items = vec![font_node(font_size, thickness, bold, italic)];
    items.extend(extras);
    node("effects", items)
}