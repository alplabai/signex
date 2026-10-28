//! Parameters section of the Project tab (#1080): the title-block values
//! every sheet inherits (title, document number, revision, date, company,
//! drawn / checked / approved by) and user-defined name/value pairs, all
//! usable as `${NAME}` special strings. A sheet overrides a value in its own
//! Title Block; see `docs/USER_GUIDE.md`.

mod view;

pub(super) use view::view;
