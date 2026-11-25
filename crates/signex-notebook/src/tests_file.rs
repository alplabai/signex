//! The `.snxnbk` file: header, format token, byte-for-byte round trip and
//! the fixture corpus.

use std::path::{Path, PathBuf};

use crate::{
    Header, HeaderError, NOTEBOOK_FORMAT, NotebookFile, NotebookFileError, Reference,
    StoredReference, distinct_references,
};

const ID_R7: &str = "0193a0c4-5b1e-7c3a-9f00-6f1c2d3e40e2";

fn uuid(text: &str) -> uuid::Uuid {
    uuid::Uuid::parse_str(text).expect("a uuid")
}

/// Every `.snxnbk` fixture of the corpus, sorted.
fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../signex-types/tests/corpus/notebook");
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("the corpus folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e == crate::NOTEBOOK_EXTENSION)
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The corpus cannot shrink unnoticed; raise it with the fixtures.
const MIN_NOTEBOOK_FIXTURES: usize = 4;

#[test]
fn every_corpus_notebook_saves_back_byte_for_byte() {
    let fixtures = corpus();
    assert!(
        fixtures.len() >= MIN_NOTEBOOK_FIXTURES,
        "found {} notebook fixtures",
        fixtures.len()
    );
    for path in fixtures {
        let text = std::fs::read_to_string(&path).expect("a fixture");
        let file = NotebookFile::parse(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        assert_eq!(file.write().expect("a save"), text, "{path:?}");
    }
}

#[test]
fn every_corpus_header_is_in_canonical_form() {
    for path in corpus() {
        let text = std::fs::read_to_string(&path).expect("a fixture");
        let file = NotebookFile::parse(&text).expect("a notebook");
        let canonical = format!("+++\n{}+++\n{}", file.header.to_toml(), file.body);
        if text.starts_with("+++") {
            assert_eq!(canonical, text, "{path:?} is not canonical");
        }
    }
}

#[test]
fn every_corpus_header_stores_what_its_text_references() {
    for path in corpus() {
        let text = std::fs::read_to_string(&path).expect("a fixture");
        let file = NotebookFile::parse(&text).expect("a notebook");
        let made = distinct_references(&text);
        for stored in &file.header.references {
            let reference = &stored.reference;
            assert!(
                made.contains(reference),
                "{path:?} stores {reference:?}, which its text does not make"
            );
        }
    }
}

#[test]
fn a_header_round_trips_through_its_model() {
    let mut header = Header::default();
    header.set_id(&Reference::Component("R7".into()), Some(uuid(ID_R7)));
    header.set_id(
        &Reference::Pin {
            component: "U3".into(),
            pin: "12".into(),
        },
        None,
    );
    header.set_id(&Reference::Net("say \"hi\"".into()), None);
    header
        .extra
        .insert("layout".into(), toml::Value::String("split".into()));
    header.references[2]
        .extra
        .insert("checked on".into(), toml::Value::Integer(3));
    let toml = header.to_toml();
    let (read, token) = Header::read(&toml).expect("a header");
    assert_eq!(read, header, "{toml}");
    assert_eq!(token, "snxnbk/1");
    assert_eq!(read.to_toml(), toml, "same header, same bytes");
    let head = "format = \"snxnbk/1\"\nlayout = \"split\"\n";
    assert!(toml.starts_with(head), "{toml}");
}

#[test]
fn stored_ids_are_found_set_and_pruned() {
    let r7 = Reference::Component("R7".into());
    let net = Reference::Net("+3V3".into());
    let mut header = Header::default();
    assert!(header.is_empty());
    header.set_id(&r7, None);
    header.set_id(&net, None);
    header.set_id(&r7, Some(uuid(ID_R7)));
    assert_eq!(header.references.len(), 2, "set_id replaces");
    assert_eq!(header.id_of(&r7), Some(uuid(ID_R7)));
    assert_eq!(header.id_of(&net), None);
    header.retain_used(std::slice::from_ref(&net));
    assert_eq!(
        header.references,
        [StoredReference {
            reference: net,
            id: None,
            extra: toml::Table::new(),
        }]
    );
}

#[test]
fn a_header_without_a_token_reads_as_version_one() {
    let (header, token) = Header::read("").expect("a header");
    assert!(header.is_empty());
    assert_eq!(token, "snxnbk/1");
    assert_eq!(header.written_token(), NOTEBOOK_FORMAT.current);
}

#[test]
fn a_bad_header_is_refused_with_its_reason() {
    let bad_entry = |e: &HeaderError| matches!(e, HeaderError::BadEntry { index: 0, .. });
    let cases: [(&str, fn(&HeaderError) -> bool); 7] = [
        ("format = ", |e| matches!(e, HeaderError::Toml(_))),
        ("format = 1", |e| matches!(e, HeaderError::NotText { .. })),
        ("format = \"snxnbk/2\"", |e| {
            matches!(e, HeaderError::Version(_))
        }),
        ("references = 3", |e| matches!(e, HeaderError::NotAList)),
        ("[[references]]\nkind = \"sheet\"", bad_entry),
        (
            "[[references]]\nkind = \"pin\"\ncomponent = \"U3\"",
            bad_entry,
        ),
        ("[[references]]\nkind = \"net\"\nid = \"x\"", bad_entry),
    ];
    let error = Header::read("[[references]]\nkind = \"net\"\nnet = \"A\"\nid = \"nope\"");
    assert!(matches!(error, Err(HeaderError::BadId { index: 0, .. })));
    for (text, is_expected) in cases {
        let error = Header::read(text).expect_err(text);
        assert!(is_expected(&error), "{text:?}: {error:?}");
    }
}

#[test]
fn a_newer_minor_opens_read_only_and_is_not_saved() {
    let text = "+++\nformat = \"snxnbk/1.1\"\nlater = true\n+++\nBody\n";
    let file = NotebookFile::parse(text).expect("read-only");
    assert!(file.is_read_only());
    assert_eq!(file.token(), "snxnbk/1.1");
    assert!(matches!(
        file.write(),
        Err(NotebookFileError::Header(HeaderError::Version(_)))
    ));
}

#[test]
fn an_unclosed_header_is_refused() {
    assert_eq!(
        NotebookFile::parse("+++\nformat = \"snxnbk/1\"\n= Title\n"),
        Err(NotebookFileError::UnclosedHeader)
    );
}

#[test]
fn an_unchanged_file_is_written_as_read() {
    let text = "\u{feff}+++\r\n# kept comment\nformat=\"snxnbk/1\"\n\n[[references]]\nkind=\"net\"\nnet=\"VBUS\"\n+++  \r\n= Title\n";
    let file = NotebookFile::parse(text).expect("a notebook");
    assert_eq!(file.body, "= Title\n");
    assert_eq!(file.header.references.len(), 1);
    assert_eq!(file.write().expect("a save"), text);
}

#[test]
fn a_changed_header_is_written_canonically() {
    let text = "+++\n# a comment\nformat=\"snxnbk/1\"\n+++\n= Title\n#component(\"R7\")\n";
    let mut file = NotebookFile::parse(text).expect("a notebook");
    file.header
        .set_id(&Reference::Component("R7".into()), Some(uuid(ID_R7)));
    let written = file.write().expect("a save");
    assert_eq!(
        written,
        format!(
            "+++\nformat = \"snxnbk/1\"\n\n[[references]]\nkind = \"component\"\ncomponent = \"R7\"\nid = \"{ID_R7}\"\n+++\n= Title\n#component(\"R7\")\n"
        )
    );
    let read_back = NotebookFile::parse(&written).expect("reads back");
    assert_eq!(read_back.header, file.header);
}

#[test]
fn a_file_without_a_header_keeps_none_until_it_stores_something() {
    let text = "= Plain Typst\n";
    let mut file = NotebookFile::parse(text).expect("a notebook");
    assert_eq!(file.token(), "snxnbk/1");
    assert_eq!(file.write().expect("a save"), text, "byte for byte");
    file.header.set_id(&Reference::Net("GND".into()), None);
    let written = file.write().expect("a save");
    let head = "+++\nformat = \"snxnbk/1\"\n";
    assert!(written.starts_with(head), "{written}");
    assert!(written.ends_with("+++\n= Plain Typst\n"), "{written}");
}

#[test]
fn a_new_notebook_starts_with_its_token() {
    let file = NotebookFile::new("= Notebook\n");
    assert!(!file.is_read_only());
    assert_eq!(
        file.write().expect("a save"),
        "+++\nformat = \"snxnbk/1\"\n+++\n= Notebook\n"
    );
}

#[test]
fn the_annotation_scan_still_skips_the_header() {
    let file = NotebookFile::new("#annotation(kind: \"note\")[Body]\n");
    let text = file.write().expect("a save");
    let (found, problems) = crate::scan(&text);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
}
