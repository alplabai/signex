//! The files Signex serves the bundled package beside its own: a Typst
//! package reads only inside its own root, so `#attachment("scope/a.csv")`
//! finds what it prints here. The world adds them under the package root:
//!
//! - `derived/attachments.json`: every attachment by its file, with its
//!   kind, whether it is present, the stale changes, the image path or the
//!   drawn scope plot and its labels, or the reason it cannot be drawn;
//! - `attachments/<file>`: each attachment image, as the project holds it;
//! - `derived/plots/<n>.svg`: each scope capture drawn as vectors.
//!
//! The first `#attachment` of a file decides how it is drawn.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::attachment::AttachmentKind;
use crate::bench::record;
use crate::report::attachment_states;
use crate::sources::NotebookSources;

/// The manifest's path in the package root.
pub(crate) const MANIFEST: &str = "derived/attachments.json";

/// The derived files for `sources`, as (path in the package root, bytes).
/// The manifest is always there, empty without attachments, so the
/// package never meets a missing file.
pub(crate) fn package_files(sources: &NotebookSources) -> Vec<(String, Vec<u8>)> {
    let found = record(sources);
    let states = attachment_states(sources, &found);
    let mut files = Vec::new();
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    for state in &states {
        let attachment = &state.attachment;
        if entries.contains_key(&attachment.file) {
            continue;
        }
        let path = attachment.path_in_notebook();
        let image = match (attachment.kind, sources.get(&path)) {
            (AttachmentKind::Image, Some(bytes)) => {
                files.push((path.clone(), bytes.to_vec()));
                Some(path)
            }
            _ => None,
        };
        let plot = state.plot.as_ref().map(|plot| {
            let svg = format!("derived/plots/{}.svg", entries.len() + 1);
            files.push((svg.clone(), plot.to_svg().into_bytes()));
            json!({ "svg": svg, "scope": plot })
        });
        entries.insert(
            attachment.file.clone(),
            json!({
                "kind": attachment.kind,
                "present": state.present,
                "image": image,
                "plot": plot,
                "error": state.error,
                "changes": state.changes,
            }),
        );
    }
    let manifest = json!({ "files": entries });
    files.push((MANIFEST.to_string(), manifest.to_string().into_bytes()));
    files
}
