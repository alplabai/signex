//! Paging for the list endpoints.

/// Rows returned when a request names no `limit`.
pub const DEFAULT_PAGE_LIMIT: i64 = 500;

/// Hard cap on `limit`; larger requests are clamped to it.
pub const MAX_PAGE_LIMIT: i64 = 1000;

/// A `LIMIT` / `OFFSET` window over an ordered list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub limit: i64,
    pub offset: i64,
}

impl Page {
    /// Window from raw query values: `limit` defaults to
    /// [`DEFAULT_PAGE_LIMIT`] and is clamped to `1..=MAX_PAGE_LIMIT`;
    /// `offset` defaults to 0 and is never negative.
    pub fn new(limit: Option<i64>, offset: Option<i64>) -> Self {
        Self {
            limit: limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT),
            offset: offset.unwrap_or(0).max(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamps() {
        let page = Page::new(None, None);
        assert_eq!((page.limit, page.offset), (DEFAULT_PAGE_LIMIT, 0));
        let page = Page::new(Some(10_000), Some(-5));
        assert_eq!((page.limit, page.offset), (MAX_PAGE_LIMIT, 0));
        let page = Page::new(Some(0), Some(7));
        assert_eq!((page.limit, page.offset), (1, 7));
    }
}
