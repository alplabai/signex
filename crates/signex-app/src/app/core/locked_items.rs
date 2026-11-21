//! Which commands of an edit the lock lets through.

use signex_engine::{Command, Engine};

/// `commands` without what the lock would refuse (see
/// `Engine::without_locked`), and the references skipped.
pub(crate) fn locked_items_pruned(
    engine: &Engine,
    commands: Vec<Command>,
) -> (Vec<Command>, Vec<String>) {
    let mut kept = Vec::with_capacity(commands.len());
    let mut skipped = Vec::new();
    for command in commands {
        let (command, references) = engine.without_locked(command);
        kept.extend(command);
        skipped.extend(references);
    }
    skipped.sort();
    skipped.dedup();
    (kept, skipped)
}
