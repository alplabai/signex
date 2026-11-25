//! Annotation blocks as review records (mockup row 198) and the "Power
//! stage" page of mockup row 60b: one function per kind with its typed id,
//! the record fields of each kind, the measurement verdict, the links
//! between blocks, a failing measurement opening an Issue, and the source
//! an insert button writes.

use std::path::Path;

use crate::{
    Alternative, Annotation, AnnotationKind, BlockId, Citation, CompileOptions, Filter, IsoDate,
    Limits, NotebookSources, PACKAGE_IMPORT, ProblemKind, Record, Reference, Severity, Status,
    Verdict, cited_by, compile, issue_for_failure, links, new_block_source, next_block_id,
    open_failing_measurements, scan, unopened_failures, verdict_of,
};

/// The blocks of mockup row 198's source pane, as drawn (the Issue's body
/// shortened).
const ROW_198: &str = r#"= Decisions and open issues <sec:dec>

#decision(
  id: "DEC-002", title: [Inductor for the power stage],
  status: "accepted", owner: "j.berg",
  date: "2026-11-18", review: "R-03",
  target: (component("L1"), pin("U3", "3")),
  requirement: "REQ-002",
  chosen: [Candidate A, 4.7 µH, 6.2 A, 24 mΩ],
  alternatives: (
    ([B, 3.3 µH], [ripple 1.45 A, no margin]),
    ([C, 6.8 µH], [1.6 A saturation margin]),
  ),
  supersedes: "DEC-001",
)

#issue(
  id: "ISS-004", title: [Worst-case output above +2 %],
  status: "open", owner: "j.berg", due: "rev C",
  severity: "minor", from: "CALC-2.1",
  target: (component("R7"), component("R8")),
)[
  With 1 % parts and the ±1 % reference the output spans 3.249 to 3.417 V.
]

#note(id: "NOTE-012", title: [Switch-node copper],
  target: net("SW"))[Keep the copper of #net("SW") away from #pin("U3", "3").]

#measurement(
  id: "MEAS-010", target: net("VIN"),
  value: 310, unit: "mV", limit: (max: 250),
  conditions: "VIN 12 V, 2 A, 25 °C",
)
"#;

fn blocks(source: &str) -> Vec<Annotation> {
    let (found, problems) = scan(source);
    assert!(problems.is_empty(), "{problems:?}");
    found
}

fn id(text: &str) -> BlockId {
    BlockId::parse(text).expect("a block id")
}

fn net(name: &str) -> Reference {
    Reference::Net(name.into())
}

fn component(name: &str) -> Reference {
    Reference::Component(name.into())
}

fn pin(part: &str, number: &str) -> Reference {
    Reference::Pin {
        component: part.into(),
        pin: number.into(),
    }
}

fn date(text: &str) -> IsoDate {
    IsoDate::parse(text).expect("a date")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn each_kind_function_is_read_as_a_block_with_its_typed_id() {
    let found = blocks(ROW_198);
    let read: Vec<(AnnotationKind, Option<String>)> = found
        .iter()
        .map(|a| (a.kind, a.id.as_ref().map(BlockId::to_string)))
        .collect();
    assert_eq!(
        read,
        [
            (AnnotationKind::Decision, Some("DEC-002".into())),
            (AnnotationKind::Issue, Some("ISS-004".into())),
            (AnnotationKind::Note, Some("NOTE-012".into())),
            (AnnotationKind::Measurement, Some("MEAS-010".into())),
        ]
    );
    let decision = &found[0];
    assert_eq!(
        decision.title, "Inductor for the power stage",
        "a content title"
    );
    assert_eq!(decision.status, Some(Status::Accepted));
    assert_eq!(decision.date, Some(date("2026-11-18")));
    assert_eq!(decision.line, 3);
    assert!(decision.source.starts_with("#decision("));
    assert_eq!(&ROW_198[decision.span.clone()], decision.source);
}

#[test]
fn a_decision_records_who_chose_what_over_what() {
    let found = blocks(ROW_198);
    let Record::Decision(record) = &found[0].record else {
        panic!("a decision record: {:?}", found[0].record);
    };
    assert_eq!(record.owner.as_deref(), Some("j.berg"));
    assert_eq!(record.review.as_deref(), Some("R-03"));
    assert_eq!(record.requirements, ["REQ-002"]);
    assert_eq!(
        record.chosen.as_deref(),
        Some("Candidate A, 4.7 µH, 6.2 A, 24 mΩ")
    );
    assert_eq!(
        record.alternatives,
        [
            Alternative {
                option: "B, 3.3 µH".into(),
                reason: Some("ripple 1.45 A, no margin".into()),
            },
            Alternative {
                option: "C, 6.8 µH".into(),
                reason: Some("1.6 A saturation margin".into()),
            },
        ]
    );
    assert_eq!(record.supersedes, Some(id("DEC-001")));
    assert_eq!(found[0].references, [component("L1"), pin("U3", "3")]);
    assert_eq!(
        found[0].snippet, "Candidate A, 4.7 µH, 6.2 A, 24 mΩ",
        "a block with no body reads as what it chose"
    );
}

#[test]
fn an_issue_records_owner_severity_due_and_where_it_came_from() {
    let found = blocks(ROW_198);
    let Record::Issue(record) = &found[1].record else {
        panic!("an issue record");
    };
    assert_eq!(record.owner.as_deref(), Some("j.berg"));
    assert_eq!(record.severity, Some(Severity::Minor));
    assert_eq!(record.due.as_deref(), Some("rev C"));
    assert_eq!(record.from, Some(id("CALC-2.1")));
    assert_eq!(found[1].status, Some(Status::Open));
    assert_eq!(found[1].references, [component("R7"), component("R8")]);
    assert!(found[1].snippet.starts_with("With 1 % parts"));
}

#[test]
fn a_note_lists_its_target_first_and_each_reference_once() {
    let found = blocks(ROW_198);
    let note = &found[2];
    assert_eq!(note.record, Record::Note(Default::default()));
    assert_eq!(note.status, None);
    assert_eq!(note.references, [net("SW"), pin("U3", "3")]);
    assert_eq!(note.snippet, "Keep the copper of SW away from U3.3.");
}

#[test]
fn a_single_value_measurement_is_one_row_judged_against_its_limit() {
    let found = blocks(ROW_198);
    let Record::Measurement(record) = &found[3].record else {
        panic!("a measurement record");
    };
    assert_eq!(record.rows.len(), 1);
    let row = &record.rows[0];
    assert_eq!(row.target, Some(net("VIN")));
    assert!(close(row.measured, 310.0));
    assert_eq!(
        row.limits,
        Some(Limits {
            min: None,
            max: Some(250.0)
        })
    );
    assert!(close(row.error_percent.expect("an error"), 24.0), "{row:?}");
    assert_eq!(row.verdict, Some(Verdict::Fail));
    assert_eq!(record.verdict, Some(Verdict::Fail));
    assert_eq!(record.unit.as_deref(), Some("mV"));
    assert_eq!(record.conditions.as_deref(), Some("VIN 12 V, 2 A, 25 °C"));
    assert_eq!(found[3].record.verdict(), Some(Verdict::Fail));
}

#[test]
fn the_verdict_passes_warns_under_one_percent_from_a_limit_and_fails_outside() {
    let band = Limits::around(5.0, 5.0, true).expect("a band");
    assert!(close(band.min.expect("min"), 4.75) && close(band.max.expect("max"), 5.25));
    assert_eq!(verdict_of(5.0, band), Verdict::Pass);
    assert_eq!(
        verdict_of(4.78, band),
        Verdict::Warn,
        "mockup row 60b: VBUS 4.78 V"
    );
    assert_eq!(verdict_of(4.75, band), Verdict::Warn, "on the limit");
    assert_eq!(verdict_of(4.74, band), Verdict::Fail);
    assert_eq!(verdict_of(5.26, band), Verdict::Fail);
    let open = Limits {
        min: None,
        max: Some(0.0),
    };
    assert_eq!(
        verdict_of(0.0, open),
        Verdict::Pass,
        "no warning band at zero"
    );
    assert_eq!(verdict_of(0.1, open), Verdict::Fail);
    let absolute = Limits::around(3.3, 0.1, false).expect("a band");
    assert!(close(absolute.min.expect("min"), 3.2));
    assert_eq!(
        Limits::around(1.0, -1.0, false),
        None,
        "a negative tolerance"
    );
    assert!(Verdict::Pass < Verdict::Warn && Verdict::Warn < Verdict::Fail);
}

#[test]
fn the_power_stage_page_reads_as_drawn() {
    let found = blocks(&power_stage());
    let ids: Vec<String> = found
        .iter()
        .filter_map(|a| a.id.as_ref().map(BlockId::to_string))
        .collect();
    assert_eq!(
        ids,
        ["CALC-3.1", "MEAS-3.2", "ISS-3.1", "DEC-3.1", "NOTE-3.1"]
    );
    let Record::Calculation(calculation) = &found[0].record else {
        panic!("a calculation");
    };
    assert_eq!(calculation.nominal, Some(3.3));
    assert_eq!(
        calculation.verdict,
        Some(Verdict::Pass),
        "3.332 V in 3.30 V ±2 %"
    );
    assert_eq!(
        found[0].references,
        [component("R3"), component("R4"), component("U1")]
    );
    let Record::Measurement(rails) = &found[1].record else {
        panic!("a measurement");
    };
    let verdicts: Vec<Option<Verdict>> = rails.rows.iter().map(|r| r.verdict).collect();
    assert_eq!(
        verdicts,
        [
            Some(Verdict::Pass),
            Some(Verdict::Pass),
            Some(Verdict::Warn)
        ]
    );
    assert_eq!(
        rails.counts(),
        [(Verdict::Pass, 2), (Verdict::Warn, 1), (Verdict::Fail, 0)],
        "2 pass · 1 warn"
    );
    assert!(close(
        rails.rows[0].error_percent.expect("an error"),
        (11.96 - 12.0) / 12.0 * 100.0
    ));
    assert_eq!(rails.instrument.as_deref(), Some("DMM 6½ digit, LAB-0117"));
    assert_eq!(rails.operator.as_deref(), Some("j.berg"));
    assert_eq!(
        found[1].references,
        [net("VIN_12V"), net("VOUT_3V3"), net("VBUS")]
    );
    assert_eq!(
        cited_by(&id("MEAS-3.2"), &found),
        [(Citation::From, id("ISS-3.1"))],
        "MEAS-3.2 opens ISS-3.1"
    );
    assert_eq!(found[4].references, [component("U7")]);
}

#[test]
fn the_power_stage_page_prints() {
    let sources = NotebookSources::new("power-stage.snxnbk", power_stage());
    let compilation = compile(&sources, &CompileOptions::default());
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    assert!(compilation.notebook.expect("pages").page_count() >= 1);
}

#[test]
fn the_printed_verdict_follows_the_same_rule() {
    let cases = [
        (5.0, Limits::around(5.0, 5.0, true)),
        (4.78, Limits::around(5.0, 5.0, true)),
        (4.74, Limits::around(5.0, 5.0, true)),
        (
            310.0,
            Some(Limits {
                min: None,
                max: Some(250.0),
            }),
        ),
        (
            249.0,
            Some(Limits {
                min: None,
                max: Some(250.0),
            }),
        ),
        (
            0.0,
            Some(Limits {
                min: Some(0.0),
                max: None,
            }),
        ),
    ];
    let mut text = format!("{PACKAGE_IMPORT}\n");
    for (value, limits) in cases {
        let limits = limits.expect("a band");
        let bound = |name: &str, limit: Option<f64>| {
            limit.map_or(String::new(), |l| format!("{name}: {l:?}, "))
        };
        text.push_str(&format!(
            "#assert.eq(verdict({value:?}, ({}{})), \"{}\")\n",
            bound("min", limits.min),
            bound("max", limits.max),
            verdict_of(value, limits).token()
        ));
    }
    let compilation = compile(
        &NotebookSources::new("design.snxnbk", text),
        &CompileOptions::default(),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
}

#[test]
fn links_are_read_both_ways_and_say_whether_their_target_exists() {
    let found = blocks(ROW_198);
    let all = links(&found);
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!(
        (all[0].by.to_string(), all[0].to.to_string()),
        ("DEC-002".into(), "DEC-001".into())
    );
    assert_eq!(all[0].citation, Citation::Supersedes);
    assert!(!all[0].resolved, "DEC-001 is not in the list");
    assert_eq!(all[1].citation, Citation::From);
    assert_eq!(
        cited_by(&id("DEC-001"), &found),
        [(Citation::Supersedes, id("DEC-002"))]
    );
    assert!(cited_by(&id("NOTE-012"), &found).is_empty());
}

#[test]
fn a_failing_measurement_opens_an_issue_after_it() {
    let found = blocks(ROW_198);
    let failures = unopened_failures(&found);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].id, Some(id("MEAS-010")));
    let today = date("2026-11-25");
    let opened = open_failing_measurements(ROW_198, today, |m| {
        format!(
            "{} out of limit",
            m.id.as_ref().map(BlockId::to_string).unwrap_or_default()
        )
    })
    .expect("an issue to open");
    let after = blocks(&opened);
    let issue = after.last().expect("the new issue");
    assert_eq!(issue.kind, AnnotationKind::Issue);
    assert_eq!(issue.id, Some(id("ISS-005")), "one past ISS-004");
    assert_eq!(issue.title, "MEAS-010 out of limit");
    assert_eq!(issue.status, Some(Status::Open));
    assert_eq!(issue.date, Some(today));
    assert_eq!(issue.references, [net("VIN")]);
    let Record::Issue(record) = &issue.record else {
        panic!("an issue");
    };
    assert_eq!(record.from, Some(id("MEAS-010")));
    assert!(
        opened.starts_with(ROW_198.trim_end()),
        "nothing before it changes"
    );
    assert_eq!(
        open_failing_measurements(&opened, today, |_| String::new()),
        None,
        "an opened failure opens nothing more"
    );
    assert!(unopened_failures(&after).is_empty());
}

#[test]
fn a_sectioned_measurement_opens_an_issue_in_its_section() {
    let source = "#measurement(id: \"MEAS-3.2\", rows: ((net(\"VBUS\"), 5.0, 5%, 4.5),))\n\
                  #issue(id: \"ISS-3.1\", from: \"CALC-3.1\")[Other.]\n";
    let found = blocks(source);
    let (issue, text) = issue_for_failure(
        &found[0],
        found.iter().filter_map(|a| a.id.as_ref()),
        date("2026-11-25"),
        "VBUS low",
    )
    .expect("an issue");
    assert_eq!(issue, id("ISS-3.2"));
    assert!(text.contains("from: \"MEAS-3.2\""), "{text}");
    assert!(text.contains("target: (net(\"VBUS\"),)"), "{text}");
    let unnumbered = blocks("#measurement(value: 3, limit: (max: 1))\n");
    assert!(issue_for_failure(&unnumbered[0], [], date("2026-11-25"), "x").is_none());
    assert!(
        unopened_failures(&unnumbered).is_empty(),
        "no id, nothing to cite"
    );
}

#[test]
fn a_new_block_of_each_kind_reads_back_with_its_id_and_no_problem() {
    let today = date("2026-11-25");
    for kind in AnnotationKind::ALL {
        let block_id = next_block_id(kind, &[], &blocks(ROW_198));
        let source = new_block_source(&block_id, today);
        let (found, problems) = scan(&source);
        assert!(problems.is_empty(), "{kind:?}: {problems:?}\n{source}");
        assert_eq!(found.len(), 1, "{source}");
        assert_eq!(found[0].kind, kind);
        assert_eq!(found[0].id.as_ref(), Some(&block_id));
        let printed = compile(
            &NotebookSources::new("design.snxnbk", format!("{PACKAGE_IMPORT}\n{source}\n")),
            &CompileOptions::default(),
        );
        assert!(
            printed.diagnostics.is_empty(),
            "{kind:?}: {:?}",
            printed.diagnostics
        );
    }
    assert_eq!(
        next_block_id(AnnotationKind::Issue, &[], &blocks(ROW_198)),
        id("ISS-005")
    );
    assert_eq!(
        next_block_id(AnnotationKind::Calculation, &[2], &[]),
        id("CALC-2.1")
    );
}

#[test]
fn an_id_of_another_kind_or_a_malformed_one_is_reported_and_dropped() {
    let (found, problems) = scan("#note(id: \"DEC-002\")[A]\n#issue(id: \"ISS-x\")[B]\n");
    assert_eq!(found.len(), 2, "the blocks are kept");
    assert!(found.iter().all(|a| a.id.is_none()));
    let kinds: Vec<ProblemKind> = problems.into_iter().map(|p| p.kind).collect();
    assert_eq!(
        kinds,
        [
            ProblemKind::BadId {
                kind: AnnotationKind::Note,
                id: "DEC-002".into()
            },
            ProblemKind::BadId {
                kind: AnnotationKind::Issue,
                id: "ISS-x".into()
            },
        ]
    );
}

#[test]
fn a_repeated_id_is_reported_and_kept() {
    let (found, problems) = scan("#note(id: \"NOTE-001\")[A]\n#note(id: \"NOTE-001\")[B]\n");
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].id, Some(id("NOTE-001")));
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].line, 2);
    assert_eq!(
        problems[0].kind,
        ProblemKind::DuplicateId {
            id: "NOTE-001".into()
        }
    );
}

#[test]
fn a_field_the_kind_does_not_take_or_cannot_use_is_reported() {
    let (found, problems) = scan(
        "#issue(id: \"ISS-001\", severity: \"huge\", chosen: [A], from: 3, limit: (max: 1))[Body]\n",
    );
    assert_eq!(found.len(), 1);
    let kinds: Vec<ProblemKind> = problems.into_iter().map(|p| p.kind).collect();
    assert!(
        kinds.contains(&ProblemKind::BadField {
            argument: "severity".into(),
            value: "\"huge\"".into()
        }),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&ProblemKind::BadField {
            argument: "from".into(),
            value: "3".into()
        }),
        "{kinds:?}"
    );
    for argument in ["chosen", "limit"] {
        assert!(
            kinds.contains(&ProblemKind::UnknownArgument {
                argument: argument.into()
            }),
            "{argument}: {kinds:?}"
        );
    }
    let (_, problems) = scan("#note(status: \"open\")[A]\n");
    assert_eq!(
        problems[0].kind,
        ProblemKind::UnknownStatus {
            kind: AnnotationKind::Note,
            status: "open".into()
        },
        "a note has no status"
    );
}

#[test]
fn a_body_may_be_given_in_the_parentheses_or_left_out() {
    let found = blocks(
        "#note(id: \"NOTE-001\", [Inline #component(\"R1\").])\n#decision(id: \"DEC-001\")\n",
    );
    assert_eq!(found[0].snippet, "Inline R1.");
    assert_eq!(found[0].title, "Inline R1.");
    assert_eq!(found[1].snippet, "");
    assert_eq!(found[1].title, "");
}

#[test]
fn the_earlier_form_has_no_id_and_an_empty_record() {
    let found = blocks("#annotation(kind: \"decision\", status: \"accepted\")[Use the LDO.]\n");
    assert_eq!(found[0].id, None);
    assert_eq!(found[0].record, Record::empty(AnnotationKind::Decision));
    assert!(links(&found).is_empty());
}

#[test]
fn a_block_inside_another_belongs_to_the_outer_one() {
    let found = blocks("#issue(id: \"ISS-001\")[See #note(id: \"NOTE-001\")[inner].]\n");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, AnnotationKind::Issue);
}

#[test]
fn the_text_filter_finds_a_block_by_its_id() {
    let found = blocks(ROW_198);
    let query = Filter {
        query: "dec-002".into(),
        ..Filter::default()
    };
    let shown: Vec<&Annotation> = found
        .iter()
        .filter(|a| query.shows(a, date("2026-11-25")))
        .collect();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].kind, AnnotationKind::Decision);
}

#[test]
fn a_block_is_a_json_record_cited_by_its_id() {
    let found = blocks(ROW_198);
    let json = serde_json::to_value(&found[0]).expect("json");
    assert_eq!(json["id"], "DEC-002");
    assert_eq!(json["kind"], "decision");
    assert_eq!(json["status"], "accepted");
    assert_eq!(json["date"], "2026-11-18");
    assert_eq!(json["record"]["supersedes"], "DEC-001");
    assert_eq!(json["record"]["alternatives"][0]["option"], "B, 3.3 µH");
    assert_eq!(json["references"][1]["name"], "U3.3");
    let json = serde_json::to_value(&found[3]).expect("json");
    assert_eq!(json["record"]["verdict"], "fail");
    assert_eq!(json["record"]["rows"][0]["limits"]["max"], 250.0);
    let link = serde_json::to_value(&links(&found)[0]).expect("json");
    assert_eq!(link["citation"], "supersedes");
}

#[test]
fn field_values_read_numbers_units_arrays_dictionaries_and_references() {
    use crate::record::value::FieldValue as V;
    let number = |value: f64, unit: Option<&str>| V::Number {
        value,
        unit: unit.map(str::to_string),
    };
    assert_eq!(V::read("310"), number(310.0, None));
    assert_eq!(V::read("-1.5"), number(-1.5, None));
    assert_eq!(V::read("5%"), number(5.0, Some("%")));
    assert_eq!(V::read("2.5mm"), number(2.5, Some("mm")));
    assert_eq!(V::read("\"a, b\""), V::Text("a, b".into()));
    assert_eq!(
        V::read("[*Bold* #net(\"SW\")]"),
        V::Content("Bold SW".into())
    );
    assert_eq!(V::read("none"), V::None);
    assert_eq!(V::read("(:)"), V::Dict(Vec::new()));
    assert_eq!(V::read("()"), V::Array(Vec::new()));
    assert_eq!(
        V::read("(max: 250, min: 1)"),
        V::Dict(vec![
            ("max".into(), number(250.0, None)),
            ("min".into(), number(1.0, None)),
        ])
    );
    assert_eq!(
        V::read("(net(\"SW\"),)"),
        V::Array(vec![V::Reference(net("SW"))])
    );
    assert_eq!(V::read("pin(\"U3.3\")"), V::Reference(pin("U3", "3")));
    assert_eq!(V::read("pin(\"U3\", 3)"), V::Other, "not the quoted names");
    assert_eq!(V::read("(a) + (b)"), V::Other, "two groups, not one");
    assert_eq!(V::read("x.y"), V::Other);
}

#[test]
fn the_power_stage_fixture_is_in_the_corpus() {
    let text = power_stage();
    assert!(text.starts_with("+++\nformat = \"snxnbk/1\""));
}

fn power_stage() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/notebook/power-stage.snxnbk");
    std::fs::read_to_string(path).expect("the power-stage fixture")
}
