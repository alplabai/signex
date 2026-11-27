//! The two templates of the notebook tab's empty state (mockup row 195):
//! a design report with a title block, contents and fixed sections, and a
//! lab notebook of dated entries with no fixed sections. A template is a
//! new notebook in memory; it is written only on Save.

use crate::date::IsoDate;
use crate::file::NotebookFile;
use crate::reference::quote;
use crate::world::PACKAGE_IMPORT;

/// A notebook to start from; the design report is the card picked first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Template {
    /// Title block, contents, Introduction, Requirements, Design, Bring-up,
    /// Decisions and open issues, Change log.
    #[default]
    DesignReport,
    /// Dated entries, measurements and attachments, no fixed sections.
    LabNotebook,
}

impl Template {
    /// Both templates, in the order the empty state shows them.
    pub const ALL: [Template; 2] = [Template::DesignReport, Template::LabNotebook];

    /// The token a command names the template by.
    pub fn token(self) -> &'static str {
        match self {
            Template::DesignReport => "design_report",
            Template::LabNotebook => "lab_notebook",
        }
    }

    /// The template `token` names.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|template| template.token() == token)
    }

    /// The Typst body of a new notebook of project `project`, its first
    /// entry dated `today`.
    pub fn body(self, project: &str, today: IsoDate) -> String {
        let title = quote(&format!("{project} Design Notebook"));
        let mut body = format!("{PACKAGE_IMPORT}\n");
        match self {
            Template::DesignReport => {
                body.push_str(&format!(
                    "#show: report.with(title: {title}, rev: \"A\")\n\n\
                     #outline()\n\n\
                     = Introduction <sec:intro>\n\n\
                     == Scope\n\n\
                     == Requirements <sec:requirements>\n\n\
                     = Design <sec:design>\n\n\
                     = Bring-up <sec:bringup>\n\n\
                     = Decisions and open issues <sec:decisions>\n\n\
                     = Change log <sec:changes>\n\n\
                     - Rev A, {today}: first issue.\n"
                ));
            }
            Template::LabNotebook => {
                body.push_str(&format!(
                    "#show: report.with(title: {title})\n\n\
                     = {today}\n\n"
                ));
            }
        }
        body
    }

    /// A new notebook file of this template, for project `project`.
    pub fn file(self, project: &str, today: IsoDate) -> NotebookFile {
        NotebookFile::new(self.body(project, today))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::outline_of_text;
    use crate::{CompileOptions, NotebookSources, compile};

    fn today() -> IsoDate {
        IsoDate::new(2026, 11, 25).expect("a date")
    }

    #[test]
    fn the_design_report_has_its_fixed_sections_in_order() {
        let body = Template::DesignReport.body("buck-3v3", today());
        let titles: Vec<String> = outline_of_text("design.snxnbk", &body)
            .sections
            .into_iter()
            .map(|s| format!("{} {}", s.number, s.title))
            .collect();
        assert_eq!(
            titles,
            [
                "1 Introduction",
                "1.1 Scope",
                "1.2 Requirements",
                "2 Design",
                "3 Bring-up",
                "4 Decisions and open issues",
                "5 Change log",
            ]
        );
        assert!(body.contains("buck-3v3 Design Notebook"), "{body}");
        assert!(body.contains("2026-11-25"), "{body}");
    }

    #[test]
    fn the_lab_notebook_opens_with_an_entry_for_today_and_no_fixed_sections() {
        let body = Template::LabNotebook.body("buck-3v3", today());
        let sections = outline_of_text("design.snxnbk", &body).sections;
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "2026-11-25");
    }

    #[test]
    fn both_templates_compile_and_write_a_header_with_the_format_token() {
        for template in Template::ALL {
            let file = template.file("buck-3v3", today());
            let text = file.write().expect("a new notebook writes");
            assert!(text.starts_with("+++\n"), "{text}");
            assert!(text.contains("snxnbk/1"), "{text}");
            let compiled = compile(
                &NotebookSources::new("design.snxnbk", text),
                &CompileOptions::default(),
            );
            assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
        }
    }

    #[test]
    fn a_project_name_with_quotes_stays_one_string() {
        let body = Template::DesignReport.body("a \"b\"", today());
        assert!(
            body.contains(r#"title: "a \"b\" Design Notebook""#),
            "{body}"
        );
    }

    #[test]
    fn every_token_names_its_template() {
        for template in Template::ALL {
            assert_eq!(Template::parse(template.token()), Some(template));
        }
        assert_eq!(Template::parse("report"), None);
    }
}
