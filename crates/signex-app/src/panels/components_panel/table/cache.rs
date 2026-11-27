//! The table as `update` last built it: the category's rows with their
//! facts, the effective setup, the sorted and grouped tree and the lines
//! the view draws. `view` only reads this (#1353 and
//! docs/architecture/library-scale-bench.md): with 25,000 parts a sort or
//! a regroup runs once per change, never per frame.
//!
//! Three stages, each rebuilt only when its inputs change:
//! 1. base: the category's rows, facts, offered columns and template
//!    (data version, category, language);
//! 2. arrange: search, range filters and In stock, then sort and group
//!    (the search text, the setup's sort and group, the filters);
//! 3. lines: the open groups flattened (the open set).

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use signex_i18n::I18n;
use signex_library::components_view::{
    CellValue, DetailEntry, GroupPath, RowFacts, ViewLine, ViewNode, ViewSetup, arrange_rows,
    category_view, column_value, detail_parameters, offered_columns, visible_lines,
};
use signex_library::templates::{CategoryView, ColumnKey, ParameterTemplate, SortKey};
use signex_library::{ComponentRow, RangeFilter, RangeOn, RangeVerdict, TemplateRegistry};
use uuid::Uuid;

use super::category::{CategoryEntry, categories_of};
use super::state::{CategoryKey, ComponentsTable};
use super::templates::{RowTemplates, class_columns, class_view, recent_view};
use crate::library::range_filter::FilterColumn;
use crate::library::state::{ComponentsPanelRowKey, OpenLibrary, PanelRows};

/// What the table is built from, borrowed from `LibraryState`.
pub(crate) struct TableSource<'a> {
    pub libraries: &'a [OpenLibrary],
    /// The search matches, per library.
    pub rows: &'a PanelRows,
    pub registry: &'a TemplateRegistry,
    pub data_version: u64,
    /// The search text as typed.
    pub needle: &'a str,
    pub selected: Option<&'a ComponentsPanelRowKey>,
    /// The Recently placed log, most recent first.
    pub recent: &'a [ComponentsPanelRowKey],
}

/// One part of the category.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub key: ComponentsPanelRowKey,
    /// Index into `LibraryState::open_libraries` at the data version the
    /// cache was built for.
    pub lib: usize,
    pub table: Arc<str>,
    pub index: usize,
    pub facts: RowFacts,
    /// The class template the row reads, in [`TableCache::templates`].
    pub template: Option<usize>,
}

impl TableRow {
    pub fn row<'a>(&self, libraries: &'a [OpenLibrary]) -> Option<&'a ComponentRow> {
        libraries
            .get(self.lib)?
            .tables
            .get(&*self.table)?
            .get(self.index)
    }
}

/// One line the view draws.
#[derive(Debug, Clone, PartialEq)]
pub enum TableLine {
    Group {
        path: GroupPath,
        column: ColumnKey,
        value: CellValue,
        count: usize,
        depth: usize,
        open: bool,
    },
    /// A part, by its index in [`TableCache::base`].
    Row { row: usize, depth: usize },
}

/// One distributor offer of the selected part, for Details' Part Choices.
#[derive(Debug, Clone, PartialEq)]
pub struct PartChoice {
    pub distributor: String,
    pub sku: String,
    pub stock: Option<u32>,
    /// Unit price at quantity 1, with its currency.
    pub price: Option<(f64, String)>,
}

/// The selected part's Details lists.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DetailsCache {
    pub key: Option<ComponentsPanelRowKey>,
    /// The part's category, whose pins the list follows.
    pub category: CategoryKey,
    pub parameters: Vec<DetailEntry>,
    /// The "≥ low" / "≤ high" text of each min-to-max pair with one end
    /// set, by name.
    pub half_set: BTreeMap<String, String>,
    /// Datasheet and other document labels or links.
    pub references: Vec<String>,
    pub choices: Vec<PartChoice>,
    /// The part's category view and offered columns, so a pin starts the
    /// category's saved setup from them.
    pub view: CategoryView,
    pub offered: Vec<ColumnKey>,
    built_for: Option<(u64, Option<Vec<String>>)>,
}

#[derive(Debug, Clone, PartialEq)]
struct BaseInputs {
    data_version: u64,
    category: CategoryKey,
    locale: &'static str,
    recent: Vec<ComponentsPanelRowKey>,
}

#[derive(Debug, Clone, PartialEq)]
struct ArrangeInputs {
    needle: String,
    sort: Vec<SortKey>,
    group: Vec<SortKey>,
    filters: Vec<(String, RangeFilter, RangeOn)>,
    in_stock: bool,
    /// Recently placed with no sort or grouping: the log's order.
    keep_order: bool,
}

/// See the module docs.
#[derive(Debug, Clone, Default)]
pub struct TableCache {
    base_inputs: Option<BaseInputs>,
    arrange_inputs: Option<ArrangeInputs>,
    lines_for: Option<u64>,
    pub categories: Vec<CategoryEntry>,
    /// The templates the rows read; a library's own wins for its rows.
    pub templates: RowTemplates,
    /// The category's leading template (`RowTemplates::primary`), if any.
    pub template: Option<ParameterTemplate>,
    /// The library a column comes from when only its own template offers
    /// it.
    pub column_origin: HashMap<ColumnKey, String>,
    pub offered: Vec<ColumnKey>,
    pub default_view: CategoryView,
    /// The setup in effect, fitted to `offered`.
    pub setup: ViewSetup,
    /// Every part of the category.
    pub base: Vec<TableRow>,
    /// Parts per column that hold a value, for the Columns picker.
    pub coverage: HashMap<ColumnKey, usize>,
    /// Measurement columns a range filter can go on.
    pub(crate) range_columns: Vec<FilterColumn>,
    /// Indices into `base` that pass the search and the filters.
    pub shown: Vec<usize>,
    /// Parts each range filter dropped for having no comparable value.
    pub excluded: BTreeMap<String, usize>,
    /// `shown` sorted and grouped; rows index into `shown`.
    pub nodes: Vec<ViewNode>,
    pub lines: Vec<TableLine>,
    pub details: DetailsCache,
    #[cfg(test)]
    pub arrange_passes: usize,
}

/// Bring `table.cache` up to date with `src`.
pub(crate) fn sync(table: &mut ComponentsTable, src: &TableSource<'_>, i18n: &I18n) {
    let base_inputs = BaseInputs {
        data_version: src.data_version,
        category: table.category.clone(),
        locale: i18n.locale(),
        recent: src.recent.to_vec(),
    };
    let base_changed = table.cache.base_inputs.as_ref() != Some(&base_inputs);
    if base_changed {
        table.cache.rebuild_base(src, &table.category, i18n);
        table.cache.base_inputs = Some(base_inputs);
    }
    table.cache.setup = table.effective_setup();
    let filters = table.filters().cloned().unwrap_or_default();
    let arrange_inputs = ArrangeInputs {
        needle: src.needle.trim().to_owned(),
        sort: table.cache.setup.sort.clone(),
        group: table.cache.setup.group.clone(),
        filters: filters.range.active_on(),
        in_stock: filters.in_stock,
        keep_order: table.category == CategoryKey::Recent
            && table.cache.setup.sort.is_empty()
            && table.cache.setup.group.is_empty(),
    };
    let arranged = base_changed || table.cache.arrange_inputs.as_ref() != Some(&arrange_inputs);
    if arranged {
        table.cache.rearrange(src, &arrange_inputs, i18n);
        table.cache.arrange_inputs = Some(arrange_inputs);
    }
    if arranged || table.cache.lines_for != Some(table.open_generation) {
        let lines = flatten(&table.cache.nodes, &table.cache.shown, |path| {
            table.is_open(path)
        });
        table.cache.lines = lines;
        table.cache.lines_for = Some(table.open_generation);
    }
    sync_details(table, src, i18n);
}

impl TableCache {
    fn rebuild_base(&mut self, src: &TableSource<'_>, category: &CategoryKey, i18n: &I18n) {
        self.categories = categories_of(src.libraries, src.recent.len(), i18n);
        self.templates = RowTemplates::default();
        self.base = match category {
            CategoryKey::Recent => recent_rows(src, &mut self.templates),
            _ => rows_of(src, category, &mut self.templates),
        };
        self.template = self.templates.get(self.templates.primary()).cloned();
        let libraries = src.libraries;
        (self.offered, self.column_origin) = match category.class() {
            Some(class) => {
                let global = src.registry.resolve(Uuid::nil(), class);
                let rows = with_templates(&self.base, libraries);
                class_columns(&self.templates, global, rows)
            }
            None => {
                let rows = with_templates(&self.base, libraries).map(|(row, _)| row);
                (offered_columns(None, rows), HashMap::new())
            }
        };
        self.default_view = match category {
            CategoryKey::AllParts => CategoryView::all_parts(),
            CategoryKey::Recent => recent_view(),
            CategoryKey::Class(_) => class_view(&self.templates),
        };
        self.coverage = self
            .offered
            .iter()
            .map(|column| {
                let n = self
                    .base
                    .iter()
                    .filter_map(|r| Some((r.row(libraries)?, r)))
                    .filter(|(row, r)| {
                        !column_value(row, &r.facts, self.templates.get(r.template), column)
                            .is_missing()
                    })
                    .count();
                (column.clone(), n)
            })
            .collect();
        self.range_columns = range_columns(self, libraries, i18n);
    }

    /// The template `r` reads.
    pub fn template_of(&self, r: &TableRow) -> Option<&ParameterTemplate> {
        self.templates.get(r.template)
    }

    /// Whether `name` is a min-to-max column of any template in use.
    pub fn is_range(&self, name: &str) -> bool {
        self.templates.is_range(name)
    }

    fn rearrange(&mut self, src: &TableSource<'_>, inputs: &ArrangeInputs, i18n: &I18n) {
        #[cfg(test)]
        {
            self.arrange_passes += 1;
        }
        let matches = search_matches(src, inputs.needle.is_empty());
        let templates = &self.templates;
        let mut excluded: BTreeMap<String, usize> = BTreeMap::new();
        self.shown = (0..self.base.len())
            .filter(|&i| {
                let r = &self.base[i];
                let Some(row) = r.row(src.libraries) else {
                    return false;
                };
                if let Some(matches) = &matches
                    && !matches.contains(&(r.lib, &*r.table, r.index))
                {
                    return false;
                }
                if inputs.in_stock && r.facts.stock.unwrap_or(0) == 0 {
                    return false;
                }
                let mut keep = true;
                for (key, filter, on) in &inputs.filters {
                    let column = ColumnKey::from(key.clone());
                    let cell = column_value(row, &r.facts, templates.get(r.template), &column);
                    match cell.range_verdict(filter, *on) {
                        RangeVerdict::Matches => {}
                        RangeVerdict::Outside => keep = false,
                        RangeVerdict::Excluded => {
                            *excluded.entry(key.clone()).or_default() += 1;
                            keep = false;
                        }
                    }
                }
                keep
            })
            .collect();
        self.excluded = excluded;
        let (base, shown, libraries) = (&self.base, &self.shown, src.libraries);
        if inputs.keep_order {
            self.nodes = (0..self.shown.len()).map(ViewNode::Row).collect();
            return;
        }
        let cell = |i: usize, column: &ColumnKey| {
            let r = &base[shown[i]];
            r.row(libraries).map_or(CellValue::Missing, |row| {
                column_value(row, &r.facts, templates.get(r.template), column)
            })
        };
        let text_order = |a: &str, b: &str| -> Ordering { i18n.compare_for_display(a, b) };
        self.nodes = arrange_rows(
            self.shown.len(),
            &inputs.sort,
            &inputs.group,
            cell,
            text_order,
        );
    }
}

/// Every part of `category` across the mounted libraries, in library,
/// table name, then row order.
fn rows_of(
    src: &TableSource<'_>,
    category: &CategoryKey,
    templates: &mut RowTemplates,
) -> Vec<TableRow> {
    let mut out = Vec::new();
    for (lib_index, lib) in src.libraries.iter().enumerate() {
        let names = Names::of(lib);
        let mut tables: Vec<(&String, &Vec<ComponentRow>)> = lib.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        for (name, rows) in tables {
            let table: Arc<str> = Arc::from(name.as_str());
            for (index, row) in rows.iter().enumerate() {
                if category.class().is_some_and(|c| c != row.class.as_str()) {
                    continue;
                }
                let template = templates.index_for(src.registry, lib, row.class.as_str());
                out.push(table_row(
                    lib_index, lib, &table, index, row, &names, template,
                ));
            }
        }
    }
    out
}

/// The Recently placed parts still mounted, in the log's order (most
/// recent first, as many as the log keeps).
fn recent_rows(src: &TableSource<'_>, templates: &mut RowTemplates) -> Vec<TableRow> {
    src.recent
        .iter()
        .filter_map(|key| {
            let (lib_index, lib) = src
                .libraries
                .iter()
                .enumerate()
                .find(|(_, l)| l.root == key.library_path)?;
            let rows = lib.tables.get(&key.table)?;
            let index = rows.iter().position(|r| r.row_id == key.row_id.0)?;
            let row = &rows[index];
            let names = Names::of(lib);
            let template = templates.index_for(src.registry, lib, row.class.as_str());
            let table: Arc<str> = Arc::from(key.table.as_str());
            Some(table_row(
                lib_index, lib, &table, index, row, &names, template,
            ))
        })
        .collect()
}

/// A library's symbol and footprint names by uuid.
struct Names<'a> {
    symbols: HashMap<Uuid, &'a str>,
    footprints: HashMap<Uuid, &'a str>,
}

impl<'a> Names<'a> {
    fn of(lib: &'a OpenLibrary) -> Self {
        Names {
            symbols: names_by_uuid(&lib.cached_symbols),
            footprints: names_by_uuid(&lib.cached_footprints),
        }
    }
}

fn table_row(
    lib_index: usize,
    lib: &OpenLibrary,
    table: &Arc<str>,
    index: usize,
    row: &ComponentRow,
    names: &Names<'_>,
    template: Option<usize>,
) -> TableRow {
    let (stock, price) = supply_facts(lib, row);
    TableRow {
        key: ComponentsPanelRowKey {
            library_path: lib.root.clone(),
            table: table.to_string(),
            row_id: signex_library::RowId(row.row_id),
        },
        lib: lib_index,
        table: Arc::clone(table),
        index,
        facts: RowFacts {
            library_ref: names
                .symbols
                .get(&row.symbol_ref.uuid)
                .map(|s| s.to_string()),
            footprint: row
                .footprint_ref
                .and_then(|r| names.footprints.get(&r.uuid))
                .map(|s| s.to_string()),
            stock,
            price,
        },
        template,
    }
}

/// Each row of `base` with the template it reads.
fn with_templates<'r>(
    base: &'r [TableRow],
    libraries: &'r [OpenLibrary],
) -> impl Iterator<Item = (&'r ComponentRow, Option<usize>)> + 'r {
    base.iter()
        .filter_map(move |r| Some((r.row(libraries)?, r.template)))
}

fn names_by_uuid(listing: &[signex_library::PrimitiveSummary]) -> HashMap<Uuid, &str> {
    listing
        .iter()
        .filter(|s| !s.name.is_empty())
        .map(|s| (s.uuid, s.name.as_str()))
        .collect()
}

/// The first cached offer's stock and quantity-1 price.
fn supply_facts(lib: &OpenLibrary, row: &ComponentRow) -> (Option<u64>, Option<f64>) {
    let Some(cached) = lib.pricing.get(&row.row_id) else {
        return (None, None);
    };
    let offers = signex_library::current_pricing(row, cached);
    let Some((_, offer)) = offers.first() else {
        return (None, None);
    };
    (offer.stock.map(u64::from), offer.unit_price_at(1))
}

/// The rows the search keeps, as `(library, table, index)`; `None` without
/// a search (every row stays).
fn search_matches<'a>(
    src: &TableSource<'a>,
    no_search: bool,
) -> Option<HashSet<(usize, &'a str, usize)>> {
    if no_search {
        return None;
    }
    let mut out = HashSet::new();
    for (lib_index, lib) in src.libraries.iter().enumerate() {
        for (table, index) in src.rows.visible_rows(&lib.root, usize::MAX) {
            out.insert((lib_index, table, index));
        }
    }
    Some(out)
}

/// The measurement columns of the category, each in the unit most of its
/// values are in.
fn range_columns(cache: &TableCache, libraries: &[OpenLibrary], i18n: &I18n) -> Vec<FilterColumn> {
    cache
        .offered
        .iter()
        .filter_map(|column| {
            let name = column.param_name()?;
            let mut units: Vec<(signex_types::quantity::Unit, usize)> = Vec::new();
            for r in &cache.base {
                let Some(row) = r.row(libraries) else {
                    continue;
                };
                let cell = column_value(row, &r.facts, cache.template_of(r), column);
                if let Some(q) = cell.quantities().first() {
                    match units.iter_mut().find(|(u, _)| u == q.unit()) {
                        Some((_, n)) => *n += 1,
                        None => units.push((q.unit().clone(), 1)),
                    }
                }
            }
            let (unit, _) = units.into_iter().max_by_key(|(_, n)| *n)?;
            Some(FilterColumn {
                key: name.to_owned(),
                label: super::columns::caption(column, i18n),
                unit,
            })
        })
        .collect()
}

/// The view lines: groups always, rows and deeper groups under open ones.
fn flatten(
    nodes: &[ViewNode],
    shown: &[usize],
    is_open: impl Fn(&GroupPath) -> bool,
) -> Vec<TableLine> {
    visible_lines(nodes, &is_open)
        .into_iter()
        .map(|line| match line {
            ViewLine::Group { node, depth, open } => TableLine::Group {
                path: node.path.clone(),
                column: node.column.clone(),
                value: node.value.clone(),
                count: node.count,
                depth,
                open,
            },
            ViewLine::Row { index, depth } => TableLine::Row {
                row: shown[index],
                depth,
            },
        })
        .collect()
}

/// Rebuild the selected part's Details lists when the part, its data or
/// its category's pins changed.
fn sync_details(table: &mut ComponentsTable, src: &TableSource<'_>, i18n: &I18n) {
    let Some(key) = src.selected else {
        table.cache.details = DetailsCache::default();
        return;
    };
    let Some((lib, row)) = row_at(src.libraries, key) else {
        table.cache.details = DetailsCache::default();
        return;
    };
    let category = CategoryKey::Class(row.class.as_str().to_owned());
    // The row's own library's template wins (Caner, 2026-11-27).
    let template = src.registry.resolve(lib.library_id, row.class.as_str());
    let saved_pins = table
        .setups
        .get(category.pref_key())
        .map(|s| s.pinned.clone());
    let fresh = table.cache.details.key.as_ref() == Some(key)
        && table
            .cache
            .details
            .built_for
            .as_ref()
            .is_some_and(|(version, pins)| *version == src.data_version && *pins == saved_pins);
    if fresh {
        return;
    }
    let view = category_view(template);
    let pinned = saved_pins.clone().unwrap_or_else(|| view.pinned.clone());
    let offered = if table.category == category {
        table.cache.offered.clone()
    } else {
        let rows = src
            .libraries
            .iter()
            .flat_map(|l| l.tables.values())
            .flatten()
            .filter(|r| r.class == row.class);
        offered_columns(template, rows)
    };
    let mut references = Vec::new();
    if !row.datasheet.is_empty() {
        references.push(reference_text(&row.datasheet_label, &row.datasheet));
    }
    for doc in &row.more_datasheets {
        references.push(reference_text(&doc.label, &doc.source));
    }
    let choices = part_choices(lib, row);
    let parameters = detail_parameters(row, template, &pinned);
    let half_set = parameters
        .iter()
        .filter(|e| e.value.is_missing())
        .filter_map(|e| {
            Some((
                e.name.clone(),
                super::cells::half_set(row, template, &e.name, i18n)?,
            ))
        })
        .collect();
    table.cache.details = DetailsCache {
        key: Some(key.clone()),
        category,
        parameters,
        half_set,
        references,
        choices,
        view,
        offered,
        built_for: Some((src.data_version, saved_pins)),
    };
}

fn row_at<'a>(
    libraries: &'a [OpenLibrary],
    key: &ComponentsPanelRowKey,
) -> Option<(&'a OpenLibrary, &'a ComponentRow)> {
    let lib = libraries.iter().find(|l| l.root == key.library_path)?;
    let row = lib
        .tables
        .get(&key.table)?
        .iter()
        .find(|r| r.row_id == key.row_id.0)?;
    Some((lib, row))
}

/// A document as Details lists it: its label, else its link or file.
fn reference_text(label: &str, source: &signex_library::DatasheetRef) -> String {
    if !label.trim().is_empty() {
        return label.trim().to_owned();
    }
    match source {
        signex_library::DatasheetRef::Url { url } => url.clone(),
        signex_library::DatasheetRef::HashPinned { filename, .. } => filename.clone(),
    }
}

fn part_choices(lib: &OpenLibrary, row: &ComponentRow) -> Vec<PartChoice> {
    let cached = lib.pricing.get(&row.row_id);
    row.supply
        .iter()
        .map(|listing| {
            let offer = cached.and_then(|c| {
                signex_library::current_pricing(row, c)
                    .into_iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&listing.distributor))
                    .map(|(_, offer)| offer)
            });
            PartChoice {
                distributor: listing.distributor.clone(),
                sku: listing.sku.clone(),
                stock: offer.and_then(|o| o.stock),
                price: offer.and_then(|o| Some((o.unit_price_at(1)?, o.currency.clone()))),
            }
        })
        .collect()
}
