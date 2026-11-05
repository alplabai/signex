//! Temporary probe-only timing spans (not for merge).
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

thread_local! {
    static SPANS: RefCell<BTreeMap<&'static str, (Duration, u32)>> = RefCell::new(BTreeMap::new());
}

pub fn time<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    let d = t.elapsed();
    SPANS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.entry(name).or_default();
        e.0 += d;
        e.1 += 1;
    });
    out
}

pub fn take() -> BTreeMap<&'static str, (Duration, u32)> {
    SPANS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}
