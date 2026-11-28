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

/// Inter — the default UI chrome face (Graphite look). Static faces, unmodified.
/// Every weight the chrome asks for (400, 500, 600, 700) is bundled: a weight
/// with no Inter face draws in another family (#3004).
pub const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
pub const INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
pub const INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.ttf");

/// JetBrains Mono — chrome-only monospace (shortcuts, values, badges). Never
/// canvas text: canvas extents depend on the Iosevka default.
pub const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");
pub const JETBRAINS_MONO_MEDIUM: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMono-Medium.ttf");

/// Every bundled face, in the order `main` registers them.
pub const BUNDLED_FONTS: [&[u8]; 12] = [
    IOSEVKA_REGULAR,
    IOSEVKA_BOLD,
    IOSEVKA_ITALIC,
    IOSEVKA_BOLD_ITALIC,
    ROBOTO_REGULAR,
    ROBOTO_BOLD,
    INTER_REGULAR,
    INTER_MEDIUM,
    INTER_SEMIBOLD,
    INTER_BOLD,
    JETBRAINS_MONO_REGULAR,
    JETBRAINS_MONO_MEDIUM,
];

/// Every face the app registers with iced: the bundled faces, then iced_aw's
/// icon font (the ColorPicker's Cancel / OK glyphs; without it their buttons
/// draw as empty rectangles). `main` and the test layout probe both load
/// this list, so tests measure text in the faces the app draws.
pub fn registered() -> impl Iterator<Item = &'static [u8]> {
    BUNDLED_FONTS
        .into_iter()
        .chain(std::iter::once(iced_aw::ICED_AW_FONT_BYTES))
}

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

#[cfg(test)]
mod weight_tests {
    /// #3004 — the bundled set, which the GUI registers and a headless run
    /// measures with (`measure::install_hermetic`), holds an upright face of
    /// the default UI family at every weight the chrome asks for (400, 500,
    /// 600, 700), so no weight falls back to another family.
    #[test]
    fn the_ui_family_has_an_upright_face_at_every_chrome_weight() {
        let mut db = fontdb::Database::new();
        for face in super::BUNDLED_FONTS {
            db.load_font_data(face.to_vec());
        }
        let family = signex_widgets::ui_font::DEFAULT_FAMILY;
        let weights: Vec<u16> = db
            .faces()
            .filter(|face| face.style == fontdb::Style::Normal)
            .filter(|face| face.families.iter().any(|(name, _)| name == family))
            .map(|face| face.weight.0)
            .collect();
        for weight in [400, 500, 600, 700] {
            assert!(
                weights.contains(&weight),
                "no {family} face at weight {weight}: {weights:?}"
            );
        }
    }
}
