//! Round-trip corpus for `.snxblk` block diagrams (#2796). Each fixture
//! under `tests/corpus/block_diagram/` must read with its stored id under the
//! current token and re-save to the exact same bytes. The files are
//! goldens: hand-written to the writer's layout
//! (`docs/architecture/snxblk-format.md`), never regenerated at test time.

use std::fs;
use std::path::{Path, PathBuf};

use signex_types::block_diagram::{
    Interface, LinkState, SNXBLK_FORMAT_V1, encode_block_diagram, parse_block_diagram,
};

/// Lower bound on `.snxblk` fixtures so deleting one cannot pass silently.
const MIN_BLOCK_DIAGRAM_FIXTURES: usize = 3;

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/block_diagram")
}

fn fixtures() -> Vec<PathBuf> {
    let mut found: Vec<_> = fs::read_dir(corpus_dir())
        .expect("block diagram corpus directory must be readable")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "snxblk"))
        .collect();
    found.sort();
    found
}

fn resave(text: &str) -> Result<String, String> {
    let read = parse_block_diagram(text).map_err(|e| format!("parse failed: {e}"))?;
    if read.minted_id {
        return Err("the fixture carries no document id".into());
    }
    if read.token != SNXBLK_FORMAT_V1 {
        return Err(format!("token {} is not the current one", read.token));
    }
    encode_block_diagram(&read.diagram).map_err(|e| format!("write failed: {e}"))
}

#[test]
fn block_diagram_fixtures_round_trip_byte_for_byte() {
    let found = fixtures();
    assert!(
        found.len() >= MIN_BLOCK_DIAGRAM_FIXTURES,
        "block diagram corpus shrank: {} < {MIN_BLOCK_DIAGRAM_FIXTURES}",
        found.len()
    );
    let failures: Vec<String> = found
        .iter()
        .filter_map(|path| {
            let on_disk = fs::read_to_string(path).expect("fixture must be UTF-8");
            match resave(&on_disk) {
                Ok(saved) if saved == on_disk => None,
                Ok(saved) => Some(format!(
                    "  {}: not byte-stable; re-saved as:\n{saved}",
                    path.display()
                )),
                Err(e) => Some(format!("  {}: {e}", path.display())),
            }
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} block diagram fixtures fail:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_dcdc_fixture_reads_as_the_mockup_diagram() {
    let text = fs::read_to_string(corpus_dir().join("dcdc-buck.snxblk")).unwrap();
    let diagram = parse_block_diagram(&text).unwrap().diagram;
    let names: Vec<&str> = diagram.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Power In",
            "DC/DC Buck",
            "MCU",
            "Connectors",
            "Debug probe",
            "Sensor board"
        ]
    );
    assert_eq!(diagram.groups.len(), 1);
    assert_eq!(diagram.connections.len(), 6);
    let buck = &diagram.blocks[1];
    assert_eq!(buck.position.x, 70.0);
    assert_eq!(buck.specs["Vout"], "3.30 V");
    assert_eq!(buck.sheet.as_ref().unwrap().path, "dcdc.snxsch");
    let sensor = diagram.blocks[5].sheet.as_ref().unwrap();
    assert_eq!(sensor.state, LinkState::Unlinked);
    assert_eq!(
        diagram.connections[5].interface,
        Interface::Other("SWD".into())
    );
    assert_eq!(diagram.connections[2].interface, Interface::I2c);
    assert_eq!(
        diagram.connections[0].derived_from.as_deref(),
        Some("VIN_12V")
    );
}
