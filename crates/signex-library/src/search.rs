//! Faceted parametric search. Tantivy-backed implementation lives in
//! [`crate::search_index`].

use crate::adapter::ComponentSummary;

#[derive(Clone, Debug, Default)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub category: Option<String>,
    pub facets: Vec<Facet>,
    pub limit: usize,
}

#[derive(Clone, Debug)]
pub struct Facet {
    pub field: String,
    pub op: FacetOp,
    pub value: String,
}

#[derive(Clone, Debug)]
pub enum FacetOp {
    Eq,
    Lt,
    Gt,
    Contains,
}

pub trait SearchIndex: Send + Sync {
    fn query(&self, q: &SearchQuery) -> Vec<ComponentSummary>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_default_is_empty() {
        let q = SearchQuery::default();
        assert!(q.text.is_none());
        assert!(q.facets.is_empty());
    }
}
