//! End to end (#3116): `signex-cli library render`, against committed
//! goldens for the stock library's `R` symbol and `Chip-0603` footprint.
//! The writer prints every number to four decimals, so the bytes are the
//! contract. Regenerate with `SIGNEX_BLESS_LIBRARY_RENDER=1`.

#![allow(
    clippy::disallowed_methods,
    reason = "the tests run the signex-cli binary as a child process"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

const BLESS_VAR: &str = "SIGNEX_BLESS_LIBRARY_RENDER";

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/library_render")
        .join(name)
}

fn render(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_signex-cli"))
        .args(["library", "render"])
        .args(args)
        .output()
        .expect("run signex-cli");
    (
        out.status.code().expect("exit code"),
        String::from_utf8(out.stdout).expect("utf-8"),
    )
}

fn written(out: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_stock_r_symbol_and_a_footprint_match_their_goldens() {
    let out = tempfile::tempdir().unwrap();
    let stock = repo("assets/stock-library");
    let (code, stdout) = render(&[
        stock.to_str().unwrap(),
        "--out",
        out.path().to_str().unwrap(),
        "--symbol",
        "R",
        "--footprint",
        "Chip-0603",
    ]);
    assert_eq!(code, 0, "{stdout}");
    let names = written(out.path());
    assert_eq!(names.len(), 2, "{names:?}");
    for name in &names {
        let kind = if name.starts_with("symbol-r-") {
            "symbol-r.svg"
        } else {
            "footprint-chip-0603.svg"
        };
        assert!(name.starts_with(&kind[..kind.len() - 4]), "{name}");
        let svg = std::fs::read_to_string(out.path().join(name)).unwrap();
        assert!(svg.starts_with("<svg "), "{svg}");
        let path = golden(kind);
        if std::env::var(BLESS_VAR).is_ok_and(|v| v == "1") {
            std::fs::write(&path, &svg).unwrap();
        }
        let want = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("no golden {}", path.display()));
        assert_eq!(svg, want, "{name} differs from {}", path.display());
    }
}

#[test]
fn file_names_are_kind_slug_and_uuid_prefix() {
    let out = tempfile::tempdir().unwrap();
    let (code, _) = render(&[
        repo("assets/stock-library").to_str().unwrap(),
        "--out",
        out.path().to_str().unwrap(),
        "--symbol",
        "C_Polarized",
    ]);
    assert_eq!(code, 0);
    let [name] = written(out.path()).try_into().unwrap();
    let stem = name.strip_suffix(".svg").unwrap();
    let rest = stem.strip_prefix("symbol-c-polarized-").unwrap();
    assert_eq!(rest.len(), 8);
    assert!(rest.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn the_whole_conformance_library_renders() {
    let out = tempfile::tempdir().unwrap();
    let (code, stdout) = render(&[
        repo("docs/fixtures/library-format/library")
            .to_str()
            .unwrap(),
        "--out",
        out.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stdout}");
    let names = written(out.path());
    assert_eq!(
        names.iter().filter(|n| n.starts_with("symbol-")).count(),
        2,
        "{names:?}"
    );
    assert_eq!(
        names.iter().filter(|n| n.starts_with("footprint-")).count(),
        4,
        "{names:?}"
    );
    for name in &names {
        let svg = std::fs::read_to_string(out.path().join(name)).unwrap();
        assert!(
            svg.starts_with("<svg ") && svg.trim_end().ends_with("</svg>"),
            "{name}"
        );
    }
}

#[test]
fn png_is_written_beside_the_svg() {
    let out = tempfile::tempdir().unwrap();
    let (code, _) = render(&[
        repo("assets/stock-library").to_str().unwrap(),
        "--out",
        out.path().to_str().unwrap(),
        "--symbol",
        "R",
        "--png",
    ]);
    assert_eq!(code, 0);
    let pngs: Vec<_> = written(out.path())
        .into_iter()
        .filter(|n| n.ends_with(".png"))
        .collect();
    let bytes = std::fs::read(out.path().join(&pngs[0])).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn an_unknown_selector_fails_and_a_bad_flag_is_a_usage_error() {
    let out = tempfile::tempdir().unwrap();
    let (code, stdout) = render(&[
        repo("assets/stock-library").to_str().unwrap(),
        "--out",
        out.path().to_str().unwrap(),
        "--symbol",
        "no-such-symbol",
    ]);
    assert_eq!(code, 1);
    assert!(
        stdout.contains("no primitive matches `no-such-symbol`"),
        "{stdout}"
    );
    let (code, _) = render(&["lib", "--bogus"]);
    assert_eq!(code, 2);
}
