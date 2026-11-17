//! Running a command under an [`Origin`] (#2253). The type itself lives in
//! `app::core::origin`.

use crate::app::Signex;
use crate::app::core::origin::Origin;

impl Signex {
    /// Run `f` with `origin` as the source of every command it runs, and put
    /// the previous one back when it returns, on every return path of `f`. A
    /// panic is not covered.
    pub(crate) fn with_origin<R>(&mut self, origin: Origin, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = std::mem::replace(&mut self.agent.origin, origin);
        let result = f(self);
        self.agent.origin = previous;
        result
    }
}
