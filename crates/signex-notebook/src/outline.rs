//! The outline of a notebook (mockup row 195, "Outline"): its numbered
//! sections, its figures and tables, and its annotation blocks, in reading
//! order across the notebook file and the chapters it includes, each with
//! the file and line it starts on. Read from the Typst source, so the
//! outline is there before (and while) the notebook compiles.

use typst::syntax::ast::{self, AstNode};

use crate::annotation::AnnotationKind;
use crate::id::BlockId;
use crate::sources::{NotebookSources, resolve};
use crate::syntax::{line_of, typst_source, walk};

/// The deepest include chain the outline follows; a deeper chain is a loop
/// or a mistake, and the compile reports it.
const MAX_INCLUDE_DEPTH: usize = 16;

/// Where an outline entry starts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SourceAt {
    /// The file, as a path inside the notebook folder.
    pub file: String,
    /// The 1-based line in that file.
    pub line: u32,
}

/// One heading.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Section {
    /// `2.1`: one number per level, as the report numbers headings.
    pub number: String,
    /// 1 for `=`, 2 for `==`, ...
    pub depth: u8,
    pub title: String,
    /// The `<sec:power>` label after the title, without the brackets.
    pub label: Option<String>,
    pub at: SourceAt,
}

/// Whether a figure holds a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureKind {
    Figure,
    Table,
}

/// One `#figure(..)`, numbered within its kind as Typst numbers it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Figure {
    pub kind: FigureKind,
    pub number: u32,
    /// The `caption:` text; empty when there is none.
    pub caption: String,
    pub at: SourceAt,
}

/// One annotation block.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BlockEntry {
    /// The kind's token: `note`, `calculation`, ...
    pub kind: &'static str,
    /// The typed id, when the block has one that reads.
    pub id: Option<BlockId>,
    /// The `title:` text; empty when there is none.
    pub title: String,
    pub at: SourceAt,
}

/// The outline of a notebook.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Outline {
    pub sections: Vec<Section>,
    pub figures: Vec<Figure>,
    pub blocks: Vec<BlockEntry>,
}

impl Outline {
    /// True when the notebook has no section, figure or block.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.figures.is_empty() && self.blocks.is_empty()
    }

    /// The section `line` of `file` sits in: the last heading of that file
    /// at or before the line. Only the file's own headings count.
    pub fn section_at(&self, file: &str, line: u32) -> Option<&Section> {
        self.sections
            .iter()
            .rev()
            .find(|section| section.at.file == file && section.at.line <= line)
    }

    /// Every block id the notebook already uses, for the next free one.
    pub fn block_ids(&self) -> impl Iterator<Item = &BlockId> {
        self.blocks.iter().filter_map(|block| block.id.as_ref())
    }
}

/// What one file says, in source order, before numbering.
enum Item {
    Heading {
        depth: u8,
        title: String,
        label: Option<String>,
        line: u32,
    },
    Figure {
        kind: FigureKind,
        caption: String,
        line: u32,
    },
    Block {
        kind: AnnotationKind,
        id: Option<BlockId>,
        title: String,
        line: u32,
    },
    Include {
        path: String,
    },
}

/// The outline of the notebook in `sources`, its chapters spliced in where
/// they are included. A chapter the sources do not hold is left out.
pub fn outline(sources: &NotebookSources) -> Outline {
    let mut numbering = Numbering::default();
    let mut out = Outline::default();
    let mut chain = Vec::new();
    add_file(
        sources,
        sources.main(),
        &mut chain,
        &mut numbering,
        &mut out,
    );
    out
}

/// The outline of one file text alone, includes not followed.
pub fn outline_of_text(file: &str, text: &str) -> Outline {
    outline(&NotebookSources::new(file, text))
}

fn add_file(
    sources: &NotebookSources,
    file: &str,
    chain: &mut Vec<String>,
    numbering: &mut Numbering,
    out: &mut Outline,
) {
    if chain.len() >= MAX_INCLUDE_DEPTH || chain.iter().any(|held| held == file) {
        return;
    }
    let Some(bytes) = sources.get(file) else {
        return;
    };
    chain.push(file.to_string());
    let text = String::from_utf8_lossy(bytes);
    for item in items(&text) {
        let at = |line| SourceAt {
            file: file.to_string(),
            line,
        };
        match item {
            Item::Heading {
                depth,
                title,
                label,
                line,
            } => out.sections.push(Section {
                number: numbering.heading(depth),
                depth,
                title,
                label,
                at: at(line),
            }),
            Item::Figure {
                kind,
                caption,
                line,
            } => out.figures.push(Figure {
                kind,
                number: numbering.figure(kind),
                caption,
                at: at(line),
            }),
            Item::Block {
                kind,
                id,
                title,
                line,
            } => out.blocks.push(BlockEntry {
                kind: kind.token(),
                id,
                title,
                at: at(line),
            }),
            Item::Include { path } => {
                if let Some(target) = resolve(file, &path) {
                    add_file(sources, &target, chain, numbering, out);
                }
            }
        }
    }
    chain.pop();
}

/// The heading and figure counters, carried across files.
#[derive(Default)]
struct Numbering {
    levels: Vec<u32>,
    figures: u32,
    tables: u32,
}

impl Numbering {
    fn heading(&mut self, depth: u8) -> String {
        let depth = usize::from(depth.max(1));
        self.levels.resize(depth, 0);
        if let Some(last) = self.levels.last_mut() {
            *last += 1;
        }
        let parts: Vec<String> = self.levels.iter().map(u32::to_string).collect();
        parts.join(".")
    }

    fn figure(&mut self, kind: FigureKind) -> u32 {
        let counter = match kind {
            FigureKind::Figure => &mut self.figures,
            FigureKind::Table => &mut self.tables,
        };
        *counter += 1;
        *counter
    }
}

/// The items of the file text `source`, in source order.
fn items(source: &str) -> Vec<Item> {
    let text = typst_source(source);
    let root = typst::syntax::parse(&text);
    let mut found = Vec::new();
    walk(&root, 0, &mut |node, offset| {
        let line = line_of(&text, offset);
        if let Some(heading) = node.cast::<ast::Heading>() {
            let depth = u8::try_from(heading.depth().get()).unwrap_or(u8::MAX);
            let (title, label) = split_label(&node_text(heading.body().to_untyped()));
            found.push(Item::Heading {
                depth,
                title,
                label,
                line,
            });
        } else if let Some(include) = node.cast::<ast::ModuleInclude>() {
            if let ast::Expr::Str(path) = include.source() {
                found.push(Item::Include {
                    path: path.get().to_string(),
                });
            }
        } else if let Some(call) = node.cast::<ast::FuncCall>()
            && let Some(item) = call_item(call, line)
        {
            found.push(item);
        }
    });
    found
}

/// A figure or an annotation block, from its call.
fn call_item(call: ast::FuncCall<'_>, line: u32) -> Option<Item> {
    let ast::Expr::Ident(name) = call.callee() else {
        return None;
    };
    let name = name.as_str();
    if name == "figure" {
        return Some(Item::Figure {
            kind: figure_kind(call),
            caption: named_text(call, "caption").unwrap_or_default(),
            line,
        });
    }
    // The earlier one-function form names its kind with `kind:`.
    let kind = match name {
        "annotation" => AnnotationKind::parse(&named_text(call, "kind")?)?,
        other => AnnotationKind::from_function(other)?,
    };
    Some(Item::Block {
        kind,
        id: named_text(call, "id").and_then(|id| BlockId::parse_for(kind, &id)),
        title: named_text(call, "title").unwrap_or_default(),
        line,
    })
}

/// A figure whose body is a `table(..)` call, or that says `kind: table`.
fn figure_kind(call: ast::FuncCall<'_>) -> FigureKind {
    let is_table = call.args().items().any(|arg| match arg {
        ast::Arg::Pos(ast::Expr::FuncCall(inner)) => {
            matches!(inner.callee(), ast::Expr::Ident(name) if name.as_str() == "table")
        }
        ast::Arg::Named(named) => {
            named.name().as_str() == "kind"
                && matches!(named.expr(), ast::Expr::Ident(name) if name.as_str() == "table")
        }
        _ => false,
    });
    if is_table {
        FigureKind::Table
    } else {
        FigureKind::Figure
    }
}

/// The text of the named argument `name`: a string's value, or a content
/// block's source without its brackets, trimmed.
fn named_text(call: ast::FuncCall<'_>, name: &str) -> Option<String> {
    call.args().items().find_map(|arg| match arg {
        ast::Arg::Named(named) if named.name().as_str() == name => Some(match named.expr() {
            ast::Expr::Str(text) => text.get().to_string(),
            ast::Expr::Content(block) => node_text(block.body().to_untyped()),
            other => node_text(other.to_untyped()),
        }),
        _ => None,
    })
}

/// The source text of `node`, trimmed, on one line.
fn node_text(node: &typst::syntax::SyntaxNode) -> String {
    let text = node.clone().into_text();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `Power stage <sec:power>` -> (`Power stage`, `sec:power`).
fn split_label(title: &str) -> (String, Option<String>) {
    let trimmed = title.trim();
    if let Some(open) = trimmed.rfind('<')
        && trimmed.ends_with('>')
    {
        let label = &trimmed[open + 1..trimmed.len() - 1];
        let is_label = !label.is_empty()
            && label
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'));
        if is_label {
            return (
                trimmed[..open].trim_end().to_string(),
                Some(label.to_string()),
            );
        }
    }
    (trimmed.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "#import \"@signex/notebook:0.1.0\": *\n\
        = Introduction <sec:intro>\n\
        == Scope\n\
        #figure(image(\"img/path.png\"), caption: [Power path])\n\
        = Power stage\n\
        == Feedback network <sec:fb>\n\
        #calculation(id: \"CALC-2.1\", title: [Output voltage])[ $ x $ ]\n\
        #figure(table(columns: 2)[a][b], caption: [Inductor candidates])\n\
        #issue(id: \"ISS-004\", title: [Worst-case output], status: \"open\")[x]\n\
        #annotation(kind: \"decision\", title: \"Old form\")[y]\n";

    #[test]
    fn sections_are_numbered_by_level_with_their_labels_and_lines() {
        let outline = outline_of_text("design.snxnbk", REPORT);
        let rows: Vec<(&str, u8, &str, Option<&str>, u32)> = outline
            .sections
            .iter()
            .map(|s| {
                (
                    s.number.as_str(),
                    s.depth,
                    s.title.as_str(),
                    s.label.as_deref(),
                    s.at.line,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("1", 1, "Introduction", Some("sec:intro"), 2),
                ("1.1", 2, "Scope", None, 3),
                ("2", 1, "Power stage", None, 5),
                ("2.1", 2, "Feedback network", Some("sec:fb"), 6),
            ]
        );
    }

    #[test]
    fn figures_and_tables_are_numbered_apart_with_their_captions() {
        let outline = outline_of_text("design.snxnbk", REPORT);
        let rows: Vec<(FigureKind, u32, &str)> = outline
            .figures
            .iter()
            .map(|f| (f.kind, f.number, f.caption.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (FigureKind::Figure, 1, "Power path"),
                (FigureKind::Table, 1, "Inductor candidates"),
            ]
        );
    }

    #[test]
    fn blocks_of_both_forms_are_listed_with_their_ids_and_titles() {
        let outline = outline_of_text("design.snxnbk", REPORT);
        let rows: Vec<(&str, Option<String>, &str)> = outline
            .blocks
            .iter()
            .map(|b| {
                (
                    b.kind,
                    b.id.as_ref().map(ToString::to_string),
                    b.title.as_str(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("calculation", Some("CALC-2.1".into()), "Output voltage"),
                ("issue", Some("ISS-004".into()), "Worst-case output"),
                ("decision", None, "Old form"),
            ]
        );
        assert_eq!(outline.block_ids().count(), 2);
    }

    #[test]
    fn a_chapter_is_spliced_in_where_it_is_included_and_numbering_carries_on() {
        let mut sources = NotebookSources::new(
            "design.snxnbk",
            "= Introduction\n#include \"chapters/power.snxnbk\"\n= Change log\n",
        );
        assert!(sources.insert("chapters/power.snxnbk", "= Power stage\n== Ripple\n"));
        let outline = outline(&sources);
        let rows: Vec<(&str, &str, &str, u32)> = outline
            .sections
            .iter()
            .map(|s| {
                (
                    s.number.as_str(),
                    s.title.as_str(),
                    s.at.file.as_str(),
                    s.at.line,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("1", "Introduction", "design.snxnbk", 1),
                ("2", "Power stage", "chapters/power.snxnbk", 1),
                ("2.1", "Ripple", "chapters/power.snxnbk", 2),
                ("3", "Change log", "design.snxnbk", 3),
            ]
        );
    }

    #[test]
    fn a_missing_or_looping_chapter_is_left_out() {
        let mut sources = NotebookSources::new(
            "design.snxnbk",
            "= A\n#include \"missing.snxnbk\"\n#include \"loop.snxnbk\"\n",
        );
        assert!(sources.insert("loop.snxnbk", "= B\n#include \"loop.snxnbk\"\n"));
        let titles: Vec<String> = outline(&sources)
            .sections
            .into_iter()
            .map(|s| s.title)
            .collect();
        assert_eq!(titles, ["A", "B"]);
    }

    #[test]
    fn header_lines_keep_their_numbers() {
        let text = "+++\nformat = \"snxnbk/1\"\n+++\n= Notes\n";
        let outline = outline_of_text("design.snxnbk", text);
        assert_eq!(outline.sections[0].at.line, 4);
    }

    #[test]
    fn the_section_of_a_line_is_the_last_heading_above_it_in_that_file() {
        let outline = outline_of_text("design.snxnbk", REPORT);
        let at = |line| {
            outline
                .section_at("design.snxnbk", line)
                .map(|s| s.number.as_str())
        };
        assert_eq!(at(1), None);
        assert_eq!(at(4), Some("1.1"));
        assert_eq!(at(7), Some("2.1"));
        assert_eq!(outline.section_at("other.snxnbk", 7), None);
    }

    #[test]
    fn an_empty_notebook_has_an_empty_outline() {
        assert!(outline_of_text("design.snxnbk", "Some text.\n").is_empty());
        assert!(!outline_of_text("design.snxnbk", REPORT).is_empty());
    }

    #[test]
    fn a_trailing_label_is_split_from_the_title() {
        assert_eq!(
            split_label("Power stage <sec:power>"),
            ("Power stage".to_string(), Some("sec:power".to_string()))
        );
        assert_eq!(split_label("a < b"), ("a < b".to_string(), None));
        assert_eq!(split_label("x <>"), ("x <>".to_string(), None));
    }
}
