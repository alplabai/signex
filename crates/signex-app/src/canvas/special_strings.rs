//! The values `${TOKEN}` special strings resolve to on one sheet (#1080),
//! kept by the canvas so a text item shows what the PDF and SVG print for it.
//! Resolution is `signex_output::resolve`, the resolver the title block uses;
//! names that are no built-in token are the sheet's and the project's
//! variables (#1987).

use signex_output::{PdfOptions, ProjectMetadata, SubstitutionContext};
use signex_types::schematic::Symbol;

/// An owned [`SubstitutionContext`]: what the canvas holds between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecialStrings {
    /// The sheet's metadata, its variables included
    /// (`ProjectMetadata::sheet_variables`).
    pub metadata: ProjectMetadata,
    pub filename: String,
    pub sheet_name: String,
    /// `None` when the sheet has no number in its project (a hierarchical
    /// child, say): it prints none rather than another sheet's.
    pub sheet_number: Option<usize>,
    pub sheet_count: usize,
}

impl SpecialStrings {
    /// `text` with its `${TOKEN}`s replaced (title-block template text).
    pub fn resolve(&self, text: &str) -> String {
        signex_output::resolve(text, &self.context())
    }

    /// A text note's drawn text: an `=` expression evaluated, else its
    /// `${TOKEN}`s replaced (#1987). The stored text never changes.
    pub fn resolve_text(&self, text: &str) -> String {
        signex_output::resolve_text(text, &self.context())
    }

    /// A symbol value field's drawn text (#1596): its `${TOKEN}`s replaced,
    /// except on a power port, whose value is a net name and is drawn as
    /// stored.
    pub fn resolve_field(&self, text: &str, is_power: bool) -> String {
        if is_power {
            text.to_string()
        } else {
            self.resolve(text)
        }
    }

    /// A symbol's drawn value: an `=` expression evaluated through the parts
    /// of its sheet (`symbols`) and the variables, with its unit (#1986),
    /// else [`Self::resolve_field`].
    pub fn resolve_value(&self, symbols: &[Symbol], symbol: &Symbol) -> String {
        let context = self.context();
        let evaluated = (!symbol.is_power)
            .then(|| signex_output::resolve_part_field(symbols, symbol, "Value", Some(&context)))
            .flatten();
        evaluated.unwrap_or_else(|| self.resolve_field(&symbol.value, symbol.is_power))
    }

    fn context(&self) -> SubstitutionContext<'_> {
        let defaults = PdfOptions::default();
        SubstitutionContext {
            metadata: &self.metadata,
            filename: self.filename.clone(),
            sheet_name: self.sheet_name.clone(),
            sheet_number: self.sheet_number.unwrap_or(0),
            sheet_count: self.sheet_count,
            signex_version: env!("CARGO_PKG_VERSION"),
            variant: defaults.variant,
            physical_structure: defaults.use_physical_structure,
            physical_sheet_number: defaults.physical_sheet_number && self.sheet_number.is_some(),
            physical_document_number: defaults.physical_document_number,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn special() -> SpecialStrings {
        let params = BTreeMap::from([
            ("title".to_string(), "Supply".to_string()),
            ("VCC".to_string(), "5V".to_string()),
            ("VREF".to_string(), "= VCC / 2".to_string()),
        ]);
        let mut metadata = ProjectMetadata::from_values(&params);
        metadata.project_parameters = Some(params);
        metadata
            .sheet_variables
            .insert("VCC".to_string(), "3.3V".to_string());
        SpecialStrings {
            metadata,
            filename: "power.snxsch".to_string(),
            sheet_name: "power".to_string(),
            sheet_number: Some(1),
            sheet_count: 1,
        }
    }

    /// #1596: a value field draws its tokens resolved; a power port's value
    /// is a net name and draws as stored.
    #[test]
    fn a_value_field_resolves_except_on_a_power_port() {
        let s = special();
        assert_eq!(s.resolve_field("${TITLE} ${VREF}", false), "Supply 1.65V");
        assert_eq!(s.resolve_field("${VCC}", true), "${VCC}");
    }

    /// #1987: a text note that is an `=` expression draws its value.
    #[test]
    fn a_text_note_expression_draws_its_value() {
        let s = special();
        assert_eq!(s.resolve_text("= VCC * 2"), "6.6V");
        assert_eq!(s.resolve_text("VCC is ${VCC}"), "VCC is 3.3V");
    }

    /// #1986: an `=` value draws its value with its unit, through another
    /// part and the sheet's variables; a power port's value is never
    /// evaluated.
    #[test]
    fn a_value_expression_draws_its_value() {
        let s = special();
        let r1 = Symbol {
            reference: "R1".to_string(),
            value: "= VCC / 10mA".to_string(),
            ..Default::default()
        };
        let r2 = Symbol {
            reference: "R2".to_string(),
            value: "= R1.Value * 2".to_string(),
            ..Default::default()
        };
        let power = Symbol {
            reference: "#PWR1".to_string(),
            value: "= VCC".to_string(),
            is_power: true,
            ..Default::default()
        };
        let symbols = vec![r1, r2, power];
        assert_eq!(s.resolve_value(&symbols, &symbols[0]), "330Ω");
        assert_eq!(s.resolve_value(&symbols, &symbols[1]), "660Ω");
        assert_eq!(s.resolve_value(&symbols, &symbols[2]), "= VCC");
    }
}
