//! The locale the catalog samples are shown in.

use std::sync::{Arc, LazyLock};

use signex_i18n::I18n;

static EN_US: LazyLock<Arc<I18n>> = LazyLock::new(I18n::en_us);

/// The shared en-US instance. `I18n::en_us_ref` is for tests only, so the
/// catalog keeps its own `'static` borrow of the shared instance.
pub(crate) fn en_us() -> &'static I18n {
    &EN_US
}
