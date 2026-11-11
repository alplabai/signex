//! End to end (#2178): an agent reads a document, applies a list of edit
//! commands against the revision it read, and takes back only its own step,
//! through the real `signex-cli` binary.

mod support;

use serde_json::{Value, json};
use support::{Live, fixture, run_cli, snapshot, temp_root};

/// Starts a live run on the fixture, with the sheet open, and reads the
/// sheet's revision.
struct Agent {
    live: Live,
}

impl Agent {
    fn start(fx: &support::Fixture, extra: &[&str]) -> Self {
        let mut args = vec!["run", fx.project.to_str().unwrap(), "--keep-going"];
        args.extend_from_slice(extra);
        let mut live = Live::start(&args);
        let open = live.send(&json!({"id": "open_document", "args": {"path": "main.snxsch"}}));
        assert_eq!(open["status"], "answered", "{open}");
        Self { live }
    }

    fn read(&mut self, kinds: &str) -> Value {
        let read = self.live.send(&json!({"id": "read_document",
            "args": {"document": "main.snxsch", "kinds": kinds}}));
        assert_eq!(read["status"], "answered", "{read}");
        read["data"].clone()
    }

    fn revision(&mut self) -> u64 {
        self.read("junction")["revision"]
            .as_u64()
            .expect("a revision")
    }

    fn apply(&mut self, revision: u64, edits: Value) -> Value {
        self.live.send(&json!({"id": "apply_edits", "args": {
            "document": "main.snxsch", "expect_revision": revision, "edits": edits}}))
    }

    fn revert(&mut self, group: u64) -> Value {
        self.live.send(&json!({"id": "revert",
            "args": {"document": "main.snxsch", "group": group}}))
    }
}

fn edits() -> Value {
    json!([
        {"id": "place_junction", "args": {"at": [5.08, 0]}},
        {"id": "place_no_connect", "args": {"at": [7.62, 2.54]}},
        {"id": "place_net_label", "args": {"at": [10.16, 5.08], "text": "AGENT"}},
    ])
}

/// By file name: a reported path may carry a verbatim prefix on Windows.
fn names_the_sheet(changed: &Value) -> bool {
    changed.as_array().is_some_and(|paths| {
        paths
            .iter()
            .any(|p| p.as_str().unwrap().ends_with("main.snxsch"))
    })
}

#[test]
fn an_agent_applies_one_step_against_its_revision_and_reverts_only_that_step() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let before = snapshot(&root);
    let mut agent = Agent::start(&fx, &[]);
    let read = agent.revision();

    let applied = agent.apply(read, edits());
    assert_eq!(applied["status"], "answered", "{applied}");
    assert_eq!(applied["data"]["changed"], true);
    assert_eq!(
        applied["data"]["label"],
        "Agent: place_junction, place_no_connect, place_net_label"
    );
    assert!(names_the_sheet(&applied["changed"]), "{applied}");
    let group = applied["data"]["group"].as_u64().expect("a group id");
    let now = agent.revision();
    assert_eq!(applied["data"]["revision"], now);
    assert_ne!(now, read);
    assert_eq!(agent.read("junction")["total"], 1);

    // Stale: the revision before the batch. Nothing moves.
    let stale = agent.apply(read, edits());
    assert_eq!(stale["status"], "refused", "{stale}");
    assert!(
        stale["reason"]
            .as_str()
            .unwrap()
            .contains("changed since revision")
    );
    assert_eq!(stale["changed"], json!([]));
    assert_eq!(agent.revision(), now);

    // A failing edit in the middle: nothing moves either.
    let failing = agent.apply(
        now,
        json!([
            {"id": "place_junction", "args": {"at": [20.32, 20.32]}},
            {"id": "place_wire", "args": {"start": [5.08, 5.08], "end": [5.08, 5.08]}},
        ]),
    );
    assert_eq!(failing["status"], "refused", "{failing}");
    assert!(
        failing["reason"]
            .as_str()
            .unwrap()
            .contains("edit 2 (`place_wire`)")
    );
    assert_eq!(failing["changed"], json!([]));
    assert_eq!(agent.revision(), now);

    let reverted = agent.revert(group);
    assert_eq!(reverted["status"], "answered", "{reverted}");
    assert_eq!(reverted["data"]["reverted"], true);
    assert_eq!(agent.read("junction")["total"], 0);
    let again = agent.revert(group);
    assert_eq!(again["status"], "refused", "{again}");
    agent.live.finish();
    assert_eq!(snapshot(&root), before, "nothing was saved");
}

#[test]
fn revert_is_refused_under_a_newer_step_and_the_agent_has_no_other_way_back() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let mut agent = Agent::start(&fx, &[]);
    let revision = agent.revision();
    let first = agent.apply(revision, edits());
    let first_group = first["data"]["group"].as_u64().unwrap();
    let revision = agent.revision();
    let second = agent.apply(
        revision,
        json!([{"id": "place_junction", "args": {"at": [20.32, 20.32]}}]),
    );
    assert_eq!(second["status"], "answered", "{second}");

    let blocked = agent.revert(first_group);
    assert_eq!(blocked["status"], "refused", "{blocked}");
    assert!(
        blocked["reason"]
            .as_str()
            .unwrap()
            .contains("a later step sits on top")
    );
    // The headless undo does not take an agent's step either.
    let undo = agent
        .live
        .send(&json!({"id": "undo", "args": {"document": "main.snxsch"}}));
    assert_eq!(undo["status"], "refused", "{undo}");
    assert!(undo["reason"].as_str().unwrap().contains("revert"));
    assert_eq!(agent.read("junction")["total"], 2);
    agent.live.finish();
}

#[test]
fn every_edit_is_authorized_as_a_command_of_its_own() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let outside = other.join("outside.snxsch");
    std::fs::copy(&fx.sheet, &outside).expect("copy");
    let before = snapshot(&root);
    let mut agent = Agent::start(&fx, &[]);
    let revision = agent.revision();

    for (edit, needle) in [
        (
            json!({"id": "save_document", "args": {"document": "main.snxsch"}}),
            "edit 1 (`save_document`): `save_document` needs the `save` capability",
        ),
        (
            json!({"id": "zq_no_such_command"}),
            "edit 1 (`zq_no_such_command`): unknown command",
        ),
        (
            json!({"id": "color_net",
                "args": {"at": [5, 0], "color": "#336699", "document": outside}}),
            "edit 1 (`color_net`): operand `document`",
        ),
    ] {
        let refused = agent.apply(revision, json!([edit.clone()]));
        assert_eq!(refused["status"], "refused", "{edit}: {refused}");
        assert!(
            refused["reason"].as_str().unwrap().contains(needle),
            "{refused}"
        );
        assert_eq!(refused["changed"], json!([]));
    }
    agent.live.finish();

    // With `--save` the policy lets a save through, and the app still says
    // an edit list is not the place for it.
    let mut saver = Agent::start(&fx, &["--save"]);
    let revision = saver.revision();
    let refused = saver.apply(
        revision,
        json!([{"id": "save_document", "args": {"document": "main.snxsch"}}]),
    );
    assert_eq!(refused["status"], "refused", "{refused}");
    assert!(
        refused["reason"]
            .as_str()
            .unwrap()
            .contains("does not edit a document")
    );
    saver.live.finish();
    // The `--save` run journals itself in `.signex/` (#2253); nothing else.
    assert_eq!(support::without_journals(snapshot(&root)), before);
}

/// #2178: the documents a cross-sheet call names in `expect_revisions` are
/// confined like a `document` operand, so the revision of a sheet outside the
/// allowed folders is never compared (and never leaked by a refusal).
#[test]
fn a_document_named_in_expect_revisions_is_confined_like_a_document_operand() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let outside = other.join("outside.snxsch");
    std::fs::copy(&fx.sheet, &outside).expect("copy");
    let before = snapshot(&root);
    let mut agent = Agent::start(&fx, &[]);
    let revision = agent.revision();
    let refused = agent.live.send(&json!({"id": "apply_edits", "args": {
        "document": "main.snxsch",
        "expect_revision": revision,
        "expect_revisions": {outside.display().to_string(): 1},
        "edits": edits()}}));
    assert_eq!(refused["status"], "refused", "{refused}");
    assert!(
        refused["reason"]
            .as_str()
            .unwrap()
            .contains("operand `expect_revisions`"),
        "{refused}"
    );
    assert_eq!(refused["changed"], json!([]));
    agent.live.finish();
    assert_eq!(support::without_journals(snapshot(&root)), before);
}

#[test]
fn a_saved_batch_reaches_the_file_only_through_save_document() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let mut agent = Agent::start(&fx, &["--save"]);
    let revision = agent.revision();
    let applied = agent.apply(revision, edits());
    assert_eq!(applied["status"], "answered", "{applied}");
    let on_disk = |path: &std::path::Path| {
        signex_types::format::SnxSchematic::parse(&std::fs::read_to_string(path).unwrap())
            .expect("the sheet reads back")
            .sheet
    };
    assert_eq!(on_disk(&fx.sheet).junctions.len(), 0, "applied, not saved");
    let saved = agent
        .live
        .send(&json!({"id": "save_document", "args": {"document": "main.snxsch"}}));
    assert_eq!(saved["status"], "ran", "{saved}");
    assert_eq!(on_disk(&fx.sheet).junctions.len(), 1);
    agent.live.finish();
}

#[test]
fn the_catalog_lists_both_commands_with_the_edit_capability() {
    let out = run_cli(&["commands", "--json"], "");
    assert_eq!(out.code, Some(0));
    let commands = out.lines[0]["commands"].as_array().unwrap();
    for (id, operands) in [
        (
            "apply_edits",
            [
                "document",
                "expect_revision",
                "edits",
                "summary",
                "expect_revisions",
            ]
            .as_slice(),
        ),
        ("revert", ["document", "group", "documents"].as_slice()),
    ] {
        let entry = commands
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("{id} is not listed"));
        assert_eq!(entry["capability"], "edit");
        assert_eq!(entry["headless"], "runnable");
        let names: Vec<&str> = entry["operands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, operands, "{id}");
    }
}
