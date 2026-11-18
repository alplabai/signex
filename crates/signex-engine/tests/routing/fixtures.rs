//! The boards the routing soak and bench run on: a synthetic board, the
//! realistic corpus boards (`signex-types/tests/corpus/pcb/real/`), and
//! every board of the routing corpus (`testdata/routing-corpus/*.snxpcb`)
//! once it is in. A board's net classes come from the `.snxprj` beside it.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::format::SnxPcb;
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use signex_types::project::parse_project_bytes;
use uuid::Uuid;

pub struct Fixture {
    pub name: String,
    pub board: PcbBoard,
    pub classes: Vec<NetClass>,
}

/// Every board, in a fixed order: the synthetic one, the realistic corpus,
/// then the routing corpus.
pub fn all() -> Vec<Fixture> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = vec![synthetic()];
    out.extend(boards_in(
        &root.join("../signex-types/tests/corpus/pcb/real"),
    ));
    out.extend(boards_in(&root.join("../../testdata/routing-corpus")));
    out
}

/// The `.snxpcb` boards in `dir`, by file name; none when there is no `dir`.
fn boards_in(dir: &Path) -> Vec<Fixture> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "snxpcb"))
        .collect();
    paths.sort();
    paths.iter().map(|path| load(path)).collect()
}

fn load(path: &Path) -> Fixture {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{name}: {e}"));
    let board = SnxPcb::parse(&text)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
        .board;
    let project = path.with_extension("snxprj");
    let classes = match std::fs::read(&project) {
        Ok(bytes) => {
            parse_project_bytes(&project, &bytes)
                .unwrap_or_else(|e| panic!("{name}.snxprj: {e:?}"))
                .net_classes
        }
        Err(_) => Vec::new(),
    };
    Fixture {
        name,
        board,
        classes,
    }
}

/// Eight nets of three round 1 mm top pads each, in rows that bend so
/// routes cross, and four free 0.25 mm top runs of other nets between the
/// rows for a shove to push. A 0.5 mm grid and a 0.2 mm clearance.
pub fn synthetic() -> Fixture {
    let nets: Vec<_> = (1..=12u32)
        .map(|n| json!({"number": n, "name": format!("N{n}")}))
        .collect();
    let mut footprints = Vec::new();
    for net in 1..=8u32 {
        for column in 0..3u32 {
            let n = u128::from(net * 3 + column);
            let x = 2.0 + 14.0 * f64::from(column);
            let y = 3.0 * f64::from(net) + 0.5 * f64::from((column * net) % 3);
            footprints.push(pad(n, x, y, net));
        }
    }
    let segments: Vec<_> = (0..4u32)
        .map(|i| {
            let y = 4.5 + 6.0 * f64::from(i);
            track(0x900 + u128::from(i), (6.0, y), (26.0, y), 9 + i)
        })
        .collect();
    let board = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": nets,
        "footprints": footprints,
        "segments": segments
    }))
    .expect("the synthetic board");
    Fixture {
        name: "synthetic".into(),
        board,
        classes: Vec::new(),
    }
}

/// A footprint holding one round 1 mm top pad at (`x`, `y`) on `net`.
fn pad(n: u128, x: f64, y: f64, net: u32) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"],
            "net": {"number": net, "name": format!("N{net}")}
        }]
    })
}

/// A 0.25 mm top track.
fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    })
}
