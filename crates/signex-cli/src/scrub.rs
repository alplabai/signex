//! What an attached run may see of the running app's answer (#1924). The app
//! answers a query across every project it has open; this run may only see
//! the documents inside its own folders, so the CLI, which owns the policy,
//! filters the answer before it prints it: an object that names a file outside
//! them is dropped, and so is the object that holds it (an element whose
//! document is outside is dropped, a `resolve` of such an id answers
//! "unknown", as for an id that does not exist).

use serde_json::{Map, Value, json};

/// Whether a listed object may be shown, given what is inside.
type Judge = fn(&Map<String, Value>, &dyn Fn(&str) -> bool) -> bool;

/// `data` without what names a document or file `inside` does not accept, or
/// `None` when the answer itself is such a document or element. Only the keys
/// the queries use for documents are judged: the answer's `document`,
/// `element` and `project`, and the lists `documents`, `sheets`, `elements` and
/// `not_loaded`. Everything else is the answer's own data and is left whole: a
/// `path` inside it (an instance path, a library's relative path) is not a
/// file. A `count` beside a `documents` or `elements` list is recomputed.
pub(crate) fn scrub_data(data: Value, inside: &dyn Fn(&str) -> bool) -> Option<Value> {
    let Value::Object(mut map) = data else {
        return Some(data);
    };
    for key in ["document", "project"] {
        if let Some(Value::Object(document)) = map.get(key)
            && !document_ok(document, inside)
        {
            return None;
        }
    }
    if let Some(Value::Object(element)) = map.get("element")
        && !element_ok(element, inside)
    {
        return None;
    }
    // Fail closed: a listed document or element is kept only if it says whose
    // it is, and what it says is then proven.
    let lists: [(&str, Judge); 3] = [
        ("documents", document_ok),
        ("sheets", document_ok),
        ("elements", element_ok),
    ];
    for (key, keep) in lists {
        if let Some(Value::Array(items)) = map.get_mut(key) {
            items.retain(|item| item.as_object().is_some_and(|object| keep(object, inside)));
        }
    }
    if let Some(Value::Array(items)) = map.get_mut("not_loaded") {
        items.retain(|item| item.as_str().is_none_or(inside));
    }
    // The files a project's save would write (never printed, see `attach.rs`).
    if let Some(Value::Array(items)) = map.get_mut("save_writes") {
        items.retain(|item| item.as_str().is_some_and(inside));
    }
    let listed = ["documents", "elements"]
        .into_iter()
        .filter_map(|key| map.get(key).and_then(Value::as_array))
        .map(Vec::len)
        .collect::<Vec<_>>();
    if let ([len], true) = (
        listed.as_slice(),
        map.get("count").is_some_and(Value::is_number),
    ) {
        map.insert("count".into(), json!(len));
    }
    Some(Value::Object(map))
}

/// An element is shown only if its `document` is.
fn element_ok(element: &Map<String, Value>, inside: &dyn Fn(&str) -> bool) -> bool {
    element
        .get("document")
        .and_then(Value::as_object)
        .is_some_and(|document| document_ok(document, inside))
}

/// Whether an object that names a document may be shown. It must say whose it
/// is, by a `path` key: a string is inside if that path is; a null (a sheet
/// never saved) is shown only if the project that owns it (`project.path`) is
/// inside. A `project` it names must be inside too. With no proof it is hidden.
fn document_ok(document: &Map<String, Value>, inside: &dyn Fn(&str) -> bool) -> bool {
    let project = document
        .get("project")
        .and_then(|project| project.get("path"))
        .and_then(Value::as_str);
    let own = match document.get("path") {
        Some(Value::String(path)) => inside(path),
        Some(_) => project.is_some_and(inside),
        None => false,
    };
    own && project.is_none_or(inside)
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

    /// The files a project's save writes are kept only when inside.
    #[test]
    fn a_projects_save_writes_are_filtered_to_what_is_inside() {
        let info = json!({"path": "/a/P.snxprj", "kind": "project",
            "save_writes": ["/a/libs/n.snxlib", "/b/theirs.snxlib", 7]});
        let kept = scrub_data(info, &inside).unwrap();
        assert_eq!(kept["save_writes"], json!(["/a/libs/n.snxlib"]));
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
