//! The Gerber output's settings in its form (#1299, mockup row 114): units,
//! number format, zero suppression, the layers written and the aperture
//! form, each a field of its own. Every field starts at the writer's own
//! setting, the one a job without it writes.

use std::path::PathBuf;

use iced::Element;
use iced::widget::{Column, column};
use signex_i18n::{MessageId, ids, t};
use signex_types::output_job::{
    GERBER_ALL_COPPER, GerberApertures, GerberFormat, GerberUnits, GerberZeros, JobOutput,
};

use super::{Frame, SP, kit};
use crate::app::Message;
use crate::app::output_job::tab::{GerberEdit, JobTabMsg};

/// The formats the field offers; a job's own other format is added to it.
const FORMATS: [(u8, u8); 6] = [(4, 6), (4, 5), (3, 5), (2, 6), (2, 5), (2, 4)];

/// The layer ticks, by layer id, with their words. Inner copper layers one
/// by one are for the command (`set_job_gerber_layers`).
const LAYERS: [(&str, MessageId); 11] = [
    (GERBER_ALL_COPPER, ids::OUTPUT_JOB_FORM_GERBER_LAYER_COPPER),
    ("copper-top", ids::OUTPUT_JOB_FORM_GERBER_LAYER_COPPER_TOP),
    (
        "copper-bottom",
        ids::OUTPUT_JOB_FORM_GERBER_LAYER_COPPER_BOTTOM,
    ),
    ("legend-top", ids::OUTPUT_JOB_FORM_GERBER_LAYER_LEGEND_TOP),
    (
        "legend-bottom",
        ids::OUTPUT_JOB_FORM_GERBER_LAYER_LEGEND_BOTTOM,
    ),
    (
        "soldermask-top",
        ids::OUTPUT_JOB_FORM_GERBER_LAYER_SOLDERMASK_TOP,
    ),
    (
        "soldermask-bottom",
        ids::OUTPUT_JOB_FORM_GERBER_LAYER_SOLDERMASK_BOTTOM,
    ),
    ("paste-top", ids::OUTPUT_JOB_FORM_GERBER_LAYER_PASTE_TOP),
    (
        "paste-bottom",
        ids::OUTPUT_JOB_FORM_GERBER_LAYER_PASTE_BOTTOM,
    ),
    ("profile", ids::OUTPUT_JOB_FORM_GERBER_LAYER_PROFILE),
    ("vscore", ids::OUTPUT_JOB_FORM_GERBER_LAYER_VSCORE),
];

/// The settings fields of the Gerber output `output` of `job`.
pub(super) fn fields<'a>(
    frame: &Frame<'a>,
    job: PathBuf,
    output: &JobOutput,
) -> Element<'a, Message> {
    let look = frame.look;
    let settings = output.gerber_settings();
    let name = output.name.clone();
    let send = move |edit: GerberEdit| {
        Message::from(JobTabMsg::GerberEdited {
            job: job.clone(),
            name: name.clone(),
            edit,
        })
    };
    let units = {
        let send = send.clone();
        kit::choice(
            look.tokens,
            settings.units,
            vec![
                (GerberUnits::Mm, look.t(ids::OUTPUT_JOB_FORM_GERBER_MM)),
                (GerberUnits::Inch, look.t(ids::OUTPUT_JOB_FORM_GERBER_INCH)),
            ],
            move |units| send(GerberEdit::UnitsPicked(units)),
        )
    };
    let mut formats: Vec<GerberFormat> = FORMATS
        .iter()
        .map(|&(integer, decimal)| GerberFormat { integer, decimal })
        .collect();
    if !formats.contains(&settings.format) {
        formats.push(settings.format);
    }
    let format = {
        let send = send.clone();
        let choices = formats
            .into_iter()
            .map(|format| {
                let words = t!(look.i18n, ids::OUTPUT_JOB_FORM_GERBER_FORMAT_VALUE,
                    "integer" => u32::from(format.integer),
                    "decimal" => u32::from(format.decimal));
                (format, words)
            })
            .collect();
        kit::choice(look.tokens, settings.format, choices, move |format| {
            send(GerberEdit::FormatPicked(format))
        })
    };
    let zeros = {
        let send = send.clone();
        kit::choice(
            look.tokens,
            settings.zeros,
            vec![
                (
                    GerberZeros::Leading,
                    look.t(ids::OUTPUT_JOB_FORM_GERBER_ZEROS_LEADING),
                ),
                (
                    GerberZeros::Trailing,
                    look.t(ids::OUTPUT_JOB_FORM_GERBER_ZEROS_TRAILING),
                ),
                (
                    GerberZeros::None,
                    look.t(ids::OUTPUT_JOB_FORM_GERBER_ZEROS_NONE),
                ),
            ],
            move |zeros| send(GerberEdit::ZerosPicked(zeros)),
        )
    };
    let apertures = {
        let send = send.clone();
        kit::choice(
            look.tokens,
            settings.apertures,
            vec![
                (
                    GerberApertures::Standard,
                    look.t(ids::OUTPUT_JOB_FORM_GERBER_APERTURES_STANDARD),
                ),
                (
                    GerberApertures::Macros,
                    look.t(ids::OUTPUT_JOB_FORM_GERBER_APERTURES_MACROS),
                ),
            ],
            move |apertures| send(GerberEdit::AperturesPicked(apertures)),
        )
    };
    let mut layers: Column<Element<'a, Message>> = column![kit::tick(
        settings.layers.is_none(),
        look.t(ids::OUTPUT_JOB_FORM_GERBER_LAYERS_ALL),
        Some(send(GerberEdit::LayerToggled(None))),
    )]
    .spacing(SP.xs);
    for (id, words) in LAYERS {
        let ticked = settings
            .layers
            .as_ref()
            .is_some_and(|list| list.iter().any(|named| named == id));
        layers = layers.push(kit::tick(
            ticked,
            look.t(words),
            Some(send(GerberEdit::LayerToggled(Some(id.to_string())))),
        ));
    }
    column![
        kit::head(look, ids::OUTPUT_JOB_FORM_GERBER_UNITS),
        units,
        kit::head(look, ids::OUTPUT_JOB_FORM_GERBER_FORMAT),
        format,
        kit::quiet(look, look.t(ids::OUTPUT_JOB_FORM_GERBER_FORMAT_NOTE)),
        kit::head(look, ids::OUTPUT_JOB_FORM_GERBER_ZEROS),
        zeros,
        kit::head(look, ids::OUTPUT_JOB_FORM_GERBER_LAYERS),
        layers,
        kit::quiet(look, look.t(ids::OUTPUT_JOB_FORM_GERBER_LAYERS_NOTE)),
        kit::head(look, ids::OUTPUT_JOB_FORM_GERBER_APERTURES),
        apertures,
    ]
    .spacing(SP.s)
    .into()
}
