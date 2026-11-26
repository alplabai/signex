//! `image_file_missing` (#1116, mockup row 204): an image on the sheet names
//! a file that is gone, refused or holds other content than the image
//! placed. The rule reads no disk: the project run checks the files off the
//! UI thread and names the missing ones in [`crate::ErcConfig`]. One finding
//! per image, at its centre.

use std::collections::BTreeSet;

use signex_types::schematic::{Point, SchDrawing, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::RuleKind;
use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;

/// An image on the sheet as `image_file_missing` reads it.
#[derive(Debug, Clone)]
pub struct ErcImage {
    pub uuid: Uuid,
    /// The project-relative file it names.
    pub file: String,
    /// Its centre, where a finding points.
    pub centre: Point,
    /// Known missing when the context was built. A context built from a
    /// sheet leaves it `false`; the run names missing files in
    /// [`crate::ErcConfig::missing_images`].
    pub missing: bool,
}

impl ErcImage {
    /// The images among `drawings`.
    pub fn of_sheet(drawings: &[SchDrawing]) -> Vec<Self> {
        drawings
            .iter()
            .filter_map(|d| match d {
                SchDrawing::Image(image) => Some(Self {
                    uuid: image.uuid,
                    file: image.file.clone(),
                    centre: image.centre(),
                    missing: false,
                }),
                _ => None,
            })
            .collect()
    }
}

/// One finding per image whose file is missing.
pub(crate) fn image_file_missing(
    ctx: &ErcContext,
    missing_files: &BTreeSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    for image in &ctx.images {
        if !(image.missing || missing_files.contains(&image.file)) {
            continue;
        }
        let message = format!(
            "Image file missing: {}. The image keeps its place and draws a placeholder.",
            image.file
        );
        out.push(
            Diagnostic::new(RuleKind::ImageFileMissing, message, image.centre)
                .with_primary(SelectedItem::new(image.uuid, SelectedKind::Drawing)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::run_all_with_config;
    use crate::pin_matrix::ErcConfig;

    fn context_with(file: &str, missing: bool) -> ErcContext {
        let mut ctx = crate::fixtures::empty();
        ctx.images.push(ErcImage {
            uuid: Uuid::from_u128(4),
            file: file.into(),
            centre: Point::new(12.0, 8.0),
            missing,
        });
        ctx
    }

    fn found(ctx: &ErcContext, config: &ErcConfig) -> Vec<Diagnostic> {
        run_all_with_config(ctx, config)
            .into_iter()
            .filter(|d| d.rule_kind == RuleKind::ImageFileMissing)
            .collect()
    }

    #[test]
    fn a_file_the_run_names_missing_is_one_warning_at_the_image() {
        let ctx = context_with("assets/images/plot.png", false);
        assert!(found(&ctx, &ErcConfig::default()).is_empty());
        let config = ErcConfig::default()
            .with_missing_images(["assets/images/plot.png".to_string()].into_iter().collect());
        let hits = found(&ctx, &config);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].location, Point::new(12.0, 8.0));
        assert_eq!(hits[0].severity, crate::Severity::Warning);
        assert!(hits[0].message.contains("assets/images/plot.png"));
        assert_eq!(
            hits[0].primary,
            Some(SelectedItem::new(Uuid::from_u128(4), SelectedKind::Drawing))
        );
    }

    #[test]
    fn another_file_or_a_present_one_finds_nothing() {
        let ctx = context_with("assets/images/plot.png", false);
        let config = ErcConfig::default().with_missing_images(
            ["assets/images/other.png".to_string()]
                .into_iter()
                .collect(),
        );
        assert!(found(&ctx, &config).is_empty());
    }

    #[test]
    fn the_images_of_a_sheet_are_read_from_its_drawings() {
        let image = signex_types::schematic::SchImage::new(
            Uuid::from_u128(9),
            Point::new(0.0, 0.0),
            (10.0, 4.0),
            "assets/images/a.svg".into(),
            "aa".into(),
        );
        let read = ErcImage::of_sheet(&[SchDrawing::Image(image)]);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].file, "assets/images/a.svg");
        assert_eq!(read[0].centre, Point::new(5.0, 2.0));
        assert!(!read[0].missing);
    }
}
