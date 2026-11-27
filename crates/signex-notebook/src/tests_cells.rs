//! Python cells (mockup row 200): reading cells from the notebook, units on
//! schematic values, the values a cell reads, the sandbox rules, asking
//! before a notebook from outside the project runs, and the result cache.

use crate::NotebookSources;
use crate::cell::{
    Access, CELL_CACHE_FORMAT, CacheNote, CachedCell, CellCache, CellProblemKind, Field,
    InputValue, Quantity, RefusalKind, RunOutput, SANDBOX_RULE, SandboxPolicy, Trust, TrustAnswers,
    Unit, cells_in, classify, is_inside, notebook_cells, source_hash, value_reads,
};

/// The cell of mockup row 200, as the file holds it.
pub(crate) const RIPPLE: &str = "== Ripple versus input voltage\n\
\n\
#python(label: \"ripple-vs-vin\")[```python\n\
from signex import component, units as u\n\
L = component(\"L1\").value  # 4.7 uH\n\
C = component(\"C15\").value\n\
esr = component(\"C15\").param(\"ESR\")\n\
fsw = component(\"U3\").param(\"fsw\")\n\
print(L)\n\
```]\n";

#[test]
fn a_cell_is_read_with_its_label_lines_and_source() {
    let (cells, problems) = cells_in("design.snxnbk", RIPPLE);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(cells.len(), 1);
    let cell = &cells[0];
    assert_eq!(cell.label, "ripple-vs-vin");
    assert_eq!(cell.file, "design.snxnbk");
    assert_eq!(cell.line, 3);
    assert_eq!(cell.source_line, 4, "the line after the fence");
    let source = cell.source.as_str();
    assert!(source.starts_with("from signex import"), "{source}");
    assert!(source.ends_with("print(L)"), "{source}");
    assert_eq!(cell.file_line(1), 4);
    assert_eq!(cell.file_line(6), 9);
}

#[test]
fn a_header_is_skipped_and_lines_stay_those_of_the_file() {
    let text = format!("+++\nformat = \"snxnbk/1\"\n+++\n{RIPPLE}");
    let (cells, _) = cells_in("design.snxnbk", &text);
    assert_eq!((cells[0].line, cells[0].source_line), (6, 7));
}

#[test]
fn an_indented_cell_loses_the_indent_its_lines_share() {
    let text = "#python(label: \"a\")[\n  ```python\n  def f():\n      return 1\n  ```\n]\n";
    let (cells, problems) = cells_in("design.snxnbk", text);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(cells[0].source, "def f():\n    return 1");
    assert_eq!(cells[0].source_line, 3);
}

#[test]
fn a_call_that_is_not_a_runnable_cell_is_a_problem_on_its_line() {
    let text = "#python[```python\nx = 1\n```]\n\
                #python(label: name)[```python\nx = 1\n```]\n\
                #python(label: \"b\")[```matlab\nx = 1\n```]\n\
                #python(label: \"c\")[no raw block]\n\
                #python(label: \"d\", timeout: 5)[```python\nx = 1\n```]\n\
                #python(label: \"d\")[```python\nx = 2\n```]\n";
    let (cells, problems) = cells_in("design.snxnbk", text);
    let kinds: Vec<_> = problems.iter().map(|p| (p.line, p.kind.clone())).collect();
    assert_eq!(
        kinds,
        [
            (1, CellProblemKind::NoLabel),
            (4, CellProblemKind::LabelNotText),
            (
                7,
                CellProblemKind::NotPython {
                    language: "matlab".into()
                }
            ),
            (10, CellProblemKind::NoSource),
            (
                11,
                CellProblemKind::UnknownArgument {
                    argument: "timeout".into()
                }
            ),
            (14, CellProblemKind::DuplicateLabel { label: "d".into() }),
        ]
    );
    assert_eq!(cells.len(), 1, "the first d runs");
    assert_eq!(cells[0].source, "x = 1");
}

#[test]
fn a_label_is_unique_across_the_chapters() {
    let mut sources = NotebookSources::new("design.snxnbk", RIPPLE);
    assert!(sources.insert("chapters/power.snxnbk", RIPPLE));
    assert!(sources.insert("attachments/ripple.csv", "#python"));
    let (cells, problems) = notebook_cells(&sources);
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].file, "chapters/power.snxnbk", "first by path");
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].file, "design.snxnbk");
}

#[test]
fn schematic_values_read_with_their_units() {
    let micro_henry = Quantity::parse("4.7µH", Unit::Henry).unwrap();
    assert_eq!(micro_henry.unit, Unit::Henry);
    assert!((micro_henry.magnitude - 4.7e-6).abs() < 1e-15);
    let cases = [
        ("4.7µH", Unit::One, "4.7 µH"),
        ("4.7uH", Unit::One, "4.7 µH"),
        ("22 uF", Unit::One, "22 µF"),
        ("5mΩ", Unit::One, "5 mΩ"),
        ("500kHz", Unit::One, "500 kHz"),
        ("10k", Unit::Ohm, "10 kΩ"),
        ("4k7", Unit::Ohm, "4.7 kΩ"),
        ("2R2", Unit::One, "2.2 Ω"),
        ("100n", Unit::Farad, "100 nF"),
        ("3.3V", Unit::One, "3.3 V"),
        ("1M", Unit::Ohm, "1 MΩ"),
        ("0.0167 V", Unit::One, "16.7 mV"),
        ("999.96", Unit::Ohm, "1 kΩ"),
        ("12", Unit::One, "12"),
    ];
    for (text, implied, shown) in cases {
        let quantity = Quantity::parse(text, implied).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(quantity.display(), shown, "{text}");
    }
    for text in ["DNP", "BAT54", "", "4.7 furlongs"] {
        assert_eq!(Quantity::parse(text, Unit::One), None, "{text}");
    }
}

#[test]
fn a_bare_value_takes_the_unit_of_its_part_kind() {
    assert_eq!(Unit::implied_by("R7"), Unit::Ohm);
    assert_eq!(Unit::implied_by("C15"), Unit::Farad);
    assert_eq!(Unit::implied_by("L1"), Unit::Henry);
    assert_eq!(Unit::implied_by("U3"), Unit::One);
}

#[test]
fn the_values_a_cell_reads_are_found_in_its_source() {
    let (cells, _) = cells_in("design.snxnbk", RIPPLE);
    let reads = value_reads(&cells[0].source);
    let found: Vec<_> = reads
        .iter()
        .map(|r| (r.designator.as_str(), r.field.clone(), r.symbol.as_deref()))
        .collect();
    assert_eq!(
        found,
        [
            ("L1", Field::Value, Some("L")),
            ("C15", Field::Value, Some("C")),
            ("C15", Field::Param("ESR".into()), Some("esr")),
            ("U3", Field::Param("fsw".into()), Some("fsw")),
        ]
    );
    assert_eq!(reads[0].cell_line, 2);
    let other = value_reads("x = my_component(\"R1\").value\n# component(\"R2\").value\n");
    assert!(other.is_empty(), "{other:?}");
}

fn policy() -> SandboxPolicy {
    SandboxPolicy::new("/work/buck-3v3").with_read_only("/opt/signex/python")
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

#[test]
fn the_sandbox_refuses_starting_a_program() {
    let events = [
        "subprocess.Popen",
        "os.system",
        "os.exec",
        "os.posix_spawn",
        "os.fork",
    ];
    for event in events {
        let refusal = policy()
            .check_event(event, &args(&["git", "log"]))
            .expect_err(event);
        assert_eq!(refusal.kind, RefusalKind::StartProgram, "{event}");
    }
    let refusal = policy()
        .check(&Access::StartProgram {
            call: "subprocess.run".into(),
        })
        .unwrap_err();
    assert_eq!(
        refusal.error_text(),
        "PermissionError: cells cannot start programs."
    );
    assert_eq!(
        refusal.blocked_sentence(),
        "subprocess.run is blocked in notebook cells."
    );
}

#[test]
fn the_sandbox_refuses_network_access() {
    let events = [
        "socket.connect",
        "socket.getaddrinfo",
        "urllib.Request",
        "http.client.connect",
    ];
    for event in events {
        let refusal = policy()
            .check_event(event, &args(&["example.org", "443"]))
            .expect_err(event);
        assert_eq!(refusal.kind, RefusalKind::Network, "{event}");
    }
}

#[test]
fn the_sandbox_refuses_files_outside_the_project() {
    let p = policy();
    let outside = [
        ("open", vec!["/etc/passwd", "r"]),
        ("open", vec!["../other/design.snxsch", "r"]),
        ("open", vec!["notebook/../../other.txt", "r"]),
        ("os.listdir", vec!["/home"]),
        ("os.remove", vec!["/work/buck-3v3-old/x"]),
        ("open", vec!["/opt/signex/python/lib/x.py", "w"]),
    ];
    for (event, values) in outside {
        let refusal = p.check_event(event, &args(&values)).expect_err(event);
        let kind = refusal.kind;
        assert_eq!(kind, RefusalKind::OutsideProject, "{event} {values:?}");
    }
    let inside = [
        ("open", vec!["notebook/attachments/ripple.csv", "r"]),
        ("open", vec!["/work/buck-3v3/out/ripple.csv", "w"]),
        ("open", vec!["/opt/signex/python/lib/x.py", "r"]),
        ("open", vec!["3", "r"]),
        ("import", vec!["numpy"]),
    ];
    for (event, values) in inside {
        let checked = p.check_event(event, &args(&values));
        assert_eq!(checked, Ok(()), "{event} {values:?}");
    }
}

#[test]
fn an_audit_event_is_classified_once() {
    let write = classify("open", &args(&["x.csv", "a"]));
    assert_eq!(
        write,
        Some(Access::File {
            call: "open".into(),
            path: "x.csv".into(),
            write: true
        })
    );
    assert_eq!(classify("import", &args(&["math"])), None);
    assert!(SANDBOX_RULE.contains("cannot start programs"));
}

#[test]
fn windows_paths_are_read_the_same_way() {
    let p = SandboxPolicy::new("C:\\Work\\buck-3v3");
    let inside = args(&["c:/Work/buck-3v3/notebook/x.csv", "r"]);
    assert_eq!(p.check_event("open", &inside), Ok(()));
    let outside = args(&["C:\\Work\\other\\x.csv", "r"]);
    assert!(p.check_event("open", &outside).is_err());
    assert!(is_inside("/a/b/c", "/a/b"));
    assert!(!is_inside("/a/bc", "/a/b"));
}

#[test]
fn a_notebook_from_outside_the_project_asks_once_per_notebook() {
    let mut answers = TrustAnswers::default();
    let project = "/work/buck-3v3";
    let own = "/work/buck-3v3/notebook/design.snxnbk";
    let shared = "/downloads/review/design.snxnbk";
    assert_eq!(answers.trust(own, project), Trust::Trusted);
    assert_eq!(answers.trust(shared, project), Trust::Unasked);
    answers.record(shared, false);
    assert_eq!(answers.trust(shared, project), Trust::Refused);
    answers.record(shared, true);
    assert_eq!(answers.trust(shared, project), Trust::Trusted);
    let kept: TrustAnswers =
        serde_json::from_value(serde_json::to_value(&answers).unwrap()).unwrap();
    assert_eq!(kept, answers, "kept with the preferences");
    answers.forget(shared);
    assert_eq!(answers.trust(shared, project), Trust::Unasked);
}

fn cached(source: &str) -> CachedCell {
    CachedCell {
        source_hash: source_hash(source),
        runtime: "Python 3.12".into(),
        inputs: vec![InputValue {
            designator: "C15".into(),
            field: Field::Value,
            symbol: Some("C".into()),
            written: Some("22µF".into()),
            quantity: Quantity::parse("22µF", Unit::Farad),
        }],
        output: RunOutput {
            stdout: "ΔV_out = 16.7 mV\n".into(),
            duration_ms: 212,
            ..RunOutput::default()
        },
        ran_at: "2026-11-24T14:02".into(),
    }
}

#[test]
fn the_cache_file_round_trips_with_its_token() {
    let mut cache = CellCache::default();
    cache.insert("ripple-vs-vin", cached("x = 1"));
    let bytes = cache.to_bytes();
    let text = String::from_utf8(bytes.clone()).unwrap();
    let token = format!("\"format\": \"{CELL_CACHE_FORMAT}\"");
    assert!(text.contains(&token), "{text}");
    let (read, note) = CellCache::read(&bytes);
    assert_eq!(note, None);
    assert_eq!(read, cache);
    assert_eq!(read.to_bytes(), bytes, "the same results, the same file");
}

#[test]
fn a_cache_file_this_build_cannot_read_is_set_aside() {
    let (cache, note) = CellCache::read(b"not json");
    assert!(cache.is_empty());
    assert!(matches!(note, Some(CacheNote::Unreadable { .. })));
    let (cache, note) = CellCache::read(br#"{"format":"notebook-cells/2","cells":{}}"#);
    assert!(cache.is_empty());
    assert_eq!(
        note,
        Some(CacheNote::OtherFormat {
            format: "notebook-cells/2".into()
        })
    );
}

#[test]
fn the_source_hash_changes_with_the_source() {
    assert_eq!(source_hash("x = 1"), source_hash("x = 1"));
    assert_ne!(source_hash("x = 1"), source_hash("x = 2"));
    assert_eq!(source_hash("").len(), 64);
}
