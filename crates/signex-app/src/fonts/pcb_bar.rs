//! The PCB Active Bar groups' last-used rows (#2392) in `prefs.json`.

use super::*;
use crate::app::pcb_bar_groups::PcbBarLastTools;

/// The `prefs.json` key of the PCB Active Bar's last-used rows.
const PCB_BAR_LAST_TOOLS_KEY: &str = "pcb_bar_last_tools";

/// Read each group's last-used row. A missing key, or one written by a
/// build with other rows, reads as nothing used.
pub(crate) fn read_pcb_bar_last_tools_from(json: Option<&serde_json::Value>) -> PcbBarLastTools {
    PcbBarLastTools::from_json(json.and_then(|json| json.get(PCB_BAR_LAST_TOOLS_KEY)))
}

/// Read the last-used rows from the prefs file.
#[cfg(test)]
pub(crate) fn read_pcb_bar_last_tools() -> PcbBarLastTools {
    read_pcb_bar_last_tools_from(load_prefs_json().as_ref())
}

/// Persist the last-used rows without clobbering other keys.
pub(crate) fn write_pcb_bar_last_tools(last: &PcbBarLastTools) {
    update_prefs_json(&prefs_path(), PCB_BAR_LAST_TOOLS_KEY, |prefs| {
        prefs.insert(PCB_BAR_LAST_TOOLS_KEY.to_string(), last.to_json());
    })
}
