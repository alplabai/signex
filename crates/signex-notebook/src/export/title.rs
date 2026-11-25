//! The title block a notebook declares with
//! `#show: report.with(doc: "DN-0042", rev: "B", title: [...], date: "...")`:
//! what the export names the file after and dates the pages by.

use serde::Serialize;
use typst::syntax::ast::{self, AstNode};

use crate::date::IsoDate;
use crate::syntax::{typst_source, visit};

/// The title block of a notebook. Every field is optional: a notebook that
/// does not use the report layout has none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TitleBlock {
    /// The document number, `DN-0042`.
    pub doc: Option<String>,
    /// The revision, `B`.
    pub rev: Option<String>,
    /// The title as written (a content block's markup, trimmed).
    pub title: Option<String>,
    /// The revision date; the export's `datetime.today()` answers it.
    #[serde(serialize_with = "date_text")]
    pub date: Option<IsoDate>,
}

/// A date as `YYYY-MM-DD`, or null.
fn date_text<S: serde::Serializer>(
    date: &Option<IsoDate>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match date {
        Some(date) => serializer.serialize_str(&date.to_string()),
        None => serializer.serialize_none(),
    }
}

impl TitleBlock {
    /// The title block of the notebook file text `source`: the named
    /// arguments of its first `report.with(...)` or `report(...)` call.
    /// An argument that is not a literal is left out.
    pub fn of(source: &str) -> Self {
        let text = typst_source(source);
        let mut found: Option<TitleBlock> = None;
        visit(&typst::syntax::parse(&text), &mut |node| {
            if found.is_some() {
                return;
            }
            if let Some(call) = node.cast::<ast::FuncCall>()
                && is_report(call.callee())
            {
                found = Some(Self::from_args(call.args()));
            }
        });
        found.unwrap_or_default()
    }

    fn from_args(args: ast::Args<'_>) -> Self {
        let mut block = Self::default();
        for arg in args.items() {
            let ast::Arg::Named(named) = arg else {
                continue;
            };
            let value = literal_text(named.expr());
            match named.name().as_str() {
                "doc" => block.doc = value,
                "rev" => block.rev = value,
                "title" => block.title = value,
                "date" => block.date = value.as_deref().and_then(IsoDate::parse),
                _ => {}
            }
        }
        block
    }
}

/// `report.with` or `report`.
fn is_report(callee: ast::Expr<'_>) -> bool {
    match callee {
        ast::Expr::Ident(ident) => ident.as_str() == "report",
        ast::Expr::FieldAccess(access) => {
            access.field().as_str() == "with"
                && matches!(access.target(), ast::Expr::Ident(ident) if ident.as_str() == "report")
        }
        _ => false,
    }
}

/// A string literal's text, a number's digits, or a content block's
/// markup; `None` for anything computed or empty.
fn literal_text(expr: ast::Expr<'_>) -> Option<String> {
    let text = match expr {
        ast::Expr::Str(text) => text.get().to_string(),
        ast::Expr::Int(number) => number.get().to_string(),
        ast::Expr::Content(block) => block.body().to_untyped().clone().into_text().to_string(),
        _ => return None,
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}
