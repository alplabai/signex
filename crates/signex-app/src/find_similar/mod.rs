//! Find Similar Objects dialog (#1257): state and messages (`state`) and the
//! pure view (`view`). The handlers that resolve the seed, scan and select
//! live in `app/handlers/find_similar/`.

mod state;
mod view;

pub use state::{FindSimilarMsg, FindSimilarState, ProjectMatches, SimilarScope};
pub use view::view;
