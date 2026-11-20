//! The guard's test-only counters and switches (`guard.rs` re-exports them):
//! the incremental-derive self check, the tally of checks and the causes a
//! guarded pass fell back for.

/// Test switch for the check in `guard::derive_at`; on unless a timing bench turns
/// it off for its own thread.
pub(crate) mod self_check {
    use std::cell::Cell;

    thread_local! {
        static ON: Cell<bool> = const { Cell::new(true) };
    }

    pub(crate) fn enabled() -> bool {
        ON.with(Cell::get)
    }

    /// Turn the check off for this thread until the guard drops.
    pub(crate) fn off() -> Off {
        Off(ON.with(|flag| flag.replace(false)))
    }

    /// Restores the setting [`off`] found.
    pub(crate) struct Off(bool);

    impl Drop for Off {
        fn drop(&mut self) {
            ON.with(|flag| flag.set(self.0));
        }
    }
}

/// What the tests count on this thread: the guard's checks (how many ran on
/// the nets the move reaches, how many over every net, how many fell back to
/// every net, how many blamed a tip, what each decided), the follows the
/// journal rewound and the reconciles that read fewer wires than the sheet.
pub(crate) mod tally {
    use std::cell::RefCell;

    use signex_net::uf::Key;

    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub(crate) struct Tally {
        pub(crate) guards_built_local: usize,
        pub(crate) local_checks: usize,
        pub(crate) full_checks: usize,
        pub(crate) blames: usize,
        /// Checks of a guard built local that fell back to every net.
        pub(crate) fallbacks: usize,
        /// Failed follows whose journal put back at least one wire end.
        pub(crate) rewinds: usize,
        /// Reconciles that indexed only the wires near the touched ones.
        pub(crate) reconciles_narrowed: usize,
        /// Each check's answer, in order.
        pub(crate) decisions: Vec<(bool, Option<Vec<Key>>)>,
    }

    thread_local! {
        static TALLY: RefCell<Tally> = RefCell::new(Tally::default());
    }

    pub(crate) fn guard_built_local() {
        TALLY.with(|t| t.borrow_mut().guards_built_local += 1);
    }

    pub(crate) fn checked(local: bool, equal: bool, blamed: &Option<Vec<Key>>) {
        TALLY.with(|t| {
            let mut t = t.borrow_mut();
            if local {
                t.local_checks += 1;
            } else {
                t.full_checks += 1;
            }
            t.blames += usize::from(blamed.is_some());
            t.decisions.push((equal, blamed.clone()));
        });
    }

    pub(crate) fn fell_back() {
        TALLY.with(|t| t.borrow_mut().fallbacks += 1);
    }

    pub(crate) fn rewound() {
        TALLY.with(|t| t.borrow_mut().rewinds += 1);
    }

    pub(crate) fn reconcile_narrowed() {
        TALLY.with(|t| t.borrow_mut().reconciles_narrowed += 1);
    }

    /// The counts so far on this thread, starting again from zero.
    pub(crate) fn take() -> Tally {
        TALLY.with(|t| std::mem::take(&mut *t.borrow_mut()))
    }
}

/// Why the guarded passes on this thread fell back, for the tests: each
/// cause noted once per pass, taken by the test that counts them.
pub(crate) mod reasons {
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    thread_local! {
        static NOTED: RefCell<BTreeSet<&'static str>> = const { RefCell::new(BTreeSet::new()) };
    }

    /// Note `why` for the pass running now.
    pub(crate) fn note(why: &'static str) {
        NOTED.with(|noted| {
            noted.borrow_mut().insert(why);
        });
    }

    /// The causes noted so far on this thread, starting again from none.
    pub(crate) fn take() -> BTreeSet<&'static str> {
        NOTED.with(|noted| std::mem::take(&mut *noted.borrow_mut()))
    }
}
