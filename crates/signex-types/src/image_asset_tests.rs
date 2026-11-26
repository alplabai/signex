//! #1116: the asset sandbox, the image checks and the asset names.

use super::*;

/// A PNG header of `w` × `h` pixels (signature and IHDR only).
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&w.to_be_bytes());
    bytes.extend_from_slice(&h.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46];
    bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    bytes.extend_from_slice(&h.to_be_bytes());
    bytes.extend_from_slice(&w.to_be_bytes());
    bytes.extend_from_slice(&[0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[test]
fn a_relative_path_is_kept_with_forward_slashes() {
    assert_eq!(
        checked_asset_path("assets\\images\\./logo.svg").as_deref(),
        Ok("assets/images/logo.svg")
    );
}

#[test]
fn an_absolute_path_is_refused() {
    for path in [
        "/etc/passwd",
        "C:\\logo.png",
        "c:/logo.png",
        "~/logo.png",
        "\\\\share\\x.png",
    ] {
        assert!(
            matches!(checked_asset_path(path), Err(ImageRefusal::AbsolutePath(_))),
            "{path}"
        );
    }
}

#[test]
fn a_dot_dot_path_is_refused() {
    for path in [
        "../logo.png",
        "assets/../../logo.png",
        "assets\\..\\logo.png",
    ] {
        assert!(
            matches!(
                checked_asset_path(path),
                Err(ImageRefusal::LeavesProject(_))
            ),
            "{path}"
        );
    }
    assert_eq!(checked_asset_path("  "), Err(ImageRefusal::EmptyPath));
}

#[test]
fn png_jpeg_and_gif_sizes_are_read_from_the_header() {
    let facts = check_image(&png(1200, 584)).expect("a png");
    assert_eq!(
        (facts.format, facts.width_px, facts.height_px),
        (ImageFormat::Png, 1200, 584)
    );
    let facts = check_image(&jpeg(640, 480)).expect("a jpeg");
    assert_eq!(
        (facts.format, facts.width_px, facts.height_px),
        (ImageFormat::Jpeg, 640, 480)
    );
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&[0x20, 0x00, 0x10, 0x00, 0, 0, 0]);
    let facts = check_image(&gif).expect("a gif");
    assert_eq!(
        (facts.format, facts.width_px, facts.height_px),
        (ImageFormat::Gif, 32, 16)
    );
}

#[test]
fn an_svg_size_comes_from_its_attributes_or_its_view_box() {
    let sized = br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="25.4mm" height="96"></svg>"#;
    assert_eq!(pixel_size(ImageFormat::Svg, sized), Some((96, 96)));
    let boxed =
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 584"><path d="M0 0"/></svg>"#;
    assert_eq!(pixel_size(ImageFormat::Svg, boxed), Some((1200, 584)));
}

#[test]
fn a_file_that_is_not_an_image_is_refused() {
    assert_eq!(check_image(b"hello"), Err(ImageRefusal::UnknownFormat));
}

#[test]
fn an_oversized_file_is_refused_before_it_is_read_as_an_image() {
    let mut bytes = png(10, 10);
    bytes.resize(MAX_IMAGE_BYTES as usize + 1, 0);
    assert!(matches!(
        check_image(&bytes),
        Err(ImageRefusal::TooLarge { .. })
    ));
}

#[test]
fn a_decompression_bomb_is_refused_by_its_header() {
    assert!(matches!(
        check_image(&png(16_000, 16_000)),
        Err(ImageRefusal::TooManyPixels { .. })
    ));
    assert!(matches!(
        check_image(&png(20_000, 1)),
        Err(ImageRefusal::TooManyPixels { .. })
    ));
}

#[test]
fn an_svg_with_an_external_reference_is_refused() {
    for svg in [
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="file:///etc/passwd"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image xlink:href='https://example.com/a.png'/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect style="fill:url(other.svg#g)"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><style>@import "x.css";</style></svg>"#,
        r#"<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///etc/passwd">]><svg width="10" height="10">&x;</svg>"#,
    ] {
        assert!(
            matches!(
                check_image(svg.as_bytes()),
                Err(ImageRefusal::ExternalReference(_))
            ),
            "{svg}"
        );
    }
}

#[test]
fn an_svg_that_refers_only_to_itself_is_placed() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><defs><linearGradient id="g"/></defs><rect style="fill:url(#g)"/><use href="#g"/><image href="data:image/png;base64,AAAA"/></svg>"##;
    let facts = check_image(svg.as_bytes()).expect("self-contained");
    assert_eq!(facts.format, ImageFormat::Svg);
}

#[test]
fn the_hash_is_sha256_hex() {
    assert_eq!(
        content_hash(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn a_copied_name_keeps_the_picked_name_and_the_kind_extension() {
    assert_eq!(
        asset_file_name("C:\\pics\\Ripple 2A.PNG", ImageFormat::Png),
        "Ripple-2A.png"
    );
    assert_eq!(
        asset_file_name("/tmp/scope", ImageFormat::Jpeg),
        "scope.jpg"
    );
    assert_eq!(
        asset_file_name("photo.jpeg", ImageFormat::Jpeg),
        "photo.jpeg"
    );
    assert_eq!(
        asset_file_name("diagram.png", ImageFormat::Svg),
        "diagram.svg"
    );
    assert_eq!(asset_file_name("...", ImageFormat::Gif), "image.gif");
}

#[test]
fn the_same_content_reuses_the_file_already_in_the_project() {
    let existing = vec![("logo.svg".to_string(), "aa".to_string())];
    let choice = pick_asset_name("other-name.svg", "aa", &existing);
    assert_eq!(
        choice,
        AssetChoice {
            file: "assets/images/logo.svg".into(),
            reuses_existing: true
        }
    );
}

#[test]
fn a_name_taken_by_other_content_gets_the_hash_added() {
    let hash = "0123456789abcdef0123456789abcdef";
    let existing = vec![("logo.svg".to_string(), "ff".to_string())];
    let choice = pick_asset_name("logo.svg", hash, &existing);
    assert_eq!(choice.file, "assets/images/logo-01234567.svg");
    assert!(!choice.reuses_existing);
    let choice = pick_asset_name("new.svg", hash, &existing);
    assert_eq!(choice.file, "assets/images/new.svg");
}

#[test]
fn the_natural_size_is_read_at_96_dpi() {
    let facts = check_image(&png(96, 192)).expect("a png");
    let (w, h) = facts.natural_size_mm();
    assert!(
        (w - 25.4).abs() < 1e-9 && (h - 50.8).abs() < 1e-9,
        "{w} × {h}"
    );
}
