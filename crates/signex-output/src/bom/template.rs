//! A BOM template's columns as the exporter lists them (#1988):
//! `signex_types::bom_template` holds the file, this maps its columns to
//! [`BomColumn`] and back. See `docs/BOM_COLUMNS.md`.

use signex_bom::ComputedColumn;
use signex_types::bom_template::{BomTemplate, TemplateColumn};

use super::BomColumn;

/// The standard columns by their template key, in the order the keys are
/// documented.
const STANDARD: [(&str, BomColumn); 15] = [
    ("name", BomColumn::Name),
    ("designator", BomColumn::Designator),
    ("lib_ref", BomColumn::LibRef),
    ("reference", BomColumn::Reference),
    ("qty", BomColumn::Qty),
    ("value", BomColumn::Value),
    ("footprint", BomColumn::Footprint),
    ("description", BomColumn::Description),
    ("unit_price", BomColumn::UnitPrice),
    ("buy_qty", BomColumn::BuyQty),
    ("extended_price", BomColumn::ExtendedPrice),
    ("currency", BomColumn::Currency),
    ("stock", BomColumn::Stock),
    ("distributor", BomColumn::Distributor),
    ("price_age", BomColumn::PriceAge),
];

impl BomColumn {
    /// The key of a standard column in a template; `None` for a field or a
    /// computed column.
    pub fn template_key(&self) -> Option<&'static str> {
        STANDARD
            .iter()
            .find(|(_, column)| column == self)
            .map(|(key, _)| *key)
    }
}

/// The columns `template` lists. Refused, naming the column (from 1), for a
/// standard key this build does not know or a computed column the engine
/// refuses.
pub fn columns_of_template(template: &BomTemplate) -> Result<Vec<BomColumn>, String> {
    template
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let at = |why: String| format!("column {} of the BOM template: {why}", index + 1);
            match column {
                TemplateColumn::Standard(key) => STANDARD
                    .iter()
                    .find(|(known, _)| *known == key.as_str())
                    .map(|(_, column)| column.clone())
                    .ok_or_else(|| {
                        let keys: Vec<&str> = STANDARD.iter().map(|(known, _)| *known).collect();
                        at(format!(
                            "unknown column `{key}`; the standard columns are {}",
                            keys.join(", ")
                        ))
                    }),
                TemplateColumn::Field(name) => Ok(BomColumn::Custom(name.clone())),
                TemplateColumn::Computed { name, expression } => {
                    ComputedColumn::new(name.clone(), expression.clone())
                        .map(BomColumn::Computed)
                        .map_err(|error| at(error.to_string()))
                }
            }
        })
        .collect()
}

/// The template of `columns`, in their order.
pub fn template_of_columns(columns: &[BomColumn]) -> BomTemplate {
    let columns = columns
        .iter()
        .map(|column| match column {
            BomColumn::Custom(name) => TemplateColumn::Field(name.clone()),
            BomColumn::Computed(computed) => TemplateColumn::Computed {
                name: computed.name().to_string(),
                expression: computed.expression().to_string(),
            },
            standard => {
                TemplateColumn::Standard(standard.template_key().unwrap_or_default().to_string())
            }
        })
        .collect();
    BomTemplate { columns }
}
