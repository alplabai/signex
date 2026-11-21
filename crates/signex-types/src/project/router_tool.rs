//! The project's router tool diameter (mockup row 179d), stored in the
//! `.snxprj` as the optional top-level number `router_tool_diameter` (mm).
//! Absent when no fab tier or profile applied one.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A diameter the outline checks can use: finite and positive.
pub(super) fn is_usable_diameter(mm: f64) -> bool {
    mm.is_finite() && mm > 0.0
}

/// Read `router_tool_diameter`; a value that is not a positive number reads
/// as `None` rather than refusing the project.
pub(super) fn lenient_router_tool_diameter<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<Value>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let usable = value.as_f64().filter(|mm| is_usable_diameter(*mm));
    if usable.is_none() {
        tracing::warn!(
            target: "signex::project",
            "The project's router tool diameter ({value}) is not a positive number; it is ignored."
        );
    }
    Ok(usable)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::{ProjectData, encode_project, parse_project_bytes};

    const WITH_DIAMETER: &str = r#"{
  "active_variant": null,
  "dir": "",
  "enable_git": false,
  "format": "snxprj/1",
  "libraries": [],
  "name": "P",
  "pcb_file": null,
  "router_tool_diameter": 1.6,
  "schematic_root": null,
  "sheets": [],
  "variant_definitions": []
}"#;

    const WITHOUT_DIAMETER: &str = r#"{
  "active_variant": null,
  "dir": "",
  "enable_git": false,
  "format": "snxprj/1",
  "libraries": [],
  "name": "P",
  "pcb_file": null,
  "schematic_root": null,
  "sheets": [],
  "variant_definitions": []
}"#;

    fn opened(json: &str) -> ProjectData {
        parse_project_bytes(Path::new("P.snxprj"), json.as_bytes()).unwrap()
    }

    #[test]
    fn a_file_with_the_diameter_reads_it_into_the_field_and_writes_it_back_byte_for_byte() {
        let data = opened(WITH_DIAMETER);
        assert_eq!(data.router_tool_diameter, Some(1.6));
        assert_eq!(data.router_tool_diameter(), Some(1.6));
        assert!(
            data.unknown.get("router_tool_diameter").is_none(),
            "the key must not also sit in the unknown bag"
        );
        let written = String::from_utf8(encode_project(&data).unwrap()).unwrap();
        assert_eq!(written, WITH_DIAMETER);
        assert_eq!(written.matches("router_tool_diameter").count(), 1);
    }

    #[test]
    fn a_file_without_the_diameter_round_trips_unchanged_and_stays_without_it() {
        let data = opened(WITHOUT_DIAMETER);
        assert_eq!(data.router_tool_diameter, None);
        let written = String::from_utf8(encode_project(&data).unwrap()).unwrap();
        assert_eq!(written, WITHOUT_DIAMETER);
    }

    #[test]
    fn setting_the_diameter_writes_the_key_and_clearing_it_removes_it() {
        let mut data = opened(WITHOUT_DIAMETER);
        data.set_router_tool_diameter(Some(1.6));
        let written = String::from_utf8(encode_project(&data).unwrap()).unwrap();
        assert_eq!(written, WITH_DIAMETER);
        data.set_router_tool_diameter(None);
        let written = String::from_utf8(encode_project(&data).unwrap()).unwrap();
        assert_eq!(written, WITHOUT_DIAMETER);
    }

    #[test]
    fn a_value_that_is_not_a_positive_number_reads_as_absent_and_is_not_kept() {
        for bad in ["-1.0", "0", "\"2 mm\"", "null"] {
            let json = WITH_DIAMETER.replace("1.6", bad);
            let data = opened(&json);
            assert_eq!(data.router_tool_diameter, None, "{bad}");
            assert!(data.unknown.get("router_tool_diameter").is_none(), "{bad}");
            let written = String::from_utf8(encode_project(&data).unwrap()).unwrap();
            assert_eq!(written, WITHOUT_DIAMETER, "{bad}");
        }
    }
}
