//! The model types other notebook features build on: block ids,
//! references and their stored syntax, includes and the sources a compile
//! reads.

use crate::{
    AnnotationKind, BlockId, Include, LoadProblemKind, NotebookSources, Reference, ReferenceAt,
    distinct_references, includes, quote, references, resolve, scan, typst_source,
};

fn pin(component: &str, pin: &str) -> Reference {
    Reference::Pin {
        component: component.into(),
        pin: pin.into(),
    }
}

#[test]
fn each_kind_has_its_id_prefix_and_function() {
    let prefixes: Vec<_> = AnnotationKind::ALL.map(AnnotationKind::id_prefix).to_vec();
    assert_eq!(prefixes, ["NOTE", "CALC", "MEAS", "ISS", "DEC"]);
    let functions: Vec<_> = AnnotationKind::ALL.map(AnnotationKind::function).to_vec();
    assert_eq!(
        functions,
        ["note", "calculation", "measurement", "issue", "decision"]
    );
    for kind in AnnotationKind::ALL {
        assert_eq!(AnnotationKind::from_function(kind.function()), Some(kind));
    }
    assert_eq!(AnnotationKind::from_function("annotation"), None);
}

#[test]
fn a_block_id_is_read_and_written_as_typed() {
    for text in ["NOTE-012", "CALC-2.1", "MEAS-009", "ISS-004", "DEC-002"] {
        let id = BlockId::parse(text).expect(text);
        assert_eq!(id.to_string(), text, "kept as written");
    }
    let calc = BlockId::parse("CALC-2.1").expect("an id");
    assert_eq!(calc.kind(), AnnotationKind::Calculation);
    assert_eq!(calc.serial(), "2.1");
    assert_eq!(calc.numbers(), [2, 1]);
    assert_eq!(BlockId::parse("MEAS-009").expect("an id").numbers(), [9]);
}

#[test]
fn a_malformed_block_id_is_refused() {
    for text in [
        "", "NOTE", "NOTE-", "note-1", "NOTE-1a", "NOTE-1.", "NOTE-.1", "TASK-1", "NOTE-1-2",
    ] {
        assert_eq!(BlockId::parse(text), None, "{text:?}");
    }
    assert_eq!(BlockId::parse_for(AnnotationKind::Note, "DEC-002"), None);
    assert!(BlockId::parse_for(AnnotationKind::Decision, "DEC-002").is_some());
}

#[test]
fn block_ids_sort_by_kind_then_number() {
    let mut ids: Vec<BlockId> = ["MEAS-010", "NOTE-2", "MEAS-9", "CALC-2.10", "CALC-2.9"]
        .into_iter()
        .filter_map(BlockId::parse)
        .collect();
    ids.sort();
    let texts: Vec<String> = ids.iter().map(BlockId::to_string).collect();
    let sorted = ["NOTE-2", "CALC-2.9", "CALC-2.10", "MEAS-9", "MEAS-010"];
    assert_eq!(texts, sorted);
}

#[test]
fn the_next_free_id_follows_the_highest_of_its_kind() {
    let written = [
        "MEAS-009", "MEAS-003", "DEC-040", "CALC-2.1", "CALC-2.4", "CALC-3.1",
    ];
    let taken: Vec<BlockId> = written.into_iter().filter_map(BlockId::parse).collect();
    let next = |kind, section: &[u64]| BlockId::next_free(kind, section, &taken).to_string();
    assert_eq!(next(AnnotationKind::Measurement, &[]), "MEAS-010");
    assert_eq!(next(AnnotationKind::Note, &[]), "NOTE-001");
    assert_eq!(next(AnnotationKind::Calculation, &[2]), "CALC-2.5");
    assert_eq!(next(AnnotationKind::Calculation, &[4]), "CALC-4.1");
    // Dotted ids are per section, so they do not move the flat numbers.
    assert_eq!(next(AnnotationKind::Calculation, &[]), "CALC-001");
}

#[test]
fn a_block_id_is_a_json_string() {
    let id = BlockId::parse("ISS-004").expect("an id");
    assert_eq!(serde_json::to_string(&id).expect("json"), "\"ISS-004\"");
}

#[test]
fn references_are_stored_as_typst_calls() {
    assert_eq!(
        Reference::Component("R7".into()).to_markup(),
        "#component(\"R7\")"
    );
    assert_eq!(pin("U3", "3").to_markup(), "#pin(\"U3\", \"3\")");
    assert_eq!(Reference::Net("+3V3".into()).to_markup(), "#net(\"+3V3\")");
    assert_eq!(pin("U3", "3").to_code(), "pin(\"U3\", \"3\")");
    let escaped = Reference::Net("A\"B\\C".into()).to_code();
    assert_eq!(escaped, "net(\"A\\\"B\\\\C\")");
}

#[test]
fn a_quoted_name_reads_back_as_the_same_name() {
    let names = ["R7", "+3V3", "say \"hi\"", "back\\slash", "tab\there"];
    for name in names.into_iter().chain(["line\nbreak"]) {
        let markup = Reference::Net(name.into()).to_markup();
        assert_eq!(
            distinct_references(&markup),
            [Reference::Net(name.into())],
            "{markup}"
        );
    }
    assert_eq!(quote("\u{1}"), "\"\\u{1}\"");
}

#[test]
fn a_pin_may_be_one_dotted_name() {
    assert_eq!(Reference::dotted_pin("U3.3"), Some(pin("U3", "3")));
    assert_eq!(Reference::dotted_pin("U3.A1.2"), Some(pin("U3", "A1.2")));
    for bad in ["U3", ".3", "U3.", " . "] {
        assert_eq!(Reference::dotted_pin(bad), None, "{bad:?}");
    }
    let (found, problems) = scan("#annotation(kind: \"note\")[On #pin(\"U3.3\").]");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(found[0].references, [pin("U3", "3")]);
}

#[test]
fn a_reference_is_a_json_object() {
    let json = serde_json::to_value(pin("U3", "3")).expect("json");
    assert_eq!(
        json,
        serde_json::json!({"kind": "pin", "name": "U3.3", "component": "U3", "pin": "3"})
    );
    let json = serde_json::to_value(Reference::Component("R7".into())).expect("json");
    assert_eq!(json, serde_json::json!({"kind": "component", "name": "R7"}));
}

#[test]
fn every_reference_is_found_in_markup_and_in_code() {
    let source = "+++\nformat = \"snxnbk/1\"\n+++\nSet by #component(\"R7\").value into #pin(\"U3\", \"12\").\n#note(id: \"NOTE-1\", target: (net(\"SW\"), pin(\"U3.3\")))[On #net(\"+3V3\") and #component(\"R7\").]\n";
    let found = references(source);
    let expected = [
        (Reference::Component("R7".into()), 4),
        (pin("U3", "12"), 4),
        (Reference::Net("SW".into()), 5),
        (pin("U3", "3"), 5),
        (Reference::Net("+3V3".into()), 5),
        (Reference::Component("R7".into()), 5),
    ];
    let expected: Vec<ReferenceAt> = expected
        .into_iter()
        .map(|(reference, line)| ReferenceAt { reference, line })
        .collect();
    assert_eq!(found, expected);
    assert_eq!(distinct_references(source).len(), 5, "R7 once");
}

#[test]
fn calls_that_are_not_plain_references_are_left_out() {
    let source = "#component(name) #component(\"\") #net(\"A\", \"B\") #pin(\"U3\") #component[R7] #resistor(\"R1\")";
    assert!(references(source).is_empty(), "{:?}", references(source));
}

#[test]
fn the_header_is_blanked_line_for_line() {
    let source = "+++\nformat = \"snxnbk/1\"\n+++\n= Title\n";
    assert_eq!(typst_source(source), "\n\n\n= Title\n");
    assert_eq!(typst_source("= Plain\n"), "= Plain\n");
    let unclosed = "+++\nformat = 1\n= Title\n";
    assert_eq!(typst_source(unclosed), unclosed);
}

#[test]
fn includes_are_listed_with_their_lines() {
    let source = "+++\n+++\n= Notebook\n#include \"chapters/power.snxnbk\"\n// #include \"commented.snxnbk\"\n#include(\"computed\" + \".typ\")\n#include \"/appendix.typ\"\n";
    assert_eq!(
        includes(source),
        [
            Include {
                path: "chapters/power.snxnbk".into(),
                line: 4
            },
            Include {
                path: "/appendix.typ".into(),
                line: 7
            },
        ]
    );
}

#[test]
fn an_include_path_resolves_inside_the_notebook_folder() {
    let (design, power) = ("design.snxnbk", "chapters/power.snxnbk");
    let cases = [
        (design, "chapters/power.snxnbk", Some(power)),
        (power, "../appendix.typ", Some("appendix.typ")),
        (power, "/img/a.png", Some("img/a.png")),
        (power, "./b\\c.typ", Some("chapters/b/c.typ")),
        (design, "../secret.typ", None),
        (design, "/../secret.typ", None),
    ];
    for (from, path, expected) in cases {
        assert_eq!(resolve(from, path).as_deref(), expected, "{from} + {path}");
    }
}

#[test]
fn load_reads_every_chapter_once_and_lists_what_it_could_not_read() {
    let design = "#include \"chapters/a.snxnbk\"\n#include \"chapters/missing.snxnbk\"\n\
                  #include \"../outside.typ\"\n";
    let files = [
        ("design.snxnbk", design),
        (
            "chapters/a.snxnbk",
            "#include \"b.snxnbk\"\n#include \"../design.snxnbk\"\n",
        ),
        ("chapters/b.snxnbk", "#include \"a.snxnbk\"\nLeaf.\n"),
    ];
    let mut reads = Vec::new();
    let (sources, problems) = NotebookSources::load("design.snxnbk", |path| {
        reads.push(path.to_string());
        files
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, text)| text.as_bytes().to_vec())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
    });
    assert_eq!(sources.main(), "design.snxnbk");
    let held: Vec<&str> = sources.paths().collect();
    let (a, b) = ("chapters/a.snxnbk", "chapters/b.snxnbk");
    assert_eq!(held, [a, b, "design.snxnbk"]);
    // Each file is read once; the cycle back to the notebook file is not
    // followed.
    let missing = "chapters/missing.snxnbk";
    assert_eq!(reads, ["design.snxnbk", a, b, missing]);
    let summary: Vec<(&str, u32, &str, bool)> = problems
        .iter()
        .map(|p| {
            let outside = p.kind == LoadProblemKind::OutsideNotebook;
            (p.file.as_str(), p.line, p.path.as_str(), outside)
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("design.snxnbk", 2, "chapters/missing.snxnbk", false),
            ("design.snxnbk", 3, "../outside.typ", true),
        ]
    );
}

#[test]
fn an_attachment_outside_the_folder_is_refused() {
    let mut sources = NotebookSources::new("design.snxnbk", "Body");
    assert!(sources.insert("attachments/ripple.csv", "t,v\n"));
    assert!(!sources.insert("../elsewhere.csv", "t,v\n"));
    let csv = sources.get("attachments/ripple.csv");
    assert_eq!(csv, Some("t,v\n".as_bytes()));
    assert_eq!(sources.get("design.snxnbk"), Some("Body".as_bytes()));
}
