#[test]
fn grid_editor_icons_are_black_paths_on_transparent_backgrounds() {
    for asset in super::view::GRID_EDITOR_ICON_ASSETS {
        let source = std::str::from_utf8(asset).expect("grid-editor SVG must be UTF-8");

        assert!(source.starts_with("<svg "));
        assert!(source.contains("<path "));
        assert!(source.contains("fill=\"#000000\""));
        assert!(!source.contains("<rect"));
        assert!(!source.contains("<circle"));
        assert!(!source.contains("<image"));
        assert!(!source.contains("background"));
    }
}
