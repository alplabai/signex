//! What the PDF's bookmark panel lists (mockup row 202, #2611): a bookmark
//! per section and subsection heading, and per figure and table with a
//! caption, each opening the page it is on. Typst writes the bookmarks;
//! this counts them from the source for the export's answer.

use serde::Serialize;
use typst::syntax::ast;

use crate::sources::NotebookSources;
use crate::syntax::visit;

/// How many bookmarks of each kind the PDF carries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BookmarkCounts {
    /// Level 1 headings (`= Power stage`).
    pub sections: usize,
    /// Deeper headings (`== Inductor`).
    pub subsections: usize,
    /// Captioned figures that are not tables.
    pub figures: usize,
    /// Captioned figures of a table.
    pub tables: usize,
}

impl BookmarkCounts {
    /// Every bookmark, of every kind.
    pub fn total(&self) -> usize {
        self.sections + self.subsections + self.figures + self.tables
    }

    /// The bookmarks of every Typst file of `sources` (the notebook and its
    /// chapters). A chapter that is held but never included still counts:
    /// the caller loads only what the notebook includes.
    pub fn of(sources: &NotebookSources) -> Self {
        let mut counts = Self::default();
        for path in sources.paths() {
            let is_typst = path.ends_with(".snxnbk") || path.ends_with(".typ");
            let Some(bytes) = sources.get(path).filter(|_| is_typst) else {
                continue;
            };
            counts.add_source(&String::from_utf8_lossy(bytes));
        }
        counts
    }

    /// Count the bookmarks of one file text.
    pub fn add_source(&mut self, source: &str) {
        let text = crate::syntax::typst_source(source);
        visit(&typst::syntax::parse(&text), &mut |node| {
            if let Some(heading) = node.cast::<ast::Heading>() {
                self.add_heading(heading.depth().get());
            } else if let Some(call) = node.cast::<ast::FuncCall>() {
                self.add_call(call);
            }
        });
    }

    fn add_heading(&mut self, level: usize) {
        if level <= 1 {
            self.sections += 1;
        } else {
            self.subsections += 1;
        }
    }

    fn add_call(&mut self, call: ast::FuncCall<'_>) {
        let ast::Expr::Ident(name) = call.callee() else {
            return;
        };
        match name.as_str() {
            "heading" => {
                let level = named(call, "level")
                    .and_then(|expr| match expr {
                        ast::Expr::Int(level) => usize::try_from(level.get()).ok(),
                        _ => None,
                    })
                    .unwrap_or(1);
                self.add_heading(level);
            }
            "figure" if named(call, "caption").is_some() => {
                if is_table_figure(call) {
                    self.tables += 1;
                } else {
                    self.figures += 1;
                }
            }
            _ => {}
        }
    }
}

/// The named argument `name` of `call`.
fn named<'a>(call: ast::FuncCall<'a>, name: &str) -> Option<ast::Expr<'a>> {
    call.args().items().find_map(|arg| match arg {
        ast::Arg::Named(named) if named.name().as_str() == name => Some(named.expr()),
        _ => None,
    })
}

/// A figure whose body is a `table(...)` call, or whose `kind` is `table`.
fn is_table_figure(call: ast::FuncCall<'_>) -> bool {
    if let Some(ast::Expr::Ident(kind)) = named(call, "kind") {
        return kind.as_str() == "table";
    }
    call.args().items().any(|arg| match arg {
        ast::Arg::Pos(ast::Expr::FuncCall(body)) => {
            matches!(body.callee(), ast::Expr::Ident(name) if name.as_str() == "table")
        }
        _ => false,
    })
}
