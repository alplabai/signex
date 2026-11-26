//! `design_checklist` in the `.snxprj` (#1291, mockup row 206): absent by
//! default and never written then, kept byte for byte while untouched, keys
//! a newer build wrote kept through every edit, an unreadable value kept and
//! never edited over, and the edits' refusals.

use serde_json::{Value, json};

use super::*;
use crate::check::{CheckReport, Finding, Severity};

/// An edit that names neither a day nor an editor.
const NO_STAMP: EditStamp = EditStamp {
    day: None,
    by: None,
};

fn project_file(json: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(&path, json).unwrap();
    (path, dir)
}

fn file_with(checklist: Value) -> String {
    json!({
        "name": "P", "dir": "", "schematic_root": null, "pcb_file": null,
        "design_checklist": checklist,
    })
    .to_string()
}

fn empty_project() -> ProjectData {
    let (path, _dir) =
        project_file(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#);
    parse_project(&path).unwrap()
}

fn saved_value(data: &ProjectData) -> Value {
    serde_json::from_slice(&encode_project(data).unwrap()).unwrap()
}

#[test]
fn a_project_without_a_checklist_saves_no_key() {
    let data = empty_project();
    assert_eq!(data.design_checklist(), Ok(None));
    assert!(!data.has_unreadable_checklist());
    let saved = String::from_utf8(encode_project(&data).unwrap()).unwrap();
    assert!(!saved.contains(CHECKLIST_KEY), "{saved}");
    assert!(
        saved.contains("\"snxprj/1\""),
        "no new format token: {saved}"
    );
}

#[test]
fn an_untouched_checklist_is_written_back_as_it_was_read() {
    let stored = json!({
        "template": "release_review",
        "items": [{"id": "1", "stage": "schematic", "title": "ERC clean", "state": "open",
                   "link": {"kind": "erc"}}],
    });
    let (path, _dir) = project_file(&file_with(stored.clone()));
    let data = parse_project(&path).unwrap();
    assert_eq!(saved_value(&data)[CHECKLIST_KEY], stored);
    let checklist = data.design_checklist().unwrap().expect("a checklist");
    assert_eq!(checklist.items[0].link, Some(ChecklistLink::Erc));
}

#[test]
fn keys_a_newer_build_wrote_survive_an_edit() {
    let (path, _dir) = project_file(&file_with(json!({
        "items": [{"id": "1", "stage": "layout", "title": "DRC", "signed_by": "jb"}],
        "review_round": 3,
    })));
    let mut data = parse_project(&path).unwrap();
    data.set_checklist_state("1", ItemState::Done, None, &EditStamp::on("2026-11-26"))
        .unwrap();
    let saved = saved_value(&data);
    assert_eq!(saved[CHECKLIST_KEY]["review_round"], json!(3));
    let item = &saved[CHECKLIST_KEY]["items"][0];
    assert_eq!(item["signed_by"], json!("jb"));
    assert_eq!(item["state"], json!("done"));
    assert_eq!(item["updated"], json!("2026-11-26"));
}

#[test]
fn an_unreadable_checklist_is_kept_and_never_edited_over() {
    let stored = json!({"items": [{"id": "1", "stage": "schematic", "title": "T",
                                   "state": "blocked"}]});
    let (path, _dir) = project_file(&file_with(stored.clone()));
    let mut data = parse_project(&path).expect("the project still opens");
    assert!(data.has_unreadable_checklist());
    assert_eq!(
        data.add_checklist_item("schematic", "More", &NO_STAMP),
        Err(ChecklistProblem::Unreadable)
    );
    assert_eq!(
        data.start_design_checklist(Some("blank"), Vec::new(), &NO_STAMP),
        Err(ChecklistProblem::Unreadable)
    );
    assert_eq!(saved_value(&data)[CHECKLIST_KEY], stored);
}

#[test]
fn a_start_numbers_the_items_and_a_second_start_is_refused() {
    let mut data = empty_project();
    let items = ChecklistTemplate::PrototypeBuild
        .items()
        .iter()
        .map(|item| NewChecklistItem {
            stage: item.stage,
            title: item.key.to_string(),
            link: item.link.map(TemplateLink::to_link),
        })
        .collect();
    data.start_design_checklist(Some("prototype_build"), items, &NO_STAMP)
        .unwrap();
    let checklist = data.design_checklist().unwrap().unwrap();
    let ids: Vec<&str> = checklist.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["1", "2", "3", "4", "5", "6"]);
    assert_eq!(checklist.template.as_deref(), Some("prototype_build"));
    assert_eq!(checklist.counts(None).open, 6);
    assert_eq!(
        data.start_design_checklist(Some("blank"), Vec::new(), &NO_STAMP),
        Err(ChecklistProblem::AlreadyStarted)
    );
}

#[test]
fn an_added_item_takes_the_next_number_and_needs_a_title() {
    let mut data = empty_project();
    assert_eq!(
        data.add_checklist_item("layout", "  Check vias ", &NO_STAMP),
        Ok("1".into())
    );
    assert_eq!(
        data.add_checklist_item("layout", "Panel", &NO_STAMP),
        Ok("2".into())
    );
    data.remove_checklist_item("1").unwrap();
    assert_eq!(
        data.add_checklist_item("fabrication", "BOM", &NO_STAMP),
        Ok("3".into())
    );
    assert_eq!(
        data.add_checklist_item("layout", "  ", &NO_STAMP),
        Err(ChecklistProblem::EmptyTitle)
    );
    assert_eq!(
        data.add_checklist_item("review", "X", &NO_STAMP),
        Err(ChecklistProblem::UnknownStage("review".into()))
    );
    let checklist = data.design_checklist().unwrap().unwrap();
    assert_eq!(checklist.item("2").unwrap().title, "Panel");
    assert!(checklist.item("1").is_none());
}

#[test]
fn waived_needs_a_reason_and_done_drops_it() {
    let mut data = empty_project();
    data.add_checklist_item("schematic", "Decoupling", &NO_STAMP)
        .unwrap();
    assert_eq!(
        data.set_checklist_state("1", ItemState::Waived, Some(" "), &NO_STAMP),
        Err(ChecklistProblem::NeedsReason)
    );
    data.set_checklist_state("1", ItemState::Waived, Some("Not fitted"), &NO_STAMP)
        .unwrap();
    let item = data.design_checklist().unwrap().unwrap().items[0].clone();
    assert_eq!(item.state, ItemState::Waived);
    assert_eq!(item.reason.as_deref(), Some("Not fitted"));
    data.set_checklist_state("1", ItemState::Done, Some("ignored"), &NO_STAMP)
        .unwrap();
    let item = data.design_checklist().unwrap().unwrap().items[0].clone();
    assert_eq!(item.reason, None);
    assert_eq!(
        data.set_checklist_state("9", ItemState::Done, None, &NO_STAMP),
        Err(ChecklistProblem::UnknownItem("9".into()))
    );
}

#[test]
fn the_day_is_stamped_only_when_the_item_changed() {
    let mut data = empty_project();
    data.add_checklist_item("schematic", "T", &EditStamp::on("2026-11-20"))
        .unwrap();
    data.set_checklist_field(
        "1",
        ChecklistField::Title,
        "T",
        &EditStamp::on("2026-11-26"),
    )
    .unwrap();
    let item = data.design_checklist().unwrap().unwrap().items[0].clone();
    assert_eq!(item.updated.as_deref(), Some("2026-11-20"));
    data.set_checklist_field(
        "1",
        ChecklistField::Owner,
        "jb",
        &EditStamp::on("2026-11-26"),
    )
    .unwrap();
    let item = data.design_checklist().unwrap().unwrap().items[0].clone();
    assert_eq!(item.updated.as_deref(), Some("2026-11-26"));
    assert_eq!(item.owner.as_deref(), Some("jb"));
}

#[test]
fn a_link_reads_back_from_its_text_and_a_bad_one_is_refused() {
    for text in [
        "erc",
        "drc",
        "title_block",
        "requirement:REQ-002",
        "object:U3 pin 2",
    ] {
        let link = ChecklistLink::parse(text).unwrap().unwrap();
        assert_eq!(link.to_text(), text);
    }
    assert_eq!(ChecklistLink::parse(" "), Ok(None));
    assert!(ChecklistLink::parse("requirement:").is_err());
    let mut data = empty_project();
    data.add_checklist_item("schematic", "T", &NO_STAMP)
        .unwrap();
    assert_eq!(
        data.set_checklist_field("1", ChecklistField::Link, "bom", &NO_STAMP),
        Err(ChecklistProblem::UnknownLink("bom".into()))
    );
}

fn finding(rule: &str) -> Finding {
    Finding {
        rule: rule.into(),
        severity: Severity::Error,
        message: String::new(),
        evidence: json!({}),
        items: Vec::new(),
        sheet: String::new(),
        key: rule.into(),
    }
}

#[test]
fn a_link_result_follows_the_run_and_the_title_block() {
    let mut data = empty_project();
    let title = ChecklistLink::TitleBlock;
    assert_eq!(title.result(&data, None), LinkResult::Fails(4));
    for key in TITLE_BLOCK_REQUIRED {
        data.parameters.insert(key.into(), "x".into());
    }
    assert_eq!(title.result(&data, None), LinkResult::Passes);

    let erc = ChecklistLink::Erc;
    let req = ChecklistLink::Requirement { id: "REQ-1".into() };
    assert_eq!(erc.result(&data, None), LinkResult::NotRun);
    let report = CheckReport::new(
        "requirements",
        vec![finding("req::checklist::erc")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .with_passed(vec!["req::REQ-1".into()]);
    assert_eq!(erc.result(&data, Some(&report)), LinkResult::Fails(1));
    assert_eq!(req.result(&data, Some(&report)), LinkResult::Passes);
    let other = ChecklistLink::Requirement { id: "REQ-2".into() };
    assert_eq!(other.result(&data, Some(&report)), LinkResult::NotRun);
    let object = ChecklistLink::Object {
        target: "U3".into(),
    };
    assert_eq!(object.result(&data, Some(&report)), LinkResult::NoCheck);
}

#[test]
fn every_template_names_unique_keys() {
    for template in CHECKLIST_TEMPLATES {
        let mut keys: Vec<&str> = template.items().iter().map(|i| i.key).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "{}", template.key());
        assert_eq!(ChecklistTemplate::parse(template.key()), Some(template));
    }
    assert_eq!(ChecklistTemplate::Blank.items().len(), 0);
}
