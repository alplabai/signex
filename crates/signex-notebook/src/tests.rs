use crate::{
    Annotation, AnnotationKind, DateWindow, Filter, IsoDate, ProblemKind, Reference, ScanProblem,
    Status, newest_first, scan,
};

fn only(source: &str) -> Annotation {
    let (found, problems) = scan(source);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(found.len(), 1, "{found:?}");
    found.into_iter().next().expect("one block")
}

fn kinds_of(problems: &[ScanProblem]) -> Vec<(u32, ProblemKind)> {
    problems.iter().map(|p| (p.line, p.kind.clone())).collect()
}

fn component(name: &str) -> Reference {
    Reference::Component(name.into())
}

fn date(text: &str) -> IsoDate {
    IsoDate::parse(text).expect("a date")
}

#[test]
fn each_kind_is_read_from_its_token() {
    for kind in AnnotationKind::ALL {
        let block = format!("#annotation(kind: \"{}\")[Body]", kind.token());
        assert_eq!(only(&block).kind, kind, "{}", kind.token());
    }
    assert_eq!(AnnotationKind::parse("Issue"), None, "tokens are exact");
}

#[test]
fn every_argument_is_read() {
    let source = "Intro text.\n\n#annotation(kind: \"issue\", status: \"open\", title: \"U3 runs warm\", date: \"2026-11-17\")[Case reaches 71 °C from #net(\"VBUS\").]\n";
    let found = only(source);
    assert_eq!(found.kind, AnnotationKind::Issue);
    assert_eq!(found.status, Some(Status::Open));
    assert_eq!(found.title, "U3 runs warm");
    assert_eq!(found.date, Some(date("2026-11-17")));
    assert_eq!(found.line, 3);
    assert_eq!(found.references, vec![Reference::Net("VBUS".into())]);
    assert_eq!(found.snippet, "Case reaches 71 °C from VBUS.");
    assert!(found.source.starts_with("#annotation(kind: \"issue\""));
    assert!(found.source.ends_with("#net(\"VBUS\").]"));
}

#[test]
fn every_kind_accepts_only_its_own_statuses() {
    let cases = [
        (AnnotationKind::Issue, "resolved", Some(Status::Resolved)),
        (AnnotationKind::Decision, "proposed", Some(Status::Proposed)),
        (AnnotationKind::Decision, "accepted", Some(Status::Accepted)),
        (
            AnnotationKind::Decision,
            "superseded",
            Some(Status::Superseded),
        ),
        (
            AnnotationKind::Measurement,
            "planned",
            Some(Status::Planned),
        ),
        (AnnotationKind::Measurement, "done", Some(Status::Done)),
        (
            AnnotationKind::Calculation,
            "current",
            Some(Status::Current),
        ),
        (AnnotationKind::Calculation, "stale", Some(Status::Stale)),
        (AnnotationKind::Issue, "done", None),
        (AnnotationKind::Note, "open", None),
    ];
    for (kind, token, want) in cases {
        assert_eq!(Status::parse(kind, token), want, "{kind:?} {token}");
    }
    assert!(AnnotationKind::Note.statuses().is_empty());
}

#[test]
fn a_block_without_a_title_takes_its_first_line_cut_at_the_colon() {
    let source = "#annotation(kind: \"calculation\", status: \"current\")[\n  Feedback divider for #component(\"U1\"):\n  $ V_\"out\" = 0.8 \"V\" (1 + #component(\"R1\").value / #component(\"R2\").value) $\n]";
    let found = only(source);
    assert_eq!(found.title, "Feedback divider for U1");
    assert_eq!(
        found.snippet,
        "Feedback divider for U1: V_out = 0.8 V (1 + R1 / R2)"
    );
    assert_eq!(
        found.references,
        vec![component("U1"), component("R1"), component("R2")]
    );
    assert_eq!(found.status, Some(Status::Current));
}

#[test]
fn a_fallback_title_drops_markup_and_stops_at_sixty_characters() {
    let long = "x".repeat(80);
    let found = only(&format!(
        "#annotation(kind: \"note\")[\n= *Heading* {long}\nsecond line]"
    ));
    assert_eq!(found.title.chars().count(), crate::TITLE_MAX_CHARS);
    assert!(found.title.starts_with("Heading xxx"), "{}", found.title);
    let empty_title = only("#annotation(kind: \"note\", title: \"  \")[- First item]");
    assert_eq!(empty_title.title, "First item");
}

#[test]
fn brackets_in_strings_comments_escapes_and_raw_do_not_close_the_body() {
    let source = "#annotation(kind: \"note\", title: \"a ] b\")[Body with #box(inset: \"]\")[inner [x] y] // comment ]\n and /* ] */ end \\] `raw ] text`]\n#annotation(kind: \"issue\")[Next]";
    let (found, problems) = scan(source);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].title, "a ] b");
    assert_eq!(
        found[0].snippet,
        "Body with inner [x] y and end ] raw ] text"
    );
    assert_eq!(found[1].kind, AnnotationKind::Issue);
    assert_eq!(found[1].line, 3);
}

#[test]
fn references_are_found_with_parameter_access_and_pins_each_once() {
    let found = only(
        "#annotation(kind: \"note\")[Uses #component(\"R3\").param(\"Vref\") and #pin(\"U1\", \"3\") on #net(\"VOUT_3V3\"), again #component(\"R3\").]",
    );
    assert_eq!(
        found.references,
        vec![
            component("R3"),
            Reference::Pin {
                component: "U1".into(),
                pin: "3".into()
            },
            Reference::Net("VOUT_3V3".into()),
        ]
    );
    assert_eq!(found.snippet, "Uses R3 and U1.3 on VOUT_3V3, again R3.");
    assert_eq!(found.first_component(), Some("R3"));
}

#[test]
fn a_link_is_not_a_comment_and_a_commented_block_is_not_read() {
    let source = "// #annotation(kind: \"issue\")[hidden]\n/* #annotation(kind: \"issue\")[hidden] */\n#annotation(kind: \"note\")[See https://example.com/a for #component(\"U9\")]";
    let found = only(source);
    assert_eq!(found.references, vec![component("U9")]);
    assert_eq!(found.line, 3);
}

#[test]
fn the_front_matter_is_skipped_and_lines_still_count_from_the_file_start() {
    let source = "+++\nids = { U3 = \"0190\" }\nfake = \"#annotation(kind: \\\"issue\\\")[x]\"\n+++\n\n#annotation(kind: \"decision\", status: \"accepted\")[Use #component(\"U3\")]";
    let found = only(source);
    assert_eq!(found.kind, AnnotationKind::Decision);
    assert_eq!(found.line, 6);
}

#[test]
fn an_unclosed_front_matter_is_reported_and_the_file_read_as_typst() {
    let (found, problems) = scan("+++\nids = {}\n#annotation(kind: \"note\")[Kept]");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(kinds_of(&problems), [(1, ProblemKind::UnclosedFrontMatter)]);
}

#[test]
fn an_unknown_kind_skips_the_block_and_names_its_line() {
    let source = "#annotation(kind: \"todo\")[Skipped]\n\n#annotation(kind: \"note\")[Kept]";
    let (found, problems) = scan(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
    assert_eq!(
        kinds_of(&problems),
        [(
            1,
            ProblemKind::UnknownKind {
                kind: "todo".into()
            }
        )]
    );
    assert_eq!(problems[0].text, "#annotation(kind: \"todo\")[Skipped]");
}

#[test]
fn a_missing_or_unquoted_kind_skips_the_block_with_one_problem() {
    let (found, problems) = scan("#annotation(status: \"open\")[A]\n#annotation(kind: issue)[B]");
    assert!(found.is_empty());
    assert_eq!(
        kinds_of(&problems),
        [
            (1, ProblemKind::MissingKind),
            (
                2,
                ProblemKind::NotText {
                    argument: "kind".into()
                }
            ),
        ]
    );
}

#[test]
fn an_unknown_status_keeps_the_block_without_a_status() {
    let (found, problems) = scan(
        "#annotation(kind: \"issue\", status: \"done\")[A]\n#annotation(kind: \"note\", status: \"open\")[B]",
    );
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|a| a.status.is_none()));
    assert_eq!(
        kinds_of(&problems),
        [
            (
                1,
                ProblemKind::UnknownStatus {
                    kind: AnnotationKind::Issue,
                    status: "done".into()
                }
            ),
            (
                2,
                ProblemKind::UnknownStatus {
                    kind: AnnotationKind::Note,
                    status: "open".into()
                }
            ),
        ]
    );
}

#[test]
fn a_bad_date_an_unknown_argument_and_a_bad_reference_are_reported_and_the_block_kept() {
    let (found, problems) = scan(
        "#annotation(kind: \"note\", date: \"2026-02-30\", owner: \"me\")[See #component(U3) and #net(\"GND\")]",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].date, None);
    assert_eq!(found[0].references, vec![Reference::Net("GND".into())]);
    assert_eq!(
        kinds_of(&problems),
        [
            (
                1,
                ProblemKind::UnknownArgument {
                    argument: "owner".into()
                }
            ),
            (
                1,
                ProblemKind::BadDate {
                    date: "2026-02-30".into()
                }
            ),
            (
                1,
                ProblemKind::BadReference {
                    reference: "#component(U3)".into()
                }
            ),
        ]
    );
}

#[test]
fn a_block_without_a_body_is_skipped() {
    let (found, problems) = scan("#annotation(kind: \"note\") [spaced]");
    assert!(found.is_empty());
    assert_eq!(kinds_of(&problems), [(1, ProblemKind::MissingBody)]);
}

#[test]
fn an_unbalanced_block_is_skipped_and_named_by_its_first_line() {
    let source =
        "#annotation(kind: \"note\")[Kept]\n\n#annotation(kind: \"issue\")[\nnever [closed]\n";
    let (found, problems) = scan(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Kept");
    assert_eq!(kinds_of(&problems), [(3, ProblemKind::UnbalancedBlock)]);
}

#[test]
fn an_unclosed_bracket_outside_any_block_names_its_opener() {
    let (found, problems) = scan("#annotation(kind: \"note\")[Kept]\n#box[\nopen");
    assert_eq!(found.len(), 1);
    assert_eq!(
        kinds_of(&problems),
        [(2, ProblemKind::Unclosed { opener: '[' })]
    );
    let (_, problems) = scan("text $ x + y\n");
    assert_eq!(
        kinds_of(&problems),
        [(1, ProblemKind::Unclosed { opener: '$' })]
    );
}

#[test]
fn a_block_inside_another_belongs_to_the_outer_one() {
    let found = only("#annotation(kind: \"note\")[Outer #annotation(kind: \"issue\")[inner]]");
    assert_eq!(found.kind, AnnotationKind::Note);
    assert_eq!(found.snippet, "Outer inner");
}

#[test]
fn dates_parse_strictly_and_count_days() {
    assert_eq!(
        IsoDate::parse("2026-11-17").map(|d| d.month_day()),
        Some("11-17".into())
    );
    for bad in [
        "2026-13-01",
        "2026-02-29",
        "26-11-17",
        "2026/11/17",
        "2026-1-170",
    ] {
        assert_eq!(IsoDate::parse(bad), None, "{bad}");
    }
    assert!(IsoDate::parse("2024-02-29").is_some(), "a leap day");
    assert_eq!(date("1970-01-01").days_since_epoch(), 0);
    let day = date("2026-11-19");
    assert_eq!(
        IsoDate::from_days_since_epoch(day.days_since_epoch()),
        Some(day)
    );
    assert_eq!(
        date("2026-03-01").days_since_epoch() - date("2026-02-28").days_since_epoch(),
        1
    );
    assert_eq!(day.to_string(), "2026-11-19");
}

fn annotation(
    kind: AnnotationKind,
    status: Option<Status>,
    day: Option<&str>,
    line: u32,
) -> Annotation {
    Annotation {
        kind,
        title: format!("Title {line}"),
        status,
        date: day.map(date),
        source: String::new(),
        snippet: "Ripple on VOUT".into(),
        references: vec![component("U3"), Reference::Net("VBUS".into())],
        line,
    }
}

#[test]
fn the_filter_keeps_kinds_parts_statuses_text_and_dates() {
    let today = date("2026-11-19");
    let issue = annotation(
        AnnotationKind::Issue,
        Some(Status::Open),
        Some("2026-11-13"),
        1,
    );
    let everything = Filter::default();
    assert!(everything.shows(&issue, today));
    let hidden = Filter {
        hidden_kinds: [AnnotationKind::Issue].into_iter().collect(),
        ..Filter::default()
    };
    assert!(!hidden.shows(&issue, today));
    let part = |name: &str| Filter {
        part: Some(name.into()),
        ..Filter::default()
    };
    assert!(part("U3").shows(&issue, today));
    assert!(!part("VBUS").shows(&issue, today), "a net is not a part");
    let status = |s| Filter {
        status: Some(s),
        ..Filter::default()
    };
    assert!(status(Status::Open).shows(&issue, today));
    assert!(!status(Status::Resolved).shows(&issue, today));
    let query = |q: &str| Filter {
        query: q.into(),
        ..Filter::default()
    };
    assert!(query("vbus").shows(&issue, today), "a reference name");
    assert!(query("RIPPLE").shows(&issue, today), "the snippet");
    assert!(!query("missing").shows(&issue, today));
    let window = |w| Filter {
        date: w,
        ..Filter::default()
    };
    assert!(
        window(DateWindow::Last7Days).shows(&issue, today),
        "six days ago"
    );
    let older = annotation(AnnotationKind::Issue, None, Some("2026-11-12"), 2);
    assert!(
        !window(DateWindow::Last7Days).shows(&older, today),
        "seven days ago"
    );
    assert!(window(DateWindow::Last30Days).shows(&older, today));
    let undated = annotation(AnnotationKind::Note, None, None, 3);
    assert!(!window(DateWindow::Last30Days).shows(&undated, today));
    assert!(window(DateWindow::Any).shows(&undated, today));
}

#[test]
fn newest_first_puts_undated_last_and_keeps_source_order_on_a_tie() {
    let mut list = [
        annotation(AnnotationKind::Note, None, None, 1),
        annotation(AnnotationKind::Note, None, Some("2026-11-10"), 2),
        annotation(AnnotationKind::Note, None, Some("2026-11-17"), 3),
        annotation(AnnotationKind::Note, None, Some("2026-11-10"), 4),
    ];
    list.sort_by(newest_first);
    let lines: Vec<u32> = list.iter().map(|a| a.line).collect();
    assert_eq!(lines, [3, 2, 4, 1]);
}
