//! Reading a document's id from the head of its file (#2172), so a project
//! can list every sheet it references with its id without opening each one.
//!
//! The TOML kinds keep the id as a top-level key, before the first table, so
//! only that head is parsed; a project is one small JSON object.

use serde_json::Value;
use uuid::Uuid;

use crate::document_id::DocumentId;

/// The top-level TOML key that carries the id of `extension`'s file kind.
fn toml_key(extension: &str) -> Option<&'static str> {
    match extension {
        "snxsch" => Some("schematic_id"),
        "snxpcb" => Some("pcb_id"),
        "snxsym" | "snxfpt" => Some("file_uuid"),
        _ => None,
    }
}

/// The id stored in `bytes`, a file with extension `extension` (lowercase, no
/// dot). `None` when the file kind has no id, the key is absent, or it holds
/// no document id (the nil uuid, text that is not a uuid).
pub fn document_id_in_header(extension: &str, bytes: &[u8]) -> Option<DocumentId> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    if extension == "snxprj" {
        let value: Value = serde_json::from_str(text).ok()?;
        let id = value.get("document_id")?.as_str()?;
        return id.parse().ok();
    }
    let key = toml_key(extension)?;
    let head_len = text
        .lines()
        .scan(0usize, |at, line| {
            let start = *at;
            *at += line.len() + 1;
            Some((start, line))
        })
        .find(|(_, line)| line.trim_start().starts_with('['))
        .map_or(text.len(), |(start, _)| start);
    let head: toml::Table = toml::from_str(&text[..head_len.min(text.len())]).ok()?;
    let uuid = Uuid::parse_str(head.get(key)?.as_str()?).ok()?;
    DocumentId::from_uuid(uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0193a2f0-7b1c-7cde-8123-456789abcdef";

    #[test]
    fn each_toml_kind_reads_its_own_key() {
        for (extension, key) in [
            ("snxsch", "schematic_id"),
            ("snxpcb", "pcb_id"),
            ("snxsym", "file_uuid"),
            ("snxfpt", "file_uuid"),
        ] {
            let file = format!("format = \"x/1\"\n{key} = \"{ID}\"\n[extras]\nnote = \"a\"\n");
            assert_eq!(
                document_id_in_header(extension, file.as_bytes()),
                Some(ID.parse().unwrap()),
                "{extension}"
            );
        }
    }

    #[test]
    fn a_key_of_another_kind_is_not_read() {
        let file = format!("pcb_id = \"{ID}\"\n");
        assert_eq!(document_id_in_header("snxsch", file.as_bytes()), None);
    }

    #[test]
    fn only_the_head_is_parsed_so_a_body_that_is_not_toml_does_not_hide_the_id() {
        let file = format!("schematic_id = \"{ID}\"\n[wires]\ncontent = = = not toml\n");
        assert_eq!(
            document_id_in_header("snxsch", file.as_bytes()),
            Some(ID.parse().unwrap())
        );
    }

    #[test]
    fn a_key_after_the_first_table_is_not_the_documents() {
        let file = format!("format = \"snxsch/1\"\n[extras]\nschematic_id = \"{ID}\"\n");
        assert_eq!(document_id_in_header("snxsch", file.as_bytes()), None);
    }

    #[test]
    fn the_nil_uuid_and_junk_are_no_id() {
        let nil = "schematic_id = \"00000000-0000-0000-0000-000000000000\"\n";
        assert_eq!(document_id_in_header("snxsch", nil.as_bytes()), None);
        assert_eq!(document_id_in_header("snxsch", b"schematic_id = 7\n"), None);
        assert_eq!(document_id_in_header("snxsch", b"= = ="), None);
        assert_eq!(document_id_in_header("snxsch", &[0xff, 0xfe, 0x00]), None);
    }

    #[test]
    fn a_project_reads_its_document_id_key() {
        let file = format!("{{\"format\":\"snxprj/1\",\"document_id\":\"{ID}\"}}");
        assert_eq!(
            document_id_in_header("snxprj", file.as_bytes()),
            Some(ID.parse().unwrap())
        );
        assert_eq!(
            document_id_in_header("snxprj", b"{\"format\":\"snxprj/1\"}"),
            None
        );
        assert_eq!(document_id_in_header("snxprj", b""), None);
    }

    #[test]
    fn a_file_kind_without_an_id_has_none() {
        assert_eq!(document_id_in_header("txt", b"schematic_id = \"x\""), None);
    }
}
