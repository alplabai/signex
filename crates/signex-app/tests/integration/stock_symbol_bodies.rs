//! #2370 — the stock resistor body is shorter with its pin ends unchanged,
//! and the ferrite and fuse bodies are the same rectangle as the resistor's,
//! each keeping its own inner mark. Checked on the committed files, which
//! are the stock library's source of truth (#1892).

use std::path::PathBuf;

use signex_library::primitive::symbol::{Symbol, SymbolFile, SymbolGraphicKind};

/// The resistor body the stock library shipped before #2370: 1.524 x 5.08 mm.
const OLD_RESISTOR_BODY: [f64; 2] = [1.524, 5.08];

fn stock_symbol(name: &str) -> Symbol {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/stock-library/symbols")
        .join(format!("{name}.snxsym"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .replace("\r\n", "\n");
    let mut file = SymbolFile::from_toml_str(&text).unwrap_or_else(|e| panic!("parse {name}: {e}"));
    file.symbols.remove(0)
}

/// Width and height of the symbol's one rectangle, and its top edge.
fn body(sym: &Symbol) -> ([f64; 2], f64) {
    let rects: Vec<_> = sym
        .graphics
        .iter()
        .filter_map(|g| match g.kind {
            SymbolGraphicKind::Rectangle { from, to } => Some((from, to)),
            _ => None,
        })
        .collect();
    assert_eq!(rects.len(), 1, "{}: one body rectangle", sym.name);
    let (from, to) = rects[0];
    (
        [(to[0] - from[0]).abs(), (to[1] - from[1]).abs()],
        from[1].max(to[1]),
    )
}

fn lines(sym: &Symbol) -> Vec<([f64; 2], [f64; 2])> {
    sym.graphics
        .iter()
        .filter_map(|g| match g.kind {
            SymbolGraphicKind::Line { from, to } => Some((from, to)),
            _ => None,
        })
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn the_resistor_body_is_shorter_and_its_pin_ends_did_not_move() {
    let r = stock_symbol("R");
    let ([w, h], top) = body(&r);
    assert!(close(w, OLD_RESISTOR_BODY[0]), "width {w}");
    assert!(h < OLD_RESISTOR_BODY[1] - 1e-6, "height {h} is not shorter");
    let ends: Vec<_> = r
        .pins
        .iter()
        .map(|p| (p.number.as_str(), p.name.as_str(), p.position))
        .collect();
    assert_eq!(ends, [("1", "1", [0.0, 3.81]), ("2", "2", [0.0, -3.81])]);
    for p in &r.pins {
        assert!(
            close(p.position[1].abs() - p.length, top),
            "pin {} lead ends at the body edge",
            p.number
        );
    }
}

#[test]
fn ferrite_and_fuse_bodies_match_the_resistor_and_keep_their_marks() {
    let (resistor, _) = body(&stock_symbol("R"));
    for name in ["Ferrite", "Fuse"] {
        let sym = stock_symbol(name);
        let ([w, h], top) = body(&sym);
        assert!(
            close(w, resistor[0]) && close(h, resistor[1]),
            "{name} body {w} x {h} vs resistor {resistor:?}"
        );
        let ends: Vec<_> = sym.pins.iter().map(|p| p.position).collect();
        assert_eq!(ends, [[0.0, 3.81], [0.0, -3.81]], "{name} pin ends");
        for p in &sym.pins {
            assert!(close(p.position[1].abs() - p.length, top), "{name} lead");
        }
        let marks = lines(&sym);
        assert_eq!(marks.len(), 1, "{name}: one inner mark");
        let (from, to) = marks[0];
        let half = [resistor[0] / 2.0, resistor[1] / 2.0];
        let expected = match name {
            "Ferrite" => ([-half[0], -half[1]], [half[0], half[1]]),
            _ => ([0.0, half[1]], [0.0, -half[1]]),
        };
        assert!(
            close(from[0], expected.0[0])
                && close(from[1], expected.0[1])
                && close(to[0], expected.1[0])
                && close(to[1], expected.1[1]),
            "{name} mark {from:?} -> {to:?}"
        );
    }
}
