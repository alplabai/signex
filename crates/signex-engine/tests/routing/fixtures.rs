//! The boards the routing soak and bench run on: a synthetic board, the
//! realistic corpus boards (`signex-types/tests/corpus/pcb/real/`), and the
//! boards of the routing corpus (`testdata/routing-corpus/*.snxpcb`, #2481),
//! all of them or none. A board's net classes come from the `.snxprj`
//! beside it. The routing corpus is stored in Git LFS: a board whose
//! objects were not fetched is skipped with a message.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::format::SnxPcb;
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use signex_types::project::parse_project_bytes;
use uuid::Uuid;

/// The first line of a Git LFS pointer, which stands in for a file whose
/// objects were not fetched.
const LFS_POINTER: &str = "version https://git-lfs.github.com/spec/v1";

pub struct Fixture {
    pub name: String,
    pub board: PcbBoard,
    pub classes: Vec<NetClass>,
}

/// Whether a run takes the routing-corpus boards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corpus {
    All,
    /// The fixed boards only: per-PR runs, where routing a corpus board in
    /// the debug test profile takes too long.
    None,
}

/// The boards, in a fixed order: the synthetic one, the realistic corpus,
/// then the routing corpus when `corpus` takes it.
pub fn boards(corpus: Corpus) -> Vec<Fixture> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = vec![synthetic()];
    out.extend(
        board_paths(&root.join("../signex-types/tests/corpus/pcb/real"))
            .iter()
            .filter_map(|path| load(path)),
    );
    if corpus == Corpus::All {
        let paths = corpus_paths();
        let loaded: Vec<Fixture> = paths.iter().filter_map(|path| load(path)).collect();
        assert!(
            !corpus_required() || (!paths.is_empty() && loaded.len() == paths.len()),
            "{REQUIRE_CORPUS}=1 but {} of the {} routing corpus boards loaded \
             (the others are Git LFS pointers; run `git lfs pull`)",
            loaded.len(),
            paths.len()
        );
        out.extend(loaded);
    }
    out
}

/// Set to `1` where the corpus's Git LFS objects were fetched (CI does when
/// the corpus or its readers change, and the scheduled soak always): an
/// unfetched board or an empty corpus then fails instead of being skipped.
pub const REQUIRE_CORPUS: &str = "SIGNEX_REQUIRE_ROUTING_CORPUS";

pub fn corpus_required() -> bool {
    std::env::var(REQUIRE_CORPUS).is_ok_and(|v| v == "1")
}

/// The routing corpus's `.snxpcb` files, by file name.
pub fn corpus_paths() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    board_paths(&root.join("../../testdata/routing-corpus"))
}

/// The `.snxpcb` files in `dir`, by file name; none when there is no `dir`.
fn board_paths(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "snxpcb"))
        .collect();
    paths.sort();
    paths
}

/// The board at `path`; `None`, with a message, for a Git LFS pointer.
pub fn load(path: &Path) -> Option<Fixture> {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{name}: {e}"));
    if text.starts_with(LFS_POINTER) {
        eprintln!("{name}: skipped, a Git LFS pointer; run `git lfs pull` to fetch the board");
        return None;
    }
    let board = SnxPcb::parse(&text)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
        .board;
    let project = path.with_extension("snxprj");
    let classes = match std::fs::read(&project) {
        Ok(bytes) => {
            parse_project_bytes(
                &project,
                &bytes,
                signex_types::project::files_on_disk(&project),
            )
            .unwrap_or_else(|e| panic!("{name}.snxprj: {e:?}"))
            .net_classes
        }
        Err(_) => Vec::new(),
    };
    Some(Fixture {
        name,
        board,
        classes,
    })
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
