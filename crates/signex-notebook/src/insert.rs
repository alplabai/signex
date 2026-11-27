//! What the notebook tab's insert toolbar writes (mockup row 195): a
//! reference call, an annotation block with the next free id of its kind,
//! or a figure. The text is plain Typst the person then fills in; nothing
//! here knows the cursor beyond the section it is in.

use crate::annotation::AnnotationKind;
use crate::id::BlockId;
use crate::outline::Outline;
use crate::reference::Reference;
use crate::template::next_block_id;

/// What to insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Insertion {
    /// A part, pin or net, as the markup call `#component("R7")`.
    Reference(Reference),
    /// An annotation block of this kind.
    Block(AnnotationKind),
    /// A figure with an image and a caption.
    Figure,
}

impl Insertion {
    /// The insertion a command names: `calculation`, `measurement`,
    /// `note`, `issue`, `decision` or `figure`. A reference is named by its
    /// call, not by a token.
    pub fn parse(token: &str) -> Option<Self> {
        if token == "figure" {
            return Some(Self::Figure);
        }
        AnnotationKind::parse(token).map(Self::Block)
    }

    /// The tokens [`Self::parse`] reads, in toolbar order.
    pub const TOKENS: [&'static str; 6] = [
        "calculation",
        "measurement",
        "figure",
        "note",
        "issue",
        "decision",
    ];
}

/// The text to insert and the block id it gives, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inserted {
    pub text: String,
    pub id: Option<BlockId>,
}

/// Whether ids of `kind` are numbered within their top-level section
/// (`CALC-2.1`, mockup row 195) or straight through (`MEAS-009`,
/// `ISS-004`, `DEC-002`, `NOTE-012`).
pub fn numbers_within_section(kind: AnnotationKind) -> bool {
    kind == AnnotationKind::Calculation
}

/// The text `insertion` writes at `line` of `file`, numbered against the
/// blocks `outline` already lists.
pub fn insertion_text(insertion: &Insertion, outline: &Outline, file: &str, line: u32) -> Inserted {
    match insertion {
        Insertion::Reference(reference) => Inserted {
            text: reference.to_markup(),
            id: None,
        },
        Insertion::Figure => Inserted {
            text: "#figure(\n  image(\"img/\"),\n  caption: [],\n)\n".to_string(),
            id: None,
        },
        Insertion::Block(kind) => {
            let section = section_of(*kind, outline, file, line);
            let id = next_block_id(*kind, &section, outline.block_ids());
            Inserted {
                text: block_text(*kind, &id),
                id: Some(id),
            }
        }
    }
}

/// The section a block of `kind` written at `line` of `file` is numbered
/// in: its top-level section for a Calculation, none for the others.
fn section_of(kind: AnnotationKind, outline: &Outline, file: &str, line: u32) -> Vec<u64> {
    if !numbers_within_section(kind) {
        return Vec::new();
    }
    outline
        .section_at(file, line)
        .and_then(|section| section.number.split('.').next())
        .and_then(|top| top.parse::<u64>().ok())
        .into_iter()
        .collect()
}

/// A block of `kind` with `id`, its fields left to fill in.
fn block_text(kind: AnnotationKind, id: &BlockId) -> String {
    let function = kind.function();
    let fields = match kind {
        AnnotationKind::Note => String::new(),
        AnnotationKind::Calculation | AnnotationKind::Measurement => ", title: []".to_string(),
        AnnotationKind::Issue => ", title: [], status: \"open\"".to_string(),
        AnnotationKind::Decision => ", title: [], status: \"proposed\"".to_string(),
    };
    format!("#{function}(id: \"{id}\"{fields})[\n  \n]\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::outline_of_text;

    const SOURCE: &str = "= Introduction\n\
        #measurement(id: \"MEAS-009\", title: [Ripple])[x]\n\
        = Power stage\n\
        #calculation(id: \"CALC-2.1\", title: [Output voltage])[x]\n\
        \n\
        #issue(id: \"ISS-004\", title: [Worst case])[x]\n";

    fn at(insertion: Insertion, line: u32) -> Inserted {
        insertion_text(
            &insertion,
            &outline_of_text("design.snxnbk", SOURCE),
            "design.snxnbk",
            line,
        )
    }

    #[test]
    fn a_reference_is_inserted_as_its_markup_call() {
        let pin = Insertion::Reference(Reference::Pin {
            component: "U3".into(),
            pin: "12".into(),
        });
        assert_eq!(at(pin, 5).text, "#pin(\"U3\", \"12\")");
        let net = Insertion::Reference(Reference::Net("+3V3".into()));
        assert_eq!(
            at(net, 5),
            Inserted {
                text: "#net(\"+3V3\")".into(),
                id: None,
            }
        );
    }

    #[test]
    fn a_calculation_takes_the_next_number_of_its_section() {
        let inserted = at(Insertion::Block(AnnotationKind::Calculation), 5);
        assert_eq!(
            inserted.id.map(|id| id.to_string()).as_deref(),
            Some("CALC-2.2")
        );
        assert!(
            inserted
                .text
                .starts_with("#calculation(id: \"CALC-2.2\", title: [])["),
            "{}",
            inserted.text
        );
        let first = at(Insertion::Block(AnnotationKind::Calculation), 2);
        assert_eq!(
            first.id.map(|id| id.to_string()).as_deref(),
            Some("CALC-1.1")
        );
    }

    #[test]
    fn other_blocks_take_the_next_flat_number_of_their_kind() {
        let ids: Vec<String> = [
            AnnotationKind::Measurement,
            AnnotationKind::Issue,
            AnnotationKind::Decision,
            AnnotationKind::Note,
        ]
        .into_iter()
        .filter_map(|kind| at(Insertion::Block(kind), 5).id)
        .map(|id| id.to_string())
        .collect();
        assert_eq!(ids, ["MEAS-010", "ISS-005", "DEC-001", "NOTE-001"]);
    }

    #[test]
    fn an_issue_and_a_decision_start_with_their_first_status() {
        let issue = at(Insertion::Block(AnnotationKind::Issue), 5).text;
        assert!(issue.contains("status: \"open\""), "{issue}");
        let decision = at(Insertion::Block(AnnotationKind::Decision), 5).text;
        assert!(decision.contains("status: \"proposed\""), "{decision}");
        let note = at(Insertion::Block(AnnotationKind::Note), 5).text;
        assert_eq!(note, "#note(id: \"NOTE-001\")[\n  \n]\n");
    }

    #[test]
    fn a_figure_has_an_image_and_a_caption() {
        let figure = at(Insertion::Figure, 5).text;
        assert!(figure.starts_with("#figure("), "{figure}");
        assert!(figure.contains("caption: []"), "{figure}");
    }

    #[test]
    fn every_toolbar_token_parses_and_nothing_else_does() {
        for token in Insertion::TOKENS {
            assert!(Insertion::parse(token).is_some(), "{token}");
        }
        assert_eq!(Insertion::parse("figure"), Some(Insertion::Figure));
        assert_eq!(Insertion::parse("component"), None);
        assert_eq!(Insertion::parse("Calculation"), None);
    }
}
