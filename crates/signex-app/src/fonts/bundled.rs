//! The font files compiled into the binary and registered with iced at
//! startup.
//!
//! The canvas family ships all four upright / italic × regular / bold
//! faces. cosmic-text only shapes with a face whose style equals the
//! request, so an italic request with no italic Iosevka bundled leaves the
//! family for whatever italic the system has — a different typeface with
//! different advance widths (#654).
//!
//! The Iosevka files are subsets of the 34.3.0 release, all cut to the same
//! codepoints; see `assets/fonts/README.md`.

/// Iosevka — schematic / PCB canvas text (monospace, tuned for EDA).
pub const IOSEVKA_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Iosevka-Regular.ttf");
pub const IOSEVKA_BOLD: &[u8] = include_bytes!("../../assets/fonts/Iosevka-Bold.ttf");
pub const IOSEVKA_ITALIC: &[u8] = include_bytes!("../../assets/fonts/Iosevka-Italic.ttf");
pub const IOSEVKA_BOLD_ITALIC: &[u8] = include_bytes!("../../assets/fonts/Iosevka-BoldItalic.ttf");

/// Roboto — UI chrome (panels, toolbars, menus, dialogs).
pub const ROBOTO_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Roboto-Regular.ttf");
pub const ROBOTO_BOLD: &[u8] = include_bytes!("../../assets/fonts/Roboto-Bold.ttf");

/// Every bundled face, in the order `main` registers them.
pub const BUNDLED_FONTS: [&[u8]; 6] = [
    IOSEVKA_REGULAR,
    IOSEVKA_BOLD,
    IOSEVKA_ITALIC,
    IOSEVKA_BOLD_ITALIC,
    ROBOTO_REGULAR,
    ROBOTO_BOLD,
];

#[cfg(test)]
mod license_tests {
    /// #1179 — every bundled font file ships with its license: a `.ttf` in
    /// `assets/fonts` named `<Family>-<Style>.ttf` needs `OFL-<Family>.md`
    /// beside it, which the release packages copy.
    #[test]
    fn every_bundled_font_has_its_license_beside_it() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("fonts folder") {
            let path = entry.expect("entry").path();
            if path.extension().is_none_or(|e| e != "ttf") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy();
            let family = stem.split('-').next().unwrap();
            let license = dir.join(format!("OFL-{family}.md"));
            assert!(
                license.is_file(),
                "{} has no {}",
                path.display(),
                license.display()
            );
            checked += 1;
        }
        assert_eq!(checked, super::BUNDLED_FONTS.len());
    }
}
