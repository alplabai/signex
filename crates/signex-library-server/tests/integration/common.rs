//! Process-wide fixture shared by the server tests in this binary.

use std::sync::Once;

use signex_library_server::API_TOKEN_ENV;

/// Bearer token the primitive routes expect once [`install_test_token`] ran.
pub const TEST_BEARER: &str = "test-bearer-token";

/// Set `SIGNEX_API_TOKEN` exactly once for the process.
///
/// Every test in this binary calls this as its first statement, before it
/// spawns a thread or builds a router. `Once` blocks concurrent callers until
/// the write has finished, so no test-spawned thread can exist while the
/// variable is written; the only other threads are libtest's, which do not
/// read the environment. A test added to this binary must follow the same
/// rule; `tests/database_adapter.rs` is a separate binary because it needs
/// the variable unset.
pub fn install_test_token() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // SAFETY: see the function doc: the only writer runs before any
        // test-spawned thread exists, and no other thread reads the env.
        unsafe {
            std::env::set_var(API_TOKEN_ENV, TEST_BEARER);
        }
    });
}
