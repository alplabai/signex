//! What an attached run may see of the running app's answer (#1924). The app
//! answers a query across every project it has open; this run may only see
//! the documents inside its own folders, so the CLI, which owns the policy,
//! filters the answer before it prints it: an object that names a file outside
//! them is dropped, and so is the object that holds it (an element whose
//! document is outside is dropped, a `resolve` of such an id answers
//! "unknown", as for an id that does not exist).

use serde_json::{Map, Value, json};

/// `data` without what names a file `inside` does not accept, or `None` when
/// the answer itself is such an object. Every object with a string `path`, and
/// every entry of a `not_loaded` list, is checked; a `count` beside a
/// `documents` or `elements` list is recomputed.
pub(crate) fn scrub_data(data: Value, inside: &dyn Fn(&str) -> bool) -> Option<Value> {
    match data {
        Value::Object(map) => scrub_object(map, inside),
        Value::Array(items) => Some(Value::Array(scrub_items(items, inside))),
        other => Some(other),
    }
}

fn scrub_items(items: Vec<Value>, inside: &dyn Fn(&str) -> bool) -> Vec<Value> {
    items
        .into_iter()
        .filter_map(|item| scrub_data(item, inside))
        .collect()
}

fn scrub_object(map: Map<String, Value>, inside: &dyn Fn(&str) -> bool) -> Option<Value> {
    if !proven_inside(&map, inside) {
        return None;
    }
    let mut out = Map::new();
    for (key, child) in map {
        let kept = match child {
            // Fail closed: a listed document or element is kept only if it
            // says whose it is (a document names a `path`, an element a
            // `document` with one), and what it says is then proven.
            Value::Array(items) if key == "documents" => Some(Value::Array(scrub_items(
                items
                    .into_iter()
                    .filter(|item| item.get("path").is_some())
                    .collect(),
                inside,
            ))),
            Value::Array(items) if key == "elements" => Some(Value::Array(scrub_items(
                items
                    .into_iter()
                    .filter(|item| item.get("document").and_then(|d| d.get("path")).is_some())
                    .collect(),
                inside,
            ))),
            Value::Array(items) if key == "not_loaded" => Some(Value::Array(
                items
                    .into_iter()
                    .filter(|item| item.as_str().is_none_or(inside))
                    .collect(),
            )),
            // An object that holds a dropped one is dropped with it.
            Value::Object(object) => Some(scrub_object(object, inside)?),
            other => scrub_data(other, inside),
        };
        if let Some(kept) = kept {
            out.insert(key, kept);
        }
    }
    let listed = ["documents", "elements"]
        .into_iter()
        .filter_map(|key| out.get(key).and_then(Value::as_array))
        .map(Vec::len)
        .collect::<Vec<_>>();
    if let ([len], true) = (
        listed.as_slice(),
        out.get("count").is_some_and(Value::is_number),
    ) {
        out.insert("count".into(), json!(len));
    }
    Some(Value::Object(out))
}

/// Whether an object that names a document may be shown. One without a `path`
/// key names none. One with a string `path` is inside if that path is. One
/// whose `path` is null (a sheet never saved) or of another shape is shown only
/// if the project that owns it (`project.path`) is inside; with no proof it is
/// hidden, not shown.
fn proven_inside(map: &Map<String, Value>, inside: &dyn Fn(&str) -> bool) -> bool {
    match map.get("path") {
        None => true,
        Some(Value::String(path)) => inside(path),
        Some(_) => map
            .get("project")
            .and_then(|project| project.get("path"))
            .and_then(Value::as_str)
            .is_some_and(inside),
    }
}

/// The files a `changed` list names that `inside` accepts.
pub(crate) fn scrub_changed(changed: Vec<String>, inside: &dyn Fn(&str) -> bool) -> Vec<String> {
    changed.into_iter().filter(|path| inside(path)).collect()
}

/// The answer of `resolve` for an id that is not known.
pub(crate) fn unknown_id(id: &str) -> Value {
    json!({"found": false, "what": "unknown", "id": id})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(path: &str) -> bool {
        path.starts_with("/a/")
    }

    #[test]
    fn a_list_keeps_the_documents_inside_and_recounts() {
        let data = json!({"count": 3, "documents": [
            {"id": "1", "path": "/a/x.snxsch", "project": {"path": "/a/P.snxprj"}},
            {"id": "2", "path": "/b/y.snxsch"},
            {"id": "3", "path": "/a/z.snxsch", "project": {"path": "/b/Q.snxprj"}},
        ]});
        let kept = scrub_data(data, &inside).unwrap();
        assert_eq!(kept["count"], 1);
        assert_eq!(kept["documents"].as_array().unwrap().len(), 1);
        assert_eq!(kept["documents"][0]["id"], "1");
    }

    #[test]
    fn an_element_of_a_document_outside_is_dropped_and_not_loaded_is_filtered() {
        let element = |path: &str, text: &str| json!({"id": "e", "text": text, "document": {"id": "d", "path": path, "name": "n"}});
        let data = json!({"count": 2,
            "elements": [element("/a/x", "mine"), element("/b/y", "SECRET")],
            "not_loaded": ["/a/p", "/b/q"]});
        let kept = scrub_data(data, &inside).unwrap();
        assert_eq!(kept["count"], 1);
        assert_eq!(kept["elements"][0]["text"], "mine");
        assert_eq!(kept["not_loaded"], json!(["/a/p"]));
        assert!(!kept.to_string().contains("SECRET"));
    }

    #[test]
    fn an_answer_that_is_itself_outside_is_gone() {
        let outside = json!({"found": true, "what": "element",
            "element": {"id": "e", "document": {"path": "/b/y"}}});
        assert!(scrub_data(outside, &inside).is_none());
        let document = json!({"found": true, "what": "document", "document": {"path": "/b/y"}});
        assert!(scrub_data(document, &inside).is_none());
        let mine = json!({"found": true, "what": "document", "document": {"path": "/a/y"}});
        assert_eq!(scrub_data(mine.clone(), &inside), Some(mine));
        assert_eq!(unknown_id("u")["what"], "unknown");
    }

    /// An unsaved sheet has a null `path`: it is shown only if its project is
    /// proven inside; its elements say no project, so they are hidden.
    #[test]
    fn a_sheet_never_saved_is_shown_only_when_its_project_is_proven_inside() {
        let element = |text: &str| json!({"id": "e", "text": text, "document": {"id": "d", "path": null, "name": "n"}});
        let data = json!({"count": 4, "documents": [
            {"id": "mine", "path": null, "project": {"path": "/a/P.snxprj"}},
            {"id": "theirs", "path": null, "project": {"path": "/b/Q.snxprj"}},
            {"id": "loose", "path": null, "project": null},
            {"id": "nokey"},
        ]});
        let kept = scrub_data(data, &inside).unwrap();
        assert_eq!(kept["count"], 1);
        assert_eq!(kept["documents"][0]["id"], "mine");

        let elements = json!({"count": 2, "elements": [element("SECRET"), json!({"id": "x"})],
            "not_loaded": []});
        let kept = scrub_data(elements, &inside).unwrap();
        assert_eq!(kept["count"], 0);
        assert!(!kept.to_string().contains("SECRET"));
        let unsaved = json!({"found": true, "what": "element", "element": element("SECRET")});
        assert!(scrub_data(unsaved, &inside).is_none());
    }

    #[test]
    fn changed_keeps_only_files_inside() {
        let kept = scrub_changed(vec!["/a/x".into(), "/b/y".into()], &inside);
        assert_eq!(kept, ["/a/x"]);
    }
}
