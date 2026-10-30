use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

#[derive(Clone, Debug, Default)]
pub struct Queue {
    order: Vec<i64>,
    original: Vec<i64>,
    cursor: usize,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

impl Queue {
    pub fn new(mut tracks: Vec<i64>, start: usize) -> Self {
        let selected = tracks.get(start).copied();
        let mut seen = std::collections::HashSet::new();
        tracks.retain(|id| seen.insert(*id));
        let cursor = selected
            .and_then(|id| tracks.iter().position(|x| *x == id))
            .unwrap_or(0);
        Self {
            original: tracks.clone(),
            order: tracks,
            cursor,
            ..Default::default()
        }
    }
    pub fn current(&self) -> Option<i64> {
        self.order.get(self.cursor).copied()
    }
    pub fn position(&self) -> usize {
        self.cursor
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    pub fn neighbor(&self, delta: isize) -> Option<i64> {
        self.cursor
            .checked_add_signed(delta)
            .and_then(|i| self.order.get(i).copied())
    }
    pub fn ids(&self) -> &[i64] {
        &self.order
    }
    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle == enabled {
            return;
        }
        self.shuffle = enabled;
        if enabled {
            // Current track stays playing at the front; every other track is shuffled after it.
            if !self.order.is_empty() {
                let current = self.order.remove(self.cursor);
                self.order.shuffle(&mut rand::rng());
                self.order.insert(0, current);
                self.cursor = 0;
            }
        } else {
            let id = self.current();
            self.order.clone_from(&self.original);
            self.cursor = id
                .and_then(|v| self.order.iter().position(|x| *x == v))
                .unwrap_or(0);
        }
    }
    /// Drops a track removed from the library; the cursor stays on the same song when it can.
    pub fn remove(&mut self, id: i64) {
        self.original.retain(|x| *x != id);
        let Some(index) = self.order.iter().position(|x| *x == id) else {
            return;
        };
        self.order.remove(index);
        if index < self.cursor {
            self.cursor -= 1;
        }
        self.cursor = self.cursor.min(self.order.len().saturating_sub(1));
    }
    pub fn advance(&mut self, manual: bool) -> Option<i64> {
        if self.order.is_empty() {
            return None;
        }
        if !manual && self.repeat == RepeatMode::One {
            return self.current();
        }
        if self.cursor + 1 < self.order.len() {
            self.cursor += 1;
        } else if self.repeat == RepeatMode::All {
            let last = self.current();
            if self.shuffle {
                self.order.shuffle(&mut rand::rng());
                if self.order.len() > 1 && self.order.first().copied() == last {
                    self.order.swap(0, 1);
                }
            }
            self.cursor = 0;
        } else {
            return None;
        }
        self.current()
    }
    pub fn previous(&mut self) -> Option<i64> {
        self.cursor = self.cursor.saturating_sub(1);
        self.current()
    }
}
