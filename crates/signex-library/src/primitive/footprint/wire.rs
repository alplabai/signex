//! The `.snxfpt` wire format: TOML manifest, one `[[footprints]]` entry
//! per footprint with its pads as a TSV literal block, then the optional
//! `[[models]]` section (`models.rs`).

use super::format_token::FOOTPRINT_FILE_VERSION;
use super::models::{EmbeddedModels, ModelWire};
use super::*;

/// Sentinel string substituted for each footprint's `pads_tsv` field
/// before TOML serialise; replaced post-emit with the literal multi-
/// line `'''…'''` block.
pub(super) const PADS_TSV_PLACEHOLDER_PREFIX: &str = "__SIGNEX_PADS_TSV_a1b2c3d4_";

/// On-disk wire shape. Mirrors [`FootprintFile`] but each
/// [`Footprint`]'s `pads` Vec is replaced with a `pads_tsv: String`
/// carrying the TSV-encoded payload.
#[derive(Serialize, Deserialize)]
struct FootprintFileWire {
    #[serde(default)]
    format: Option<String>,
    file_uuid: Uuid,
    #[serde(default)]
    display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    copied_from: Option<Uuid>,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    #[serde(default)]
    footprints: Vec<FootprintWire>,
    /// Embedded STEP models, written last (`models.rs`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    models: Vec<ModelWire>,
}

#[derive(Serialize, Deserialize)]
struct FootprintWire {
    uuid: Uuid,
    name: String,
    #[serde(default)]
    anchor: [f64; 2],
    /// TSV-encoded pad list — header row + one row per pad.
    pads_tsv: String,
    #[serde(default)]
    courtyard: Polygon,
    #[serde(default)]
    silk_f: Vec<FpGraphic>,
    #[serde(default)]
    silk_b: Vec<FpGraphic>,
    #[serde(default)]
    fab_f: Vec<FpGraphic>,
    #[serde(default)]
    fab_b: Vec<FpGraphic>,
    #[serde(default)]
    body_3d: Body3D,
    #[serde(default)]
    step_attachment: Option<StepAttachment>,
    #[serde(default)]
    pcb_params: ParamMap,
    #[serde(default = "default_footprint_version")]
    version: String,
    #[serde(default)]
    released: bool,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    #[serde(default = "default_schema_v2")]
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sketch: Option<signex_sketch::SketchData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pours: Vec<FpPour>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keepouts: Vec<FpKeepout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cutouts: Vec<FpCutout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    v_scores: Vec<FpVScore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mask_openings: Vec<FpMaskOpening>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mask_excludes: Vec<FpMaskExclude>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    paste_apertures: Vec<FpPasteAperture>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    default_designator: String,
    #[serde(default, skip_serializing_if = "ComponentType::is_default")]
    component_type: ComponentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "GeneratedFeatures::is_default")]
    generated: GeneratedFeatures,
    // FootprintWire (per-footprint) wire fields below; the
    // FootprintFile (file-level) wire ends here.
}

impl FootprintFile {
    /// Parse the TOML+TSV wire format. The format token is checked by
    /// [`FOOTPRINT_FILE_VERSION`]; a refused one surfaces
    /// [`FootprintFileError::UnsupportedFormat`].
    pub fn from_toml_str(text: &str) -> Result<Self, FootprintFileError> {
        let wire: FootprintFileWire = signex_types::numeric::parse_toml_finite(text)?;
        let format = FOOTPRINT_FILE_VERSION.read(wire.format.as_deref())?;
        let mut footprints = Vec::with_capacity(wire.footprints.len());
        for fw in wire.footprints {
            let pads = pads_from_tsv(&fw.pads_tsv)?;
            let mut footprint = Footprint {
                uuid: fw.uuid,
                name: fw.name,
                anchor: fw.anchor,
                pads,
                courtyard: fw.courtyard,
                silk_f: fw.silk_f,
                silk_b: fw.silk_b,
                fab_f: fw.fab_f,
                fab_b: fw.fab_b,
                body_3d: fw.body_3d,
                step_attachment: fw.step_attachment,
                pcb_params: fw.pcb_params,
                version: fw.version,
                released: fw.released,
                created: fw.created,
                updated: fw.updated,
                schema_version: fw.schema_version,
                sketch: fw.sketch,
                load_repairs: Vec::new(),
                pours: fw.pours,
                keepouts: fw.keepouts,
                cutouts: fw.cutouts,
                v_scores: fw.v_scores,
                mask_openings: fw.mask_openings,
                mask_excludes: fw.mask_excludes,
                paste_apertures: fw.paste_apertures,
                description: fw.description,
                default_designator: fw.default_designator,
                component_type: fw.component_type,
                height_mm: fw.height_mm,
                generated: fw.generated,
            };
            footprint.repair_stored_sketch();
            footprints.push(footprint);
        }
        let models = EmbeddedModels::from_wire(wire.models, &footprints)?;
        Ok(FootprintFile {
            format,
            file_uuid: wire.file_uuid,
            display_name: wire.display_name,
            copied_from: wire.copied_from,
            created: wire.created,
            updated: wire.updated,
            footprints,
            models,
        })
    }

    /// Serialise to canonical TOML+TSV. Pad lists become
    /// `pads_tsv = '''\n<header>\n<rows>\n'''` literal multi-line
    /// strings so the bulk data is line-diffable in git output.
    pub fn to_toml_string(&self) -> Result<String, FootprintFileError> {
        let mut tsv_payloads: Vec<String> = Vec::with_capacity(self.footprints.len());
        let mut wire_footprints: Vec<FootprintWire> = Vec::with_capacity(self.footprints.len());
        for (idx, fp) in self.footprints.iter().enumerate() {
            tsv_payloads.push(pads_to_tsv(&fp.pads)?);
            wire_footprints.push(FootprintWire {
                uuid: fp.uuid,
                name: fp.name.clone(),
                anchor: fp.anchor,
                pads_tsv: format!("{PADS_TSV_PLACEHOLDER_PREFIX}{idx}__"),
                courtyard: fp.courtyard.clone(),
                silk_f: fp.silk_f.clone(),
                silk_b: fp.silk_b.clone(),
                fab_f: fp.fab_f.clone(),
                fab_b: fp.fab_b.clone(),
                body_3d: fp.body_3d.clone(),
                step_attachment: fp.step_attachment.clone(),
                pcb_params: fp.pcb_params.clone(),
                version: fp.version.clone(),
                released: fp.released,
                created: fp.created,
                updated: fp.updated,
                schema_version: fp.schema_version,
                sketch: fp.sketch.clone(),
                pours: fp.pours.clone(),
                keepouts: fp.keepouts.clone(),
                cutouts: fp.cutouts.clone(),
                v_scores: fp.v_scores.clone(),
                mask_openings: fp.mask_openings.clone(),
                mask_excludes: fp.mask_excludes.clone(),
                paste_apertures: fp.paste_apertures.clone(),
                description: fp.description.clone(),
                default_designator: fp.default_designator.clone(),
                component_type: fp.component_type,
                height_mm: fp.height_mm,
                generated: fp.generated.clone(),
            });
        }
        let (model_wire, model_blocks) = self.models.to_wire(&self.referenced_model_hashes());
        let wire = FootprintFileWire {
            format: Some(self.format_token().to_string()),
            file_uuid: self.file_uuid,
            display_name: self.display_name.clone(),
            copied_from: self.copied_from,
            created: self.created,
            updated: self.updated,
            footprints: wire_footprints,
            models: model_wire,
        };
        // #828: graphics, pours and parameters are written by serde, and
        // TOML spells NaN / infinity `nan` / `inf`; refuse them by key path.
        if let Some(field) = signex_types::numeric::first_non_finite_path(&wire)
            .map_err(FootprintFileError::TomlSerialize)?
        {
            return Err(FootprintFileError::NonFiniteField { field });
        }
        let mut out = toml::to_string_pretty(&wire).map_err(FootprintFileError::TomlSerialize)?;
        for (idx, payload) in tsv_payloads.iter().enumerate() {
            let needle = format!("\"{PADS_TSV_PLACEHOLDER_PREFIX}{idx}__\"");
            let replacement = format!("'''\n{payload}'''");
            out = out.replace(&needle, &replacement);
        }
        // Last, so the pad pass never scans the model text.
        Ok(model_blocks.fill(out))
    }
}
