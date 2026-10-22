//! The stock-library generator must be idempotent: designs reference
//! stock primitives by `PrimitiveRef { library_id, uuid }`, so a rerun
//! that re-minted ids or timestamps would silently break them (#323).

#[path = "../examples/build_stock_library/main.rs"]
mod stock_gen;

use std::path::PathBuf;

fn stock_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/footprints")
}

#[test]
fn generating_twice_is_byte_identical() {
    let first = stock_gen::render_all().expect("first render");
    let second = stock_gen::render_all().expect("second render");
    assert_eq!(first.len(), 5, "five stock footprints");
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(a.file_name, b.file_name);
        assert_eq!(
            a.text, b.text,
            "{} differs between two generator runs: an identity field is minted per run",
            a.file_name
        );
    }
}

#[test]
fn committed_stock_files_match_the_generator_and_id_table() {
    for file in stock_gen::render_all().expect("render") {
        let path = stock_dir().join(file.file_name);
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
            .replace("\r\n", "\n");
        assert_eq!(
            committed, file.text,
            "{} is stale: rerun `cargo run --example build_stock_library -p signex-app`",
            file.file_name
        );
    }
    for row in stock_gen::ids::STOCK_IDS {
        let path = stock_dir().join(row.file_name);
        let committed = std::fs::read_to_string(&path).expect("read committed file");
        for id in row.all_ids() {
            assert!(
                committed.contains(&id.to_string()),
                "{} does not carry pinned id {id}",
                row.file_name
            );
        }
    }
}
