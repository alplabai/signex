//! Attachments (mockup row 199): the `#attachment` call, the stale marks
//! against the design, Mark still valid, and the copy into
//! `notebook/attachments/`.

use std::path::PathBuf;

use crate::attachment::{
    AttachmentKind, AttachmentProblemKind, DesignValues, ValueChange, attachment_call, attachments,
    changes, folder_for, inside_attachments, mark_still_valid, plan_copy, reading_changes,
};
use crate::edit::{StaleTarget, append_call, locate};
use crate::measurement::readings;
use crate::report::attachments_report;
use crate::{NotebookSources, Reference};

const THERMAL: &str = "+++\nformat = \"snxnbk/1\"\n+++\n= Thermal\n\n#attachment(\"thermal/u3-2A-20min.png\",\n  caption: [Thermal image at 2 A],\n  target: (component(\"U3\"), component(\"L1\")),\n  taken-with: (\"L1\": \"3.3 µH\", U3: \"TPS62130\"),\n  taken: \"2026-10-09\",\n  rev: \"A rework\",\n)\n";

fn values() -> DesignValues {
    [("L1", "4.7 µH"), ("U3", "TPS62130"), ("C15", "22 µF")]
        .into_iter()
        .collect()
}

fn pair(part: &str, value: &str) -> (String, String) {
    (part.to_string(), value.to_string())
}

#[test]
fn an_attachment_is_read_with_its_file_targets_and_stored_values() {
    let (found, problems) = attachments(THERMAL);
    assert!(problems.is_empty(), "{problems:?}");
    let a = &found[0];
    assert_eq!(a.file, "thermal/u3-2A-20min.png");
    assert_eq!(a.path_in_notebook(), "attachments/thermal/u3-2A-20min.png");
    assert_eq!(a.kind, AttachmentKind::Image);
    assert_eq!(a.caption, "Thermal image at 2 A");
    let parts = ["U3", "L1"].map(|p| Reference::Component(p.into()));
    assert_eq!(a.targets, parts);
    let stored = [pair("L1", "3.3 µH"), pair("U3", "TPS62130")];
    assert_eq!(a.taken_with, stored);
    let taken = a.taken.map(|t| t.to_string());
    assert_eq!(taken.as_deref(), Some("2026-10-09"));
    assert_eq!(a.rev, "A rework");
    assert_eq!(a.line, 6, "lines of the file, header included");
}

#[test]
fn an_attachment_without_a_file_or_outside_the_folder_is_a_problem() {
    let text = "#attachment(caption: [x])\n#attachment(\"../design.snxnbk\")\n#attachment(file: \"sim/a.snxsim\", taken: \"soon\")\n";
    let (found, problems) = attachments(text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, AttachmentKind::Simulation);
    let kinds: Vec<_> = problems.iter().map(|p| (p.line, p.kind.clone())).collect();
    let outside = AttachmentProblemKind::OutsideAttachments {
        written: "../design.snxnbk".into(),
    };
    let bad_time = AttachmentProblemKind::BadTimestamp {
        written: "\"soon\"".into(),
    };
    assert_eq!(
        kinds,
        [
            (1, AttachmentProblemKind::MissingFile),
            (2, outside),
            (3, bad_time)
        ]
    );
}

#[test]
fn a_path_stays_inside_the_attachments_folder() {
    let inside = |path: &str| inside_attachments(path);
    assert_eq!(inside("scope/a.csv").as_deref(), Some("scope/a.csv"));
    assert_eq!(inside("./scope//a.csv").as_deref(), Some("scope/a.csv"));
    assert_eq!(inside("/scope/a.csv").as_deref(), Some("scope/a.csv"));
    assert_eq!(inside("scope/../b.png").as_deref(), Some("b.png"));
    assert_eq!(inside("../b.png"), None);
    assert_eq!(inside(""), None);
}

#[test]
fn the_kind_and_folder_follow_the_extension() {
    assert_eq!(AttachmentKind::of("a.CSV"), AttachmentKind::ScopeCsv);
    assert_eq!(AttachmentKind::of("a.jpeg"), AttachmentKind::Image);
    assert_eq!(AttachmentKind::of("a.snxsim"), AttachmentKind::Simulation);
    assert_eq!(AttachmentKind::of("notes"), AttachmentKind::Other);
    let folders: Vec<_> = [
        AttachmentKind::ScopeCsv,
        AttachmentKind::Image,
        AttachmentKind::Simulation,
        AttachmentKind::Other,
    ]
    .map(folder_for)
    .to_vec();
    assert_eq!(folders, ["scope", "images", "sim", "files"]);
}

#[test]
fn a_changed_or_gone_part_makes_it_stale() {
    let stored = vec![pair("L1", "3.3 µH"), pair("C15", "22uF"), pair("R9", "10k")];
    assert_eq!(
        changes(&stored, &values()),
        [
            ValueChange {
                part: "L1".into(),
                was: "3.3 µH".into(),
                now: Some("4.7 µH".into()),
            },
            ValueChange {
                part: "R9".into(),
                was: "10k".into(),
                now: None,
            },
        ],
        "22uF is 22 µF"
    );
    let unread = DesignValues::new();
    assert!(
        changes(&stored, &unread).is_empty(),
        "the design is not read"
    );
}

#[test]
fn a_reading_is_stale_through_its_attachment() {
    let text = "#attachment(\"scope/r.csv\", taken-with: (L1: \"3.3 µH\"))\n\
                #measurement(id: \"MEAS-005\", value: 27, limit: (max: 30), attach: \"scope/r.csv\", taken-with: (\"L1\": \"3.3uH\"))\n";
    let (found, _) = attachments(text);
    let (taken, _) = readings(text);
    let stale = reading_changes(&taken[0], &found, &values());
    assert_eq!(stale.len(), 1, "L1 once: {stale:?}");
    assert_eq!(stale[0].part, "L1");
}

#[test]
fn mark_still_valid_stores_the_current_values_and_keeps_the_rest() {
    let marked = mark_still_valid(THERMAL, 6, &values()).expect("marked");
    let stored = "taken-with: (\"L1\": \"4.7 µH\", \"U3\": \"TPS62130\"),";
    assert!(marked.contains(stored), "{marked}");
    let before = THERMAL.replace("U3: ", "\"U3\": ");
    assert_eq!(marked.replace("4.7 µH", "3.3 µH"), before);
    let (found, _) = attachments(&marked);
    let left = changes(&found[0].taken_with, &values());
    assert!(left.is_empty(), "no longer stale");
    let none_there = mark_still_valid(THERMAL, 5, &values());
    assert_eq!(none_there, None, "no call on that line");
    assert_eq!(mark_still_valid(THERMAL, 6, &DesignValues::new()), None);
    let bare = mark_still_valid("#attachment(\"a.png\")\n", 1, &values());
    assert_eq!(bare, None, "stores nothing");
}

#[test]
fn mark_still_valid_works_on_a_reading_too() {
    let text =
        "#measurement(id: \"MEAS-005\", value: 27, taken-with: (L1: \"3.3 µH\", R9: \"1k\"))\n";
    let marked = mark_still_valid(text, 1, &values()).expect("marked");
    let want = "#measurement(id: \"MEAS-005\", value: 27, taken-with: (\"L1\": \"4.7 µH\"))\n";
    assert_eq!(marked, want, "a part gone from the design is dropped");
}

#[test]
fn a_dropped_file_is_copied_under_a_free_plain_name() {
    let from = "/home/a/Scope Shots/ripple 2A.csv";
    let copy = plan_copy(from, "scope", |_| false).expect("a plan");
    assert_eq!(copy.to, "attachments/scope/ripple-2A.csv");
    assert_eq!(copy.file(), "scope/ripple-2A.csv");
    assert_eq!(copy.from, PathBuf::from(from));
    let taken =
        |path: &str| path == "attachments/scope/r.csv" || path == "attachments/scope/r-2.csv";
    let next = plan_copy("/x/r.csv", "scope", taken).expect("a plan");
    assert_eq!(next.to, "attachments/scope/r-3.csv");
    let hidden = plan_copy("/x/.hidden", "images", |_| false).expect("a plan");
    assert_eq!(hidden.to, "attachments/images/hidden");
    assert!(plan_copy("/x/a.png", "../up", |_| false).is_none());
    assert!(plan_copy("/x/a.png", "", |_| false).is_none());
    assert!(plan_copy("/", "images", |_| false).is_none());
}

#[test]
fn the_inserted_call_stores_the_values_of_the_parts_it_names() {
    let targets = [
        Reference::Net("+3V3".into()),
        Reference::Component("C15".into()),
        Reference::Pin {
            component: "U3".into(),
            pin: "3".into(),
        },
    ];
    assert_eq!(
        attachment_call("scope/r.csv", "Ripple at 2 A", &targets, &values()),
        "#attachment(\"scope/r.csv\", caption: \"Ripple at 2 A\", target: (net(\"+3V3\"), component(\"C15\"), pin(\"U3\", \"3\")), taken-with: (\"C15\": \"22 µF\", \"U3\": \"TPS62130\"))"
    );
    let vin = [Reference::Net("VIN".into())];
    assert_eq!(
        attachment_call("a.png", " ", &vin, &values()),
        "#attachment(\"a.png\", target: net(\"VIN\"))"
    );
}

#[test]
fn mark_still_valid_finds_its_call_by_file_or_by_reading() {
    let mut sources = NotebookSources::new("design.snxnbk", THERMAL);
    sources.insert(
        "chapters/b.snxnbk",
        "\n#measurement(id: \"MEAS-005\", value: 27, taken-with: (L1: \"3.3 µH\"))\n#measurement(id: \"MEAS-006\", value: 1)\n",
    );
    let found = crate::record::record(&sources);
    let by_file = StaleTarget::Attachment("thermal/u3-2A-20min.png".into());
    let design = Some(("design.snxnbk".to_string(), 6));
    assert_eq!(locate(&found, &by_file), design);
    let by_id = StaleTarget::Reading(crate::BlockId::parse("MEAS-005").unwrap());
    let chapter = Some(("chapters/b.snxnbk".to_string(), 2));
    assert_eq!(locate(&found, &by_id), chapter);
    let stores_nothing = StaleTarget::Reading(crate::BlockId::parse("MEAS-006").unwrap());
    assert_eq!(locate(&found, &stores_nothing), None);
    let elsewhere = StaleTarget::Attachment("x.png".into());
    assert_eq!(locate(&found, &elsewhere), None);
}

#[test]
fn a_new_attachment_is_its_own_paragraph_at_the_end() {
    let appended = append_call("= A\nText.", "#attachment(\"a.png\")");
    assert_eq!(appended, "= A\nText.\n\n#attachment(\"a.png\")\n");
    assert_eq!(append_call("Text.\n\n", "#x()"), "Text.\n\n#x()\n");
    assert_eq!(append_call("", "#x()"), "#x()\n");
}

#[test]
fn the_sources_say_which_attachments_to_read_before_a_compile() {
    let mut sources = NotebookSources::new("design.snxnbk", THERMAL);
    let chapter = "#attachment(\"scope/r.csv\")\n#attachment(\"scope/r.csv\")\n";
    sources.insert("chapters/b.snxnbk", chapter);
    let thermal = "attachments/thermal/u3-2A-20min.png";
    let missing = sources.missing_attachments();
    assert_eq!(missing, ["attachments/scope/r.csv", thermal]);
    sources.insert("attachments/scope/r.csv", "t,v\n0,1\n1,2\n");
    assert_eq!(sources.missing_attachments(), [thermal]);
}

#[test]
fn the_attachments_answer_marks_stale_missing_and_drawn() {
    let text = format!("{THERMAL}#attachment(\"scope/r.csv\")\n#attachment(\"scope/bad.csv\")\n");
    let mut sources = NotebookSources::new("design.snxnbk", text);
    sources.insert("attachments/scope/r.csv", "t,v\n0,1\n1,2\n");
    sources.insert("attachments/scope/bad.csv", "t,v\n0,1\n1,x\n");
    sources.set_design_values(values());
    let report = attachments_report(&sources);
    let s = report.summary;
    let counts = (s.attachments, s.stale_attachments, s.missing_attachments);
    assert_eq!(counts, (3, 1, 1));
    assert!(report.attachments[1].plot.is_some(), "the CSV is drawn");
    let error = report.attachments[2].error.as_deref();
    assert_eq!(error, Some("line 3, column 2 is not a number"));
    let json = serde_json::to_value(&report).expect("json");
    assert_eq!(json["attachments"][0]["file"], "thermal/u3-2A-20min.png");
    assert_eq!(json["attachments"][0]["changes"][0]["now"], "4.7 µH");
    assert_eq!(json["attachments"][0]["kind"], "image");
}
