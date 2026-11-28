//! The project's min slot (mockup row 179d), stored with the rules as the
//! optional number `rules.min_slot` (mm). Present, the section is written
//! at schema 2 (`RULES_SCHEMA_V2`). Absent when no fab tier or profile
//! applied one.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A min slot the outline checks can use: finite and positive.
pub(super) fn is_usable(mm: f64) -> bool {
    super::router_tool::is_usable_diameter(mm)
}

/// Read `rules.min_slot`; a value that is not a positive number reads as
/// `None` rather than refusing the rules.
pub(super) fn lenient_min_slot<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<Value>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let usable = value.as_f64().filter(|mm| is_usable(*mm));
    if usable.is_none() {
        tracing::warn!(
            target: "signex::project",
            "The project's min slot ({value}) is not a positive number; it is ignored."
        );
    }
    Ok(usable)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::{
        ProjectData, ProjectRules, RULES_SCHEMA_V1, RULES_SCHEMA_V2, encode_project,
        parse_project_bytes,
    };

    const WITH_MIN_SLOT: &str = r#"{
  "active_variant": null,
  "dir": "",
  "enable_git": false,
  "format": "snxprj/1",
  "libraries": [],
  "name": "P",
  "pcb_file": null,
  "rules": {
    "min_slot": 0.8,
    "schema": 2
  },
  "schematic_root": null,
  "sheets": [],
  "variant_definitions": []
}"#;

    const WITHOUT_MIN_SLOT: &str = r#"{
  "active_variant": null,
  "dir": "",
  "enable_git": false,
  "format": "snxprj/1",
  "libraries": [],
  "name": "P",
  "pcb_file": null,
  "rules": {
    "schema": 1
  },
  "schematic_root": null,
  "sheets": [],
  "variant_definitions": []
}"#;

    fn opened(json: &str) -> ProjectData {
        parse_project_bytes(Path::new("P.snxprj"), json.as_bytes(), |_| false).unwrap()
    }

    fn written(data: &ProjectData) -> String {
        String::from_utf8(encode_project(data).unwrap()).unwrap()
    }

    fn rules(data: &ProjectData) -> &ProjectRules {
        data.rules.as_ref().expect("the rules are read")
    }

    #[test]
    fn a_section_with_a_min_slot_reads_it_and_writes_it_back_byte_for_byte() {
        let data = opened(WITH_MIN_SLOT);
        assert!(!data.has_unreadable_rules(), "schema 2 is read");
        assert_eq!(rules(&data).min_slot(), Some(0.8));
        assert!(rules(&data).unknown.get("min_slot").is_none());
        let text = written(&data);
        assert_eq!(text, WITH_MIN_SLOT);
        assert_eq!(text.matches("min_slot").count(), 1);
    }

    #[test]
    fn a_section_without_a_min_slot_round_trips_unchanged_at_schema_1() {
        let data = opened(WITHOUT_MIN_SLOT);
        assert_eq!(rules(&data).min_slot(), None);
        assert_eq!(written(&data), WITHOUT_MIN_SLOT);
    }

    #[test]
    fn setting_the_min_slot_steps_the_schema_to_2_and_clearing_it_steps_back() {
        let mut data = opened(WITHOUT_MIN_SLOT);
        data.rules.as_mut().unwrap().set_min_slot(Some(0.8));
        assert_eq!(rules(&data).schema, Some(RULES_SCHEMA_V2));
        assert_eq!(written(&data), WITH_MIN_SLOT);
        // What a save reads back equals what was stored.
        assert_eq!(opened(&written(&data)).rules, data.rules);
        data.rules.as_mut().unwrap().set_min_slot(None);
        assert_eq!(rules(&data).schema, Some(RULES_SCHEMA_V1));
        assert_eq!(written(&data), WITHOUT_MIN_SLOT);
    }

    #[test]
    fn a_min_slot_set_on_the_field_still_writes_schema_2() {
        let mut data = opened(WITHOUT_MIN_SLOT);
        data.rules.as_mut().unwrap().min_slot = Some(0.8);
        assert_eq!(written(&data), WITH_MIN_SLOT);
    }

    #[test]
    fn a_value_that_is_not_a_positive_number_reads_as_absent_and_is_not_kept() {
        for bad in ["-1.0", "0", "\"0.8 mm\"", "null"] {
            let json = WITH_MIN_SLOT.replace("0.8", bad);
            let data = opened(&json);
            assert_eq!(rules(&data).min_slot, None, "{bad}");
            assert!(rules(&data).unknown.get("min_slot").is_none(), "{bad}");
            assert_eq!(written(&data), WITHOUT_MIN_SLOT, "{bad}");
        }
    }

    #[test]
    fn a_later_rules_schema_is_still_kept_unapplied() {
        let json = WITH_MIN_SLOT.replace("\"schema\": 2", "\"schema\": 3");
        let data = opened(&json);
        assert!(data.has_unreadable_rules());
        assert_eq!(written(&data), json);
    }

    #[test]
    fn the_min_slot_is_no_rule_content() {
        let mut data = opened(WITHOUT_MIN_SLOT);
        let before = rules(&data).clone();
        data.rules.as_mut().unwrap().set_min_slot(Some(0.6));
        let after = rules(&data);
        assert_eq!(
            super::super::rule_set_sha256(&after.erc, &after.drc),
            super::super::rule_set_sha256(&before.erc, &before.drc)
        );
    }
}
