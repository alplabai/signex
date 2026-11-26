//! Fixed ids, times and paths: the fixture bytes must not depend on the clock.

use super::*;

/// Set to `1` to rewrite the committed fixtures from the code.
pub(crate) const BLESS_VAR: &str = "SIGNEX_BLESS_LIBRARY_FORMAT";

pub(crate) const LIBRARY_ID: &str = "01940000-0000-7000-8000-000000000001";
pub(crate) const SYMBOL_FILE: &str = "01940000-0000-7000-8000-000000000010";
pub(crate) const RESISTOR_SYMBOL: &str = "01940000-0000-7000-8000-000000000011";
pub(crate) const NAND_SYMBOL: &str = "01940000-0000-7000-8000-000000000012";
pub(crate) const R0603_FOOTPRINT: &str = "01940000-0000-7000-8000-000000000021";
pub(crate) const R0805_FOOTPRINT: &str = "01940000-0000-7000-8000-000000000022";
pub(crate) const R0603_FILE: &str = "01940000-0000-7000-8000-000000000031";
pub(crate) const R0805_FILE: &str = "01940000-0000-7000-8000-000000000032";
pub(crate) const SOT23_FOOTPRINT: &str = "01940000-0000-7000-8000-000000000023";
pub(crate) const SOT23_FILE: &str = "01940000-0000-7000-8000-000000000033";
pub(crate) const ROW_R10K: &str = "01940000-0000-7000-8000-000000000041";
pub(crate) const ROW_R1K: &str = "01940000-0000-7000-8000-000000000042";
pub(crate) const ROW_NAND: &str = "01940000-0000-7000-8000-000000000043";

pub(crate) const SNXLIB_NAME: &str = "Conformance.snxlib";
pub(crate) const SYMBOL_ENVELOPE: &str = "symbols/conformance-symbols.snxsym";
pub(crate) const LAYERS: [&str; 3] = ["Top Layer", "Top Solder", "Top Paste"];

pub(crate) fn id(text: &str) -> Uuid {
    Uuid::parse_str(text).expect("fixture uuid constant")
}

pub(crate) fn lib_ref(uuid: &str) -> PrimitiveRef {
    PrimitiveRef::new(id(LIBRARY_ID), id(uuid))
}

pub(crate) fn created() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

pub(crate) fn updated() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
}

pub(crate) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures/library-format")
}
