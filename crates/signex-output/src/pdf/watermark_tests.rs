//! Tests for the PDF watermark.
use super::colour::ColourMap;
use super::optional_content_tests::page_ops;
use super::tests::sample_ctx;
use super::*;

const PAPER: (f32, f32, f32) = (1.0, 1.0, 1.0);

fn with_watermark(watermark: Watermark) -> PdfOptions {
    PdfOptions {
        watermark: Some(watermark),
        ..PdfOptions::default()
    }
}

#[test]
fn default_is_a_red_draft_at_eighteen_percent_and_forty_five_degrees() {
    let watermark = Watermark::default();
    assert_eq!(watermark.text.printed(), "DRAFT");
    assert_eq!(watermark.opacity_percent, 18);
    assert_eq!(watermark.angle_deg, 45.0);
    assert_eq!(watermark.colour_rgb, [0xC0, 0x39, 0x2B]);
    assert_eq!(watermark.pages, WatermarkPages::All);
    assert_eq!(watermark.revision_line, None);
}

#[test]
fn presets_print_in_english_and_custom_prints_as_typed() {
    assert_eq!(WatermarkText::Draft.printed(), "DRAFT");
    assert_eq!(WatermarkText::Confidential.printed(), "CONFIDENTIAL");
    assert_eq!(
        WatermarkText::Custom("FOR BENCH USE ONLY".into()).printed(),
        "FOR BENCH USE ONLY"
    );
}

#[test]
fn placement_is_centred_on_a4_landscape() {
    let placement = Watermark::default().placement(297.0, 210.0);
    assert_eq!(placement.center_mm, (148.5, 105.0));
    // A short word is held to 15 % of the short edge, not its width.
    assert!(
        (placement.text_height_mm - 31.5).abs() < 1e-9,
        "{placement:?}"
    );
    assert!(placement.revision_height_mm < placement.text_height_mm);
    assert!(placement.revision_height_mm >= 3.0);
    assert!(placement.revision_gap_mm > placement.revision_height_mm);
}

#[test]
fn placement_passes_the_angle_through() {
    for angle in [0.0, 30.0, 45.0, 90.0, -15.0] {
        let watermark = Watermark {
            angle_deg: angle,
            ..Watermark::default()
        };
        assert_eq!(watermark.placement(297.0, 210.0).angle_deg, angle);
    }
}

#[test]
fn long_text_is_made_smaller_to_fit_across_the_page() {
    let short = Watermark::default().placement(297.0, 210.0);
    let long = Watermark {
        text: WatermarkText::Custom("FOR BENCH USE ONLY, NOT FOR PRODUCTION".into()),
        ..Watermark::default()
    }
    .placement(297.0, 210.0);
    assert!(long.text_height_mm < short.text_height_mm, "{long:?}");
    assert!(long.text_height_mm > 0.0);
}

#[test]
fn first_only_applies_to_the_first_exported_page_only() {
    let first_only = Watermark {
        pages: WatermarkPages::FirstOnly,
        ..Watermark::default()
    };
    assert!(first_only.applies_to_page(0));
    assert!(!first_only.applies_to_page(1));
    assert!(!first_only.applies_to_page(7));
    let all = Watermark::default();
    assert!((0..5).all(|page| all.applies_to_page(page)));

    let ctx = sample_ctx(2);
    let opts = with_watermark(first_only);
    assert!(page_ops(&ctx, &opts, 0).contains("/ca46 gs"));
    assert!(!page_ops(&ctx, &opts, 1).contains("/ca46 gs"));
}

#[test]
fn mono_and_greyscale_convert_the_colour_like_the_page() {
    let watermark = Watermark::default();
    let mono = ColourMap::new(ColourMode::BlackAndWhite, PAPER);
    assert_eq!(watermark.ink(&mono), (0.0, 0.0, 0.0));
    let grey = ColourMap::new(ColourMode::Grayscale, PAPER);
    let (r, g, b) = watermark.ink(&grey);
    assert!(r == g && g == b, "grey: {r} {g} {b}");
    let colour = ColourMap::new(ColourMode::Colour, PAPER);
    let (r, _, _) = watermark.ink(&colour);
    assert!((r - 0xC0 as f32 / 255.0).abs() < 1e-6);

    let ops = page_ops(
        &sample_ctx(1),
        &PdfOptions {
            colour_mode: ColourMode::BlackAndWhite,
            ..with_watermark(Watermark::default())
        },
        0,
    );
    let watermark_ops = &ops[ops.rfind("q\n").expect("watermark block")..];
    assert!(
        !watermark_ops.contains(" rg") || watermark_ops.contains("0 0 0 rg"),
        "{watermark_ops}"
    );
}

#[test]
fn the_revision_line_is_fixed_english() {
    assert_eq!(
        Watermark::revision_line_for(" B ", "2026-11-19").as_deref(),
        Some("Rev B · 2026-11-19")
    );
    assert_eq!(
        Watermark::revision_line_for("", "2026-11-19").as_deref(),
        Some("2026-11-19")
    );
    assert_eq!(
        Watermark::revision_line_for("C", " ").as_deref(),
        Some("Rev C")
    );
    assert_eq!(Watermark::revision_line_for(" ", ""), None);
}

#[test]
fn the_revision_line_is_drawn_only_when_set() {
    let ctx = sample_ctx(1);
    let shows = |ops: &str| ops.matches(" Tj\n").count();
    let plain = page_ops(&ctx, &with_watermark(Watermark::default()), 0);
    let revised = page_ops(
        &ctx,
        &with_watermark(Watermark {
            revision_line: Some("Rev B · 2026-11-18".into()),
            ..Watermark::default()
        }),
        0,
    );
    let none = page_ops(&ctx, &PdfOptions::default(), 0);
    assert_eq!(shows(&plain), shows(&none) + 1, "the main text");
    assert_eq!(shows(&revised), shows(&plain) + 1, "plus the revision line");
}

#[test]
fn opacity_is_clamped_to_one_hundred_percent() {
    let over = Watermark {
        opacity_percent: 250,
        ..Watermark::default()
    };
    assert_eq!(over.alpha(), 1.0);
    assert!((Watermark::default().alpha() - 0.18).abs() < 1e-6);
    // Fully opaque needs no graphics state.
    let ops = page_ops(&sample_ctx(1), &with_watermark(over), 0);
    assert!(!ops.contains("/ca"), "{ops}");
}

#[test]
fn the_watermark_is_drawn_last_with_its_opacity_state() {
    let ctx = sample_ctx(1);
    let ops = page_ops(&ctx, &with_watermark(Watermark::default()), 0);
    assert!(ops.ends_with("Q\n"), "drawn last");
    let out = PdfExporter
        .export(&ctx, &with_watermark(Watermark::default()))
        .expect("export");
    let text = String::from_utf8_lossy(&out.bytes);
    assert!(text.contains("/ca46"), "page defines the fill-alpha state");
    assert!(text.contains("/ca 0.18"), "with the watermark's opacity");
}
