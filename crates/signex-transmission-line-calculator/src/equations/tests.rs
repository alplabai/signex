use super::*;

#[test]
fn every_reference_equation_is_embedded_as_svg() {
    assert_eq!(EQUATION_SVGS.len(), FORMULA_REFERENCES.len());

    for (equation, rendered) in FORMULA_REFERENCES.iter().zip(EQUATION_SVGS) {
        let svg = std::str::from_utf8(rendered)
            .unwrap_or_else(|error| panic!("invalid SVG for {}: {error}", equation.item));

        assert!(
            svg.contains("<svg"),
            "{} did not render as SVG",
            equation.item
        );
        assert!(
            svg.contains("width=\"") && svg.contains("height=\""),
            "{} does not expose intrinsic dimensions",
            equation.item
        );
    }
}
