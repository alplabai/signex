//! App-local undo stack now tracks engine step markers only.

#[derive(Debug, Clone)]
struct HistoryEntry {
    steps: usize,
}

/// Undo history stack with configurable depth.
pub struct UndoStack {
    history: Vec<HistoryEntry>,
    position: usize,
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            history: Vec::new(),
            position: 0,
            max_depth,
        }
    }

    pub fn record_engine_marker(&mut self, steps: usize) {
        if steps == 0 {
            return;
        }

        self.record(HistoryEntry { steps });
    }

    fn record(&mut self, entry: HistoryEntry) {
        // Truncate any redo history
        self.history.truncate(self.position);
        self.history.push(entry);
        self.position += 1;
        // Trim oldest if over max depth
        if self.history.len() > self.max_depth {
            let excess = self.history.len() - self.max_depth;
            self.history.drain(0..excess);
            self.position -= excess;
        }
    }

    pub fn peek_undo_engine_steps(&self) -> Option<usize> {
        (self.position > 0).then(|| self.history[self.position - 1].steps)
    }

    pub fn peek_redo_engine_steps(&self) -> Option<usize> {
        (self.position < self.history.len()).then(|| self.history[self.position].steps)
    }

    pub fn step_back(&mut self) -> bool {
        if self.position == 0 {
            return false;
        }
        self.position -= 1;
        true
    }

    pub fn step_forward(&mut self) -> bool {
        if self.position >= self.history.len() {
            return false;
        }
        self.position += 1;
        true
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        self.position < self.history.len()
    }
}
