//! The Export Job's annotations (#1299). A run checks the job's project and
//! starts its read; the build is later. The tab's commands read the job's
//! draft or its file, nothing implicit.

use super::super::forms::Form::Operand;
use super::Annotation;

pub(super) const OUTPUT_JOB: &[Annotation] = &[(
    &[
        "run_output_job",
        "new_output_job",
        "open_output_job",
        "read_output_job",
        "add_job_output",
        "set_job_output",
        "set_job_profile",
        "set_job_triggers",
        "set_job_gerber_units",
        "set_job_gerber_format",
        "set_job_gerber_zeros",
        "set_job_gerber_layers",
        "set_job_gerber_apertures",
        "stop_output_job",
        "view_output_job_run",
    ],
    Operand,
    &[],
)];
