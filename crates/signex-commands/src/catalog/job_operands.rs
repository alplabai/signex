//! The operands of the Export Job commands (#1299, mockup row 114). Every
//! one names the `.snxout` file by `path`.

use super::operands::{Operand, OperandKind, optional, required};

/// A run: the job, the `name` of the one output to write, the `profile` to
/// run (the job's default when absent) and `selected` for Run Selected (no
/// Validate, no zip).
pub const OUTPUT_JOB_RUN: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("name", OperandKind::Text),
    optional("profile", OperandKind::Text),
    optional("selected", OperandKind::Bool),
];

/// A job file by `path`, which a palette row without it leaves to the
/// project in view.
pub const OUTPUT_JOB_PATH_OPTIONAL: &[Operand] = &[optional("path", OperandKind::Path)];

/// A job file by `path`.
pub const OUTPUT_JOB_PATH: &[Operand] = &[required("path", OperandKind::Path)];

/// A new output of `kind`, named `name` (the kind's name when absent),
/// ticked in `profile` when one is named.
pub const JOB_OUTPUT_ADD: &[Operand] = &[
    required("path", OperandKind::Path),
    required("kind", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("profile", OperandKind::Text),
];

/// One output's settings, or its removal.
pub const JOB_OUTPUT_SET: &[Operand] = &[
    required("path", OperandKind::Path),
    required("name", OperandKind::Text),
    optional("enabled", OperandKind::Bool),
    optional("file", OperandKind::Text),
    optional("dpi", OperandKind::Text),
    optional("odb_folder", OperandKind::Bool),
    optional("remove", OperandKind::Bool),
];

/// A profile made (a copy of `from`), renamed (`to`), removed, made the
/// default, or set: its variant, ticked `outputs` (comma separated),
/// Validate, its policy and its container.
pub const JOB_PROFILE_SET: &[Operand] = &[
    required("path", OperandKind::Path),
    required("profile", OperandKind::Text),
    optional("from", OperandKind::Text),
    optional("to", OperandKind::Text),
    optional("remove", OperandKind::Bool),
    optional("default", OperandKind::Bool),
    optional("variant", OperandKind::Text),
    optional("outputs", OperandKind::Text),
    optional("validate", OperandKind::Bool),
    optional("on_errors", OperandKind::Text),
    optional("on_warnings", OperandKind::Text),
    optional("folder", OperandKind::Text),
    optional("naming", OperandKind::Text),
    optional("zip", OperandKind::Bool),
    optional("group_folders", OperandKind::Bool),
    optional("release_folder", OperandKind::Bool),
    optional("clear_folder", OperandKind::Bool),
];

/// What starts a profile without a click: on save (with its `sources` and
/// `wait` in seconds), keep outputs up to date, before close (`off`, `ask`,
/// `always`) and the release `tag` pattern.
pub const JOB_TRIGGERS_SET: &[Operand] = &[
    required("path", OperandKind::Path),
    required("profile", OperandKind::Text),
    optional("on_save", OperandKind::Bool),
    optional("sources", OperandKind::Text),
    optional("wait", OperandKind::Text),
    optional("keep_up_to_date", OperandKind::Bool),
    optional("before_close", OperandKind::Text),
    optional("tag", OperandKind::Text),
];

/// A run's files in the Gerber Viewer: `run` is the history row (0 the
/// newest), else this session's last run.
pub const JOB_RUN_VIEW: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("run", OperandKind::Text),
];
