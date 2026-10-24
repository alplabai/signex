//! Feeds arbitrary text to the `.snxsch` (TOML + TSV) reader. Any `Err` is
//! fine; a panic, overflow, or hang is the bug.
#![no_main]

use libfuzzer_sys::fuzz_target;
use signex_types::format::SnxSchematic;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = SnxSchematic::parse(text);
    }
});
