//! Which class template each table row reads (Caner, 2026-11-27): a
//! library's own template for the class wins over the global one for that
//! library's rows. A category's columns are those of every template its
//! rows use; a column only a library's own template offers names that
//! library in the Columns picker.

use std::collections::HashMap;

use signex_library::components_view::offered_columns;
use signex_library::templates::{CategoryView, ColumnKey, ParameterTemplate};
use signex_library::{ComponentRow, TemplateRegistry};
use uuid::Uuid;

use crate::library::state::OpenLibrary;

/// A template in use by some rows.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateUse {
    pub template: ParameterTemplate,
    /// The library whose own template this is (id, display name); `None`
    /// for the global one.
    pub library: Option<(Uuid, String)>,
}

/// The templates the rows of one build use, by index.
#[derive(Debug, Clone, Default)]
pub struct RowTemplates {
    uses: Vec<TemplateUse>,
}

impl RowTemplates {
    /// The index of the template a row of `class` in `lib` reads, `None`
    /// when the class has no template.
    pub fn index_for(
        &mut self,
        registry: &TemplateRegistry,
        lib: &OpenLibrary,
        class: &str,
    ) -> Option<usize> {
        let own = registry.resolve(lib.library_id, class)?;
        let global = registry.resolve(Uuid::nil(), class);
        let library = (!global.is_some_and(|g| std::ptr::eq(g, own)))
            .then(|| (lib.library_id, lib.display_name.clone()));
        if let Some(at) = self
            .uses
            .iter()
            .position(|u| u.template.class == own.class && u.library == library)
        {
            return Some(at);
        }
        self.uses.push(TemplateUse {
            template: own.clone(),
            library,
        });
        Some(self.uses.len() - 1)
    }

    pub fn get(&self, index: Option<usize>) -> Option<&ParameterTemplate> {
        Some(&self.uses.get(index?)?.template)
    }

    /// The template that leads a category: the global one when rows use
    /// it, else the first in use.
    pub fn primary(&self) -> Option<usize> {
        self.uses
            .iter()
            .position(|u| u.library.is_none())
            .or_else(|| (!self.uses.is_empty()).then_some(0))
    }

    /// Whether any template in use pairs `name` as a min-to-max value.
    pub fn is_range(&self, name: &str) -> bool {
        self.uses.iter().any(|u| u.template.range(name).is_some())
    }
}

/// A class category's columns over its rows, each `(row, template index)`:
/// part number, the parameter columns of the leading template, those the
/// other templates add, the parameters rows hold that no template names,
/// then the other row fields. The second value names, for a column only a
/// library's own template offers (the global template does not), that
/// library.
pub fn class_columns<'r>(
    templates: &RowTemplates,
    global: Option<&ParameterTemplate>,
    rows: impl IntoIterator<Item = (&'r ComponentRow, Option<usize>)>,
) -> (Vec<ColumnKey>, HashMap<ColumnKey, String>) {
    let primary = templates.primary();
    let mut groups: Vec<(Option<usize>, Vec<&'r ComponentRow>)> = vec![(primary, Vec::new())];
    for (row, index) in rows {
        match groups.iter_mut().find(|(i, _)| *i == index) {
            Some((_, list)) => list.push(row),
            None => groups.push((index, vec![row])),
        }
    }
    let global_params = global
        .map(ParameterTemplate::parameter_columns)
        .unwrap_or_default();
    let mut params: Vec<ColumnKey> = Vec::new();
    let mut origin: HashMap<ColumnKey, String> = HashMap::new();
    for (index, list) in &groups {
        let template = templates.get(*index);
        for column in offered_columns(template, list.iter().copied()) {
            if column.param_name().is_none() || params.contains(&column) {
                continue;
            }
            let own = index
                .and_then(|i| templates.uses.get(i))
                .and_then(|u| u.library.as_ref());
            let is_template_slot =
                template.is_some_and(|t| t.parameter_columns().contains(&column));
            if let Some((_, library)) = own
                && is_template_slot
                && !global_params.contains(&column)
            {
                origin.insert(column.clone(), library.clone());
            }
            params.push(column);
        }
    }
    let mut out = vec![ColumnKey::PartNumber];
    out.extend(params);
    out.extend(ColumnKey::ROW_FIELDS.into_iter().skip(1));
    (out, origin)
}

/// The view a class category opens with: its leading template's.
pub fn class_view(templates: &RowTemplates) -> CategoryView {
    signex_library::components_view::category_view(templates.get(templates.primary()))
}

/// The Recently placed view: the All parts columns, no sort and no
/// grouping, so the parts keep the log's order.
pub fn recent_view() -> CategoryView {
    CategoryView {
        sort: Vec::new(),
        ..CategoryView::all_parts()
    }
}
