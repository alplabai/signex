//! Ratchet (#1251): every panel rendered by `view_stub` is a panel that
//! does not exist yet; it says so on its face ("Planned — not available
//! yet"). The set may only shrink. A new stub panel fails here; build the
//! panel instead, or — if it must ship planned — add it to `EXPECTED` and
//! justify it in the PR. A panel that became real must be removed too.

const EXPECTED: &[&str] = &[
    "PanelKind::Filter",
    "PanelKind::Signal",
    "PanelKind::Drc",
    "PanelKind::LayerStack",
    "PanelKind::NetClasses",
    "PanelKind::Variants",
    "PanelKind::OutputJobs",
    "PanelKind::SchFilter",
    "PanelKind::SchList",
    "PanelKind::BomStudio",
    "PanelKind::Favorites",
    "PanelKind::Snippets",
    "PanelKind::Todo",
    "PanelKind::Wiki",
    "PanelKind::Library",
];

/// The panel kind named by the nearest `PanelKind::` before each stub
/// call, so one-line arms and multi-line `=> {` arms are both seen.
fn stub_panels(source: &str) -> Vec<String> {
    let call = concat!("view_", "stub(");
    let mut kinds = Vec::new();
    let mut from = 0;
    while let Some(rel) = source[from..].find(call) {
        let at = from + rel;
        from = at + call.len();
        let Some(kind_at) = source[..at].rfind("PanelKind::") else {
            continue;
        };
        let tail = &source[kind_at..];
        let end = tail
            .find(|c: char| !(c.is_alphanumeric() || c == ':' || c == '_'))
            .unwrap_or(tail.len());
        kinds.push(tail[..end].to_string());
    }
    kinds
}

#[test]
fn stub_panels_can_only_shrink() {
    let found = stub_panels(include_str!("mod.rs"));
    for kind in &found {
        assert!(
            EXPECTED.contains(&kind.as_str()),
            "`{kind}` is a new stub panel. Build the panel instead; if it must \
             ship planned, add it to EXPECTED in panels/planned_tests.rs and \
             justify it in the PR."
        );
    }
    assert_eq!(
        found.len(),
        EXPECTED.len(),
        "a stub panel became real: drop it from EXPECTED so the ratchet tightens \
         (found {found:?})"
    );
}

#[test]
fn the_scanner_sees_multi_line_arms() {
    let fake = "PanelKind::Brand => {\n    let x = 1;\n    view_stub(\"t\", \"d\", ctx)\n}";
    assert_eq!(stub_panels(fake), vec!["PanelKind::Brand"]);
}
