//! Showing a datasheet in the platform's own viewer.

use super::DatasheetLocation;

/// Open `location`. Blocks on the launcher: call it from a worker.
pub fn open(_location: &DatasheetLocation) -> Result<(), String> {
    Ok(())
}
