//! The Advanced Schematic exit-criteria projects in `tests/corpus/advanced/`
//! (`docs/ROADMAP.md`, "Advanced Schematic (P1)"): a 4-channel design with
//! two variants, a nested harness (direct and carried through a sheet), and
//! net classes given by directives. Each is a folder with its `.snxprj`,
//! its sheets and, for the classes, a board.
//!
//! The files are written by the deterministic generator in this module, as
//! the real boards are. `SIGNEX_BLESS_CORPUS=1` (or `UPDATE_GOLDEN=1`)
//! rewrites them; without it the test asserts the committed bytes are
//! exactly what the generator writes. The generic corpus harness
//! (`round_trip_corpus`) walks the sheets and the board too. The goldens
//! read from these files live with the crates that make them: the netlists
//! in `signex-net/tests/`, the BOM, PDF and Update PCB checks in signex-app
//! (`handlers/menu/export/tests/exit_fixtures.rs`). Coverage:
//! `tests/corpus/README.md`, "Advanced Schematic projects".

mod builder;
mod four_channel;
mod nested_harness;
mod net_classes;

use std::fs;
use std::path::{Path, PathBuf};

use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::project::{encode_project, parse_project_bytes};

const BLESS_ENVS: [&str; 2] = ["SIGNEX_BLESS_CORPUS", "UPDATE_GOLDEN"];

/// Every generated file, by its path under `tests/corpus/advanced/`.
fn generate() -> Vec<(String, Vec<u8>)> {
    let mut files = four_channel::files();
    files.extend(nested_harness::files());
    files.extend(nested_harness::carry_files());
    files.extend(net_classes::files());
    files
}

fn advanced_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/advanced")
}

fn blessing() -> bool {
    BLESS_ENVS
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|v| v == "1"))
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("utf-8")
}

#[test]
fn advanced_fixtures_match_their_generator() {
    let bless = blessing();
    let root = advanced_dir();
    let mut stale = Vec::new();
    for (file, bytes) in generate() {
        let path = root.join(&file);
        if bless {
            fs::create_dir_all(path.parent().expect("a folder")).unwrap();
            fs::write(&path, &bytes).unwrap();
        } else if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
            stale.push(file);
        }
    }
    assert!(
        stale.is_empty(),
        "generated corpus files differ from the generator: {stale:?}\n(an intentional change: rerun with SIGNEX_BLESS_CORPUS=1 and review the diff)"
    );
}

/// Each generated sheet, board and project reads back and saves to the
/// same bytes, whatever is on disk.
#[test]
fn every_generated_file_saves_back_to_its_own_bytes() {
    for (file, bytes) in generate() {
        let resaved = match Path::new(&file).extension().and_then(|e| e.to_str()) {
            Some("snxsch") => SnxSchematic::parse(text(&bytes))
                .unwrap_or_else(|e| panic!("{file}: {e}"))
                .write_string()
                .unwrap_or_else(|e| panic!("{file}: {e}"))
                .into_bytes(),
            Some("snxpcb") => SnxPcb::parse(text(&bytes))
                .unwrap_or_else(|e| panic!("{file}: {e}"))
                .write_string()
                .unwrap_or_else(|e| panic!("{file}: {e}"))
                .into_bytes(),
            Some("snxprj") => {
                let mut data =
                    parse_project_bytes(Path::new("/corpus").join(&file).as_path(), &bytes)
                        .unwrap_or_else(|e| panic!("{file}: {e}"));
                // The reader puts the file's real folder in `dir`; the
                // corpus stores the empty placeholder.
                data.dir.clear();
                encode_project(&data).unwrap_or_else(|e| panic!("{file}: {e}"))
            }
            other => panic!("{file}: unexpected extension {other:?}"),
        };
        let (old, new) = (text(&bytes), String::from_utf8_lossy(&resaved));
        let first = old
            .lines()
            .zip(new.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(n, (a, b))| format!("line {}: generated `{a}`, saved `{b}`", n + 1))
            .unwrap_or_else(|| "one file is longer".into());
        assert!(
            resaved == bytes,
            "{file}: a no-edit save changes the file; first difference: {first}"
        );
    }
}

/// What each project is made of, so a generator change that drops a part
/// of the exit criterion is caught here and not only as a golden diff.
#[test]
fn the_fixtures_hold_what_their_criteria_need() {
    let sheet = |file: &str| {
        let (_, bytes) = generate()
            .into_iter()
            .find(|(name, _)| name == file)
            .unwrap_or_else(|| panic!("{file} is generated"));
        SnxSchematic::parse(text(&bytes)).expect("reads").sheet
    };

    let top = sheet("four-channel-variants/top.snxsch");
    assert_eq!(top.child_sheets.len(), 1);
    assert_eq!(top.child_sheets[0].name, "Repeat(CH, 1, 4)");
    let channel = sheet("four-channel-variants/channel.snxsch");
    let mut parts: Vec<&str> = channel
        .symbols
        .iter()
        .map(|s| s.reference.as_str())
        .collect();
    parts.sort_unstable();
    assert_eq!(parts, ["C1", "C2", "R1"]);

    let mcu = sheet("nested-harness/mcu.snxsch");
    assert_eq!(mcu.harness.connectors.len(), 3, "SPI_BUS, UART and DEBUG");
    let carry = sheet("nested-harness-carry/mcu.snxsch");
    assert_eq!(carry.child_sheets[1].filename, "bridge.snxsch");
    let bridge = sheet("nested-harness-carry/bridge.snxsch");
    assert!(
        bridge.harness.connectors.is_empty(),
        "the bridge only carries"
    );

    let classes = sheet("net-class-directives/main.snxsch");
    assert_eq!(classes.directives.len(), 3);
    let mut labels: Vec<&str> = classes.labels.iter().map(|l| l.text.as_str()).collect();
    labels.sort_unstable();
    let mut nets = net_classes::NETS.to_vec();
    nets.sort_unstable();
    assert_eq!(labels, nets);
}
