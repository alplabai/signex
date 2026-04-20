# KiCad S-Expression — Rust Macro & AST Referansı

Recommended approach for generating KiCad S-expressions in Rust:
**AST/enum → `macro_rules!` DSL → `Display` serialize**.

---

## Temel AST Enum'u

```rust
/// KiCad atom tipleri — quoted/unquoted ayrımı burada yapılır
#[derive(Debug, Clone)]
pub enum Atom {
    /// Bare token: at, layer, F.Cu, 0.25 — written without quotes
    Raw(String),
    /// Quoted string: "Resistor_SMD:R_0603" — zorunlu tırnak
    Str(String),
    /// Tam sayı
    Int(i64),
    /// Decimal — 6 digits for PCB, 4 digits for schematic
    Float(f64),
}

#[derive(Debug, Clone)]
pub enum Node {
    Atom(Atom),
    List(Vec<Node>),
}
```

---

## `Display` Serialize

```rust
use std::fmt;

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Atom::Raw(s)   => write!(f, "{s}"),
            Atom::Str(s)   => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Atom::Int(n)   => write!(f, "{n}"),
            Atom::Float(v) => write!(f, "{v:.6}"),  // use .4 for schematic
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Atom(a) => write!(f, "{a}"),
            Node::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
        }
    }
}
```

### Pretty-print (girintili)

```rust
impl Node {
    pub fn pretty(&self, indent: usize) -> String {
        match self {
            Node::Atom(a) => a.to_string(),
            Node::List(items) => {
                // Kısa listeler tek satırda kalsın
                let one_line = self.to_string();
                if one_line.len() <= 80 || items.len() <= 2 {
                    return one_line;
                }
                let pad = "  ".repeat(indent + 1);
                let inner: Vec<String> = items.iter()
                    .map(|n| format!("{pad}{}", n.pretty(indent + 1)))
                    .collect();
                format!("(\n{}\n{})", inner.join("\n"), "  ".repeat(indent))
            }
        }
    }
}
```

---

## `macro_rules!` DSL

### Basit `sexpr!` macro'su

```rust
/// KiCad S-expression builder DSL macro.
/// 
/// # Sözdizimi
/// - `sexpr!( (token arg1 arg2 ...) )` → List node
/// - `sexpr!("string")` → Str atom (quoted)
/// - `sexpr!(identifier)` → Raw atom (unquoted)
/// - `sexpr!(expr)` → Raw atom, `to_string()` ile
macro_rules! sexpr {
    // İç içe liste
    (($($items:tt)*)) => {
        Node::List(sexpr_list![$($items)*])
    };
    // Quoted string literal
    ($value:literal) => {{
        let s = $value.to_string();
        // Sayısal literaller Raw, string literaller Str olsun
        if s.parse::<f64>().is_ok() {
            Node::Atom(Atom::Raw(s))
        } else {
            Node::Atom(Atom::Str(s))
        }
    }};
    // Çıplak tanımlayıcı: token, layer, at, pts, ...
    ($ident:ident) => {
        Node::Atom(Atom::Raw(stringify!($ident).to_string()))
    };
    // Genel ifade fallback
    ($expr:expr) => {
        Node::Atom(Atom::Raw(($expr).to_string()))
    };
}

/// Inner list builder for sexpr!
macro_rules! sexpr_list {
    () => { vec![] };
    ($head:tt $($tail:tt)*) => {{
        let mut v = vec![sexpr!($head)];
        v.extend(sexpr_list![$($tail)*]);
        v
    }};
}
```

### Kullanım Örnekleri

```rust
fn main() {
    let x: f64 = 10.0;
    let y: f64 = 20.0;
    let net_name = "GND";

    // at, layer gibi basit node'lar
    let at_node   = sexpr!((at {x} {y}));
    let layer_node = sexpr!((layer "F.Cu"));

    // Pad generation
    let pad = sexpr!((
        pad "1" smd circle
        (at 0 0)
        (size 1.6 1.6)
        (layers "F.Cu" "F.Paste" "F.Mask")
        (net 1 {net_name})
    ));

    println!("{}", pad);
    // → (pad "1" smd circle (at 0 0) (size 1.6 1.6)
    //      (layers "F.Cu" "F.Paste" "F.Mask") (net 1 "GND"))
}
```

---

## Yardımcı Fonksiyonlar (Helper API)

Macro yerine doğrudan çağrılabilir; daha tip güvenli.

```rust
// ─── Temel konumlandırma ───────────────────────────────────────────

/// (at X Y) — konum
pub fn at(x: f64, y: f64) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("at".into())),
        Node::Atom(Atom::Float(x)),
        Node::Atom(Atom::Float(y)),
    ])
}

/// (at X Y ANGLE) — konumlandırma + açı (derece)
pub fn at_angle(x: f64, y: f64, angle: f64) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("at".into())),
        Node::Atom(Atom::Float(x)),
        Node::Atom(Atom::Float(y)),
        Node::Atom(Atom::Float(angle)),
    ])
}

/// (xy X Y) — tek nokta
pub fn xy(x: f64, y: f64) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("xy".into())),
        Node::Atom(Atom::Float(x)),
        Node::Atom(Atom::Float(y)),
    ])
}

/// (pts (xy X1 Y1) (xy X2 Y2) ...) — nokta listesi
pub fn pts(points: &[(f64, f64)]) -> Node {
    let mut items = vec![Node::Atom(Atom::Raw("pts".into()))];
    items.extend(points.iter().map(|&(x, y)| xy(x, y)));
    Node::List(items)
}

// ─── Katman ve renk ───────────────────────────────────────────────

/// (layer "F.Cu") — single layer token
pub fn layer(name: &str) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("layer".into())),
        Node::Atom(Atom::Str(name.into())),
    ])
}

/// (layers "F.Cu" "F.Paste" ...) — pad layer list
pub fn layers(names: &[&str]) -> Node {
    let mut items = vec![Node::Atom(Atom::Raw("layers".into()))];
    items.extend(names.iter().map(|&n| Node::Atom(Atom::Str(n.into()))));
    Node::List(items)
}

// ─── Çizgi stili ──────────────────────────────────────────────────

pub enum StrokeType { Solid, Dash, Dot, DashDot, DashDotDot, Default }

/// (stroke (width W) (type solid|...))
pub fn stroke(width: f64, stroke_type: StrokeType) -> Node {
    let type_str = match stroke_type {
        StrokeType::Solid      => "solid",
        StrokeType::Dash       => "dash",
        StrokeType::Dot        => "dot",
        StrokeType::DashDot    => "dash_dot",
        StrokeType::DashDotDot => "dash_dot_dot",  // KiCad 7+
        StrokeType::Default    => "default",
    };
    Node::List(vec![
        Node::Atom(Atom::Raw("stroke".into())),
        Node::List(vec![
            Node::Atom(Atom::Raw("width".into())),
            Node::Atom(Atom::Float(width)),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("type".into())),
            Node::Atom(Atom::Raw(type_str.into())),
        ]),
    ])
}

// ─── PCB Grafik öğeleri ───────────────────────────────────────────

/// (gr_line (start X Y) (end X Y) (layer L) (stroke ...) (uuid U))
pub fn gr_line(x1: f64, y1: f64, x2: f64, y2: f64,
               lyr: &str, width: f64, uuid: &str) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("gr_line".into())),
        Node::List(vec![
            Node::Atom(Atom::Raw("start".into())),
            Node::Atom(Atom::Float(x1)), Node::Atom(Atom::Float(y1)),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("end".into())),
            Node::Atom(Atom::Float(x2)), Node::Atom(Atom::Float(y2)),
        ]),
        layer(lyr),
        stroke(width, StrokeType::Solid),
        Node::List(vec![
            Node::Atom(Atom::Raw("uuid".into())),
            Node::Atom(Atom::Raw(uuid.into())),
        ]),
    ])
}

// ─── Footprint öğeleri ────────────────────────────────────────────

/// (fp_line (start X Y) (end X Y) (layer L) (stroke ...) (uuid U))
pub fn fp_line(x1: f64, y1: f64, x2: f64, y2: f64,
               lyr: &str, width: f64, uuid: &str) -> Node {
    let mut inner = gr_line(x1, y1, x2, y2, lyr, width, uuid);
    if let Node::List(ref mut items) = inner {
        items[0] = Node::Atom(Atom::Raw("fp_line".into()));
    }
    inner
}

/// (fp_text reference|value|user "METİN" (at X Y) (layer L) (effects ...) (uuid U))
pub fn fp_text(kind: &str, text: &str, x: f64, y: f64,
               lyr: &str, uuid: &str) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("fp_text".into())),
        Node::Atom(Atom::Raw(kind.into())),
        Node::Atom(Atom::Str(text.into())),
        at(x, y),
        layer(lyr),
        // Varsayılan effects
        Node::List(vec![
            Node::Atom(Atom::Raw("effects".into())),
            Node::List(vec![
                Node::Atom(Atom::Raw("font".into())),
                Node::List(vec![
                    Node::Atom(Atom::Raw("size".into())),
                    Node::Atom(Atom::Float(1.27)),
                    Node::Atom(Atom::Float(1.27)),
                ]),
                Node::List(vec![
                    Node::Atom(Atom::Raw("thickness".into())),
                    Node::Atom(Atom::Float(0.15)),
                ]),
            ]),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("uuid".into())),
            Node::Atom(Atom::Raw(uuid.into())),
        ]),
    ])
}

/// (pad "N" smd|thru_hole circle|rect|oval (at X Y) (size W H)
///      (layers ...) [(net N "ADI")] (uuid U))
pub fn smd_pad(number: &str, shape: &str,
               x: f64, y: f64, w: f64, h: f64,
               pad_layers: &[&str],
               net: Option<(i32, &str)>,
               uuid: &str) -> Node {
    let mut items = vec![
        Node::Atom(Atom::Raw("pad".into())),
        Node::Atom(Atom::Str(number.into())),
        Node::Atom(Atom::Raw("smd".into())),
        Node::Atom(Atom::Raw(shape.into())),
        at(x, y),
        Node::List(vec![
            Node::Atom(Atom::Raw("size".into())),
            Node::Atom(Atom::Float(w)),
            Node::Atom(Atom::Float(h)),
        ]),
        layers(pad_layers),
    ];
    if let Some((net_num, net_name)) = net {
        items.push(Node::List(vec![
            Node::Atom(Atom::Raw("net".into())),
            Node::Atom(Atom::Int(net_num as i64)),
            Node::Atom(Atom::Str(net_name.into())),
        ]));
    }
    items.push(Node::List(vec![
        Node::Atom(Atom::Raw("uuid".into())),
        Node::Atom(Atom::Raw(uuid.into())),
    ]));
    Node::List(items)
}

// ─── Şematik öğeleri ──────────────────────────────────────────────

/// (wire (pts (xy X1 Y1) (xy X2 Y2)) (stroke ...) (uuid U))
pub fn wire(x1: f64, y1: f64, x2: f64, y2: f64, uuid: &str) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("wire".into())),
        pts(&[(x1, y1), (x2, y2)]),
        stroke(0.0, StrokeType::Default),
        Node::List(vec![
            Node::Atom(Atom::Raw("uuid".into())),
            Node::Atom(Atom::Raw(uuid.into())),
        ]),
    ])
}

/// (junction (at X Y) (diameter 0) (color 0 0 0 0) (uuid U))
pub fn junction(x: f64, y: f64, uuid: &str) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("junction".into())),
        at(x, y),
        Node::List(vec![
            Node::Atom(Atom::Raw("diameter".into())),
            Node::Atom(Atom::Float(0.0)),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("color".into())),
            Node::Atom(Atom::Int(0)),
            Node::Atom(Atom::Int(0)),
            Node::Atom(Atom::Int(0)),
            Node::Atom(Atom::Int(0)),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("uuid".into())),
            Node::Atom(Atom::Raw(uuid.into())),
        ]),
    ])
}
```

---

## UUID Üretimi (Rust)

```rust
/// KiCad-compatible UUID v4 generator (without uuid crate)
/// Cargo.toml: uuid = { version = "1", features = ["v4"] }

#[cfg(feature = "uuid")]
pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// uuid crate olmadan (gerekirse)
pub fn pseudo_uuid(seed: u64) -> String {
    // For testing only — use the uuid crate in production
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        seed & 0xffffffff,
        (seed >> 32) & 0xffff,
        (seed >> 48) & 0x0fff,
        0x8000 | ((seed >> 52) & 0x3fff),
        seed.wrapping_mul(6364136223846793005),
    )
}
```

---

## Tam Footprint Üretim Örneği

```rust
fn build_r0603() -> Node {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap().as_secs();
    let tedit = format!("{:X}", ts);

    Node::List(vec![
        Node::Atom(Atom::Raw("footprint".into())),
        Node::Atom(Atom::Str("Resistor_SMD:R_0603_1608Metric".into())),
        layer("F.Cu"),
        Node::List(vec![
            Node::Atom(Atom::Raw("tedit".into())),
            Node::Atom(Atom::Raw(tedit)),
        ]),
        // fp_text — reference
        fp_text("reference", "R1", 0.0, -1.65, "F.SilkS",
                &pseudo_uuid(1)),
        // fp_text — value
        fp_text("value", "10k", 0.0, 1.65, "F.Fab",
                &pseudo_uuid(2)),
        // Courtyard
        fp_line(-1.88, -0.98, 1.88, -0.98, "F.CrtYd", 0.05, &pseudo_uuid(3)),
        fp_line( 1.88, -0.98, 1.88,  0.98, "F.CrtYd", 0.05, &pseudo_uuid(4)),
        fp_line( 1.88,  0.98,-1.88,  0.98, "F.CrtYd", 0.05, &pseudo_uuid(5)),
        fp_line(-1.88,  0.98,-1.88, -0.98, "F.CrtYd", 0.05, &pseudo_uuid(6)),
        // Pad 1 — GND
        smd_pad("1", "rect", -1.525, 0.0, 1.6, 1.8,
                &["F.Cu", "F.Paste", "F.Mask"],
                Some((1, "GND")), &pseudo_uuid(10)),
        // Pad 2 — VCC
        smd_pad("2", "rect",  1.525, 0.0, 1.6, 1.8,
                &["F.Cu", "F.Paste", "F.Mask"],
                Some((2, "VCC")), &pseudo_uuid(11)),
    ])
}

fn main() {
    let fp = build_r0603();
    println!("{}", fp.pretty(0));
}
```

---

## Kritik Notlar (Rust'a Özgü)

| Konu | Kural |
|------|-------|
| `Float` hassasiyet | PCB → `{:.6}`, Şematik → `{:.4}` |
| Quoted vs Raw | `"F.Cu"` quoted `Str`, `smd` / `at` / sayılar → `Raw` |
| `uuid` format | Tam `XXXXXXXX-XXXX-4XXX-XXXX-XXXXXXXXXXXX` |
| `tedit` | `format!("{:X}", unix_timestamp_secs)` |
| `fp_text` zorunlu | `reference` ve `value` her footprint'te olmalı |
| KiCad 7 `stroke` | `width` token is now inside `stroke` — old `(width W)` is invalid |
| Macro `$expr` fallback | use `sexpr!({x})` for variable interpolation |
