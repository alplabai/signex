//! In-memory advisory lock service.
//!
//! Locks are keyed on `(ComponentId, FieldSet)`. Each lock records the holder
//! and the wall-clock time it was last touched. A lock is considered free if
//! its `last_renewed + idle_ttl` has passed — that's the "idle TTL" the spec
//! calls out (default 10 min, override per-test via `set_idle_ttl`).
//!
//! Persistence is intentionally NOT in the SQL `locks` table by default: the
//! advisory layer is purely in-memory and cheap to reset on server restart.
//! The DB table is reserved for a future durable-locks mode (e.g. across
//! horizontally-scaled replicas).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use signex_library::adapter::FieldSet;
use signex_library::identity::ComponentId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockErrorKind {
    Held { holder: String },
    UnknownHolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockError {
    pub kind: LockErrorKind,
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            LockErrorKind::Held { holder } => write!(f, "lock held by {holder}"),
            LockErrorKind::UnknownHolder => write!(f, "lock not held by caller"),
        }
    }
}

impl std::error::Error for LockError {}

#[derive(Debug, Clone)]
pub struct LockSnapshot {
    pub holder: String,
    pub acquired: DateTime<Utc>,
    pub last_renewed: DateTime<Utc>,
}

#[derive(Debug)]
struct Entry {
    holder: String,
    acquired: DateTime<Utc>,
    last_renewed: Instant,
    last_renewed_wallclock: DateTime<Utc>,
}

pub struct LockManager {
    inner: Mutex<Inner>,
}

struct Inner {
    locks: HashMap<(ComponentId, FieldSet), Entry>,
    idle_ttl: Duration,
}

impl LockManager {
    pub fn new(idle_ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(Inner {
                locks: HashMap::new(),
                idle_ttl,
            }),
        }
    }

    /// Override TTL — primarily for tests so they don't have to wait minutes.
    pub fn set_idle_ttl(&self, ttl: Duration) {
        let mut inner = self.inner.lock().unwrap();
        inner.idle_ttl = ttl;
    }

    pub fn try_lock(
        &self,
        uuid: ComponentId,
        field_set: FieldSet,
        holder: &str,
    ) -> Result<(), LockError> {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let now_wall = Utc::now();
        let key = (uuid, field_set);
        if let Some(existing) = inner.locks.get(&key) {
            if existing.holder == holder {
                // Same holder may renew without contention.
            } else if now.duration_since(existing.last_renewed) < inner.idle_ttl {
                return Err(LockError {
                    kind: LockErrorKind::Held {
                        holder: existing.holder.clone(),
                    },
                });
            }
        }
        inner.locks.insert(
            key,
            Entry {
                holder: holder.to_string(),
                acquired: now_wall,
                last_renewed: now,
                last_renewed_wallclock: now_wall,
            },
        );
        Ok(())
    }

    pub fn renew(
        &self,
        uuid: ComponentId,
        field_set: FieldSet,
        holder: &str,
    ) -> Result<(), LockError> {
        let mut inner = self.inner.lock().unwrap();
        let key = (uuid, field_set);
        let entry = inner.locks.get_mut(&key).ok_or(LockError {
            kind: LockErrorKind::UnknownHolder,
        })?;
        if entry.holder != holder {
            return Err(LockError {
                kind: LockErrorKind::Held {
                    holder: entry.holder.clone(),
                },
            });
        }
        entry.last_renewed = Instant::now();
        entry.last_renewed_wallclock = Utc::now();
        Ok(())
    }

    pub fn release(
        &self,
        uuid: ComponentId,
        field_set: FieldSet,
        holder: &str,
    ) -> Result<(), LockError> {
        let mut inner = self.inner.lock().unwrap();
        let key = (uuid, field_set);
        let entry = inner.locks.get(&key).ok_or(LockError {
            kind: LockErrorKind::UnknownHolder,
        })?;
        if entry.holder != holder {
            return Err(LockError {
                kind: LockErrorKind::Held {
                    holder: entry.holder.clone(),
                },
            });
        }
        inner.locks.remove(&key);
        Ok(())
    }

    pub fn snapshot(&self, uuid: ComponentId, field_set: FieldSet) -> Option<LockSnapshot> {
        let inner = self.inner.lock().unwrap();
        let key = (uuid, field_set);
        let entry = inner.locks.get(&key)?;
        // Treat expired entries as absent.
        if Instant::now().duration_since(entry.last_renewed) >= inner.idle_ttl {
            return None;
        }
        Some(LockSnapshot {
            holder: entry.holder.clone(),
            acquired: entry.acquired,
            last_renewed: entry.last_renewed_wallclock,
        })
    }

    /// Drop expired entries. Background tasks may call this periodically.
    pub fn sweep_expired(&self) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let ttl = inner.idle_ttl;
        inner
            .locks
            .retain(|_, e| now.duration_since(e.last_renewed) < ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn try_lock_blocks_when_held() {
        let mgr = LockManager::new(Duration::from_secs(60));
        let uuid = Uuid::now_v7();
        mgr.try_lock(uuid, FieldSet::Symbol, "alice").unwrap();
        let err = mgr.try_lock(uuid, FieldSet::Symbol, "bob").unwrap_err();
        assert!(matches!(err.kind, LockErrorKind::Held { .. }));
    }

    #[test]
    fn release_then_relock_works() {
        let mgr = LockManager::new(Duration::from_secs(60));
        let uuid = Uuid::now_v7();
        mgr.try_lock(uuid, FieldSet::Symbol, "alice").unwrap();
        mgr.release(uuid, FieldSet::Symbol, "alice").unwrap();
        mgr.try_lock(uuid, FieldSet::Symbol, "bob").unwrap();
    }

    #[test]
    fn ttl_expiry_allows_takeover() {
        let mgr = LockManager::new(Duration::from_millis(20));
        let uuid = Uuid::now_v7();
        mgr.try_lock(uuid, FieldSet::Symbol, "alice").unwrap();
        std::thread::sleep(Duration::from_millis(40));
        mgr.try_lock(uuid, FieldSet::Symbol, "bob").unwrap();
    }

    #[test]
    fn different_field_sets_are_independent() {
        let mgr = LockManager::new(Duration::from_secs(60));
        let uuid = Uuid::now_v7();
        mgr.try_lock(uuid, FieldSet::Symbol, "alice").unwrap();
        mgr.try_lock(uuid, FieldSet::Footprint, "bob").unwrap();
    }

    #[test]
    fn release_by_other_fails() {
        let mgr = LockManager::new(Duration::from_secs(60));
        let uuid = Uuid::now_v7();
        mgr.try_lock(uuid, FieldSet::Symbol, "alice").unwrap();
        assert!(mgr.release(uuid, FieldSet::Symbol, "bob").is_err());
    }
}
