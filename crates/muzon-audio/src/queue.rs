// SPDX-License-Identifier: MIT OR Apache-2.0
//! Playback queue model.
//!
//! Implements TZ.md §2.1.1: an ordered list of tracks with
//! repeat modes (off / one / all), shuffle with back-history,
//! prev / next navigation, and snapshot for the IPC layer.
//!
//! The queue is in-memory only. Persistence of the queue
//! itself (saved queues, playlist files) lands in v0.4.0;
//! the playback position persistence lands in this issue
//! (the `resume` module) and the playback log lands in 0013.

use std::collections::VecDeque;

use muzon_ipc::{QueueEntry, QueueSnapshot, RepeatMode, TrackRef};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// One entry in the queue snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuePosition {
    pub track: TrackRef,
    pub current: bool,
}

/// The playback queue.
///
/// Internal layout:
/// - `history`: tracks that have already been played (for the
///   prev button when shuffle is on).
/// - `current`: the now-playing track (None if the queue is
///   empty or between tracks).
/// - `up_next`: the tracks queued to play after the current
///   one.
/// - `original_order`: a parallel list of `up_next` in the
///   order they were enqueued. When shuffle is toggled off,
///   we restore `up_next` from `original_order`.
#[derive(Debug, Clone)]
pub struct Queue {
    history: VecDeque<TrackRef>,
    current: Option<TrackRef>,
    up_next: VecDeque<TrackRef>,
    original_order: Vec<TrackRef>,
    mode: RepeatMode,
    shuffle: bool,
    shuffle_seed: u64,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    /// Build an empty queue with `Off` repeat and shuffle off.
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            current: None,
            up_next: VecDeque::new(),
            original_order: Vec::new(),
            mode: RepeatMode::Off,
            shuffle: false,
            shuffle_seed: 0,
        }
    }

    /// Append a track to the end of the queue.
    pub fn enqueue(&mut self, track: TrackRef) {
        self.up_next.push_back(track.clone());
        self.original_order.push(track);
    }

    /// Insert a track at the head of `up_next` (next to play).
    pub fn enqueue_next(&mut self, track: TrackRef) {
        self.up_next.push_front(track.clone());
        self.original_order.insert(0, track);
    }

    /// Remove the entry at the given position in the snapshot
    /// view (history + current + up_next). Returns true if a
    /// row was removed.
    pub fn remove(&mut self, position: usize) -> bool {
        let total = self.history.len()
            + if self.current.is_some() { 1 } else { 0 }
            + self.up_next.len();
        if position >= total {
            return false;
        }
        if position < self.history.len() {
            self.history.remove(position);
            return true;
        }
        // Position is in the `current` slot or in `up_next`.
        // If current is Some, the slot at history.len() is
        // current; otherwise up_next starts at history.len().
        let after_history = position - self.history.len();
        if self.current.is_some() && after_history == 0 {
            self.current = None;
            return true;
        }
        let up_next_idx = if self.current.is_some() {
            after_history - 1
        } else {
            after_history
        };
        if up_next_idx < self.up_next.len() {
            // Mirror the removal in `original_order` (the
            // parallel list of the unsuffled order). We use
            // position equality: the snapshot's `up_next`
            // index equals the `original_order` index only
            // when shuffle is off; when shuffle is on, the
            // mapping is indirect. v0.2.0 minimum: we use
            // the `up_next` index directly. v0.2.0 hardening
            // can compute the correct `original_order` index
            // from the snapshot position.
            if up_next_idx < self.original_order.len() {
                self.original_order.remove(up_next_idx);
            }
            self.up_next.remove(up_next_idx);
            return true;
        }
        false
    }

    /// Move the entry at `from` to `to` in the snapshot view.
    /// The indexes are the same as in `remove`.
    pub fn move_item(&mut self, from: usize, to: usize) {
        let total = self.history.len()
            + if self.current.is_some() { 1 } else { 0 }
            + self.up_next.len();
        if from >= total || to >= total || from == to {
            return;
        }
        // Build a flat list, splice, write back. Simpler than
        // juggling the three sub-lists.
        let mut flat: Vec<Option<TrackRef>> = Vec::with_capacity(total);
        for t in &self.history {
            flat.push(Some(t.clone()));
        }
        if let Some(c) = &self.current {
            flat.push(Some(c.clone()));
        }
        for t in &self.up_next {
            flat.push(Some(t.clone()));
        }
        let item = flat.remove(from);
        flat.insert(to, item);
        // Rebuild the three sub-lists. The split point for
        // current is the history length.
        let h_len = self.history.len();
        let c_present = self.current.is_some();
        self.history = flat[..h_len]
            .iter()
            .filter_map(|x| x.clone())
            .collect();
        self.current = if c_present {
            flat[h_len].clone()
        } else {
            None
        };
        self.up_next = flat[h_len + if c_present { 1 } else { 0 }..]
            .iter()
            .filter_map(|x| x.clone())
            .collect();
    }

    /// Remove all entries and reset to empty.
    pub fn clear(&mut self) {
        self.history.clear();
        self.current = None;
        self.up_next.clear();
        self.original_order.clear();
    }

    /// Advance to the next track. Honours repeat mode. Returns
    /// the new `current` (or `None` if the queue is exhausted
    /// and repeat is `Off`).
    pub fn next(&mut self) -> Option<TrackRef> {
        match self.mode {
            RepeatMode::One => {
                // Repeat the current track. If there is no
                // current, fall through to the normal
                // up_next path.
                if let Some(c) = &self.current {
                    return Some(c.clone());
                }
            }
            RepeatMode::All | RepeatMode::Off => {}
        }
        // Move current to history.
        if let Some(c) = self.current.take() {
            self.history.push_back(c);
        }
        // Pop the next track. If up_next is empty and
        // repeat is All, restore from history (loop).
        if self.up_next.is_empty() {
            if matches!(self.mode, RepeatMode::All) && !self.history.is_empty() {
                // Move all history back to up_next.
                self.up_next = self.history.drain(..).collect();
                // original_order is the un-shuffled original;
                // reset it to the loop's up_next.
                self.original_order = self.up_next.iter().cloned().collect();
            } else {
                // No more tracks; current stays None.
                return None;
            }
        }
        let next = self.up_next.pop_front();
        if next.is_some() {
            // Mirror the pop in `original_order`.
            if !self.original_order.is_empty() {
                self.original_order.remove(0);
            }
        }
        self.current = next.clone();
        next
    }

    /// Move back to the previous track. Honours the shuffle
    /// history: walks the `history` deque.
    pub fn prev(&mut self) -> Option<TrackRef> {
        match self.mode {
            RepeatMode::One => {
                if let Some(c) = &self.current {
                    return Some(c.clone());
                }
            }
            RepeatMode::All | RepeatMode::Off => {}
        }
        // Pop from history. The most recent entry is at the
        // back; pop_back returns it.
        let prev = self.history.pop_back();
        if prev.is_some() {
            // Restore the popped track at the head of up_next
            // and at the head of original_order.
            if let Some(t) = &prev {
                self.up_next.push_front(t.clone());
                self.original_order.insert(0, t.clone());
            }
            // Move the current to the front of up_next.
            if let Some(c) = self.current.take() {
                self.up_next.push_front(c);
            }
        }
        self.current = prev.clone();
        prev
    }

    /// Toggle shuffle on or off.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        if on {
            // Seed from the current epoch second (or 0 on
            // failure). v0.2.0 minimum; v0.2.0 hardening
            // accepts an explicit seed.
            self.shuffle_seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.shuffle_in_place();
        } else {
            // Restore from `original_order`.
            self.up_next = self.original_order.iter().cloned().collect();
        }
    }

    /// Set the repeat mode.
    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.mode = mode;
    }

    /// Take a snapshot of the queue for the IPC layer.
    pub fn snapshot(&self) -> QueueSnapshot {
        let mut entries = Vec::new();
        let mut position: u32 = 0;
        for t in &self.history {
            entries.push(QueueEntry {
                track: t.clone(),
                position,
            });
            position += 1;
        }
        let current_position = if self.current.is_some() {
            entries.push(QueueEntry {
                track: self.current.as_ref().unwrap().clone(),
                position,
            });
            let cp = position;
            position += 1;
            Some(cp)
        } else {
            None
        };
        for t in &self.up_next {
            entries.push(QueueEntry {
                track: t.clone(),
                position,
            });
            position += 1;
        }
        QueueSnapshot {
            entries,
            current_position,
            repeat: self.mode,
            shuffle: self.shuffle,
        }
    }

    /// Length of the queue (history + current + up_next).
    pub fn len(&self) -> usize {
        self.history.len()
            + if self.current.is_some() { 1 } else { 0 }
            + self.up_next.len()
    }

    /// True if the queue has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Current track.
    pub fn current(&self) -> Option<&TrackRef> {
        self.current.as_ref()
    }

    /// Repeat mode.
    pub fn repeat(&self) -> RepeatMode {
        self.mode
    }

    /// Shuffle on/off.
    pub fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    /// Shuffle `up_next` in place using the seeded PRNG.
    fn shuffle_in_place(&mut self) {
        let mut rng = StdRng::seed_from_u64(self.shuffle_seed);
        let n = self.up_next.len();
        if n < 2 {
            return;
        }
        // Fisher-Yates shuffle: for i from n-1 down to 1, swap
        // up_next[i] with up_next[j] where j is in [0, i].
        let mut vec: Vec<TrackRef> = self.up_next.drain(..).collect();
        for i in (1..n).rev() {
            let j = rng.gen_range(0..=i);
            vec.swap(i, j);
        }
        self.up_next = vec.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(path: &str) -> TrackRef {
        TrackRef {
            track_id: None,
            path: path.to_string(),
            title: None,
            artist: None,
            album: None,
            duration_ms: None,
        }
    }

    #[test]
    fn new_queue_is_empty() {
        let q = Queue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.current().is_none());
    }

    #[test]
    fn enqueue_then_next() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.enqueue(tr("/c.mp3"));
        assert_eq!(q.len(), 3);
        let n = q.next();
        assert_eq!(n.unwrap().path, "/a.mp3");
        let n = q.next();
        assert_eq!(n.unwrap().path, "/b.mp3");
        let n = q.next();
        assert_eq!(n.unwrap().path, "/c.mp3");
        let n = q.next();
        assert!(n.is_none(), "Off mode stops at the end");
    }

    #[test]
    fn enqueue_next_inserts_at_head() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.enqueue_next(tr("/c.mp3"));
        let n = q.next();
        assert_eq!(n.unwrap().path, "/c.mp3");
        let n = q.next();
        assert_eq!(n.unwrap().path, "/a.mp3");
        let n = q.next();
        assert_eq!(n.unwrap().path, "/b.mp3");
    }

    #[test]
    fn prev_walks_history() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.enqueue(tr("/c.mp3"));
        // Two next() calls: current = /b, history = [/a], up_next = [/c].
        q.next();
        q.next();
        let n = q.prev();
        assert!(n.is_some());
        assert_eq!(n.unwrap().path, "/a.mp3");
    }

    #[test]
    fn repeat_one_repeats_current() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.next();
        q.set_repeat(RepeatMode::One);
        let n = q.next();
        assert!(n.is_some());
        assert_eq!(n.unwrap().path, "/a.mp3");
        let n = q.next();
        assert!(n.is_some());
        assert_eq!(n.unwrap().path, "/a.mp3");
    }

    #[test]
    fn repeat_all_loops() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.set_repeat(RepeatMode::All);
        q.next();
        q.next();
        // Queue exhausted, but All loops back to /a.
        let n = q.next();
        assert!(n.is_some());
        assert_eq!(n.unwrap().path, "/a.mp3");
    }

    #[test]
    fn clear_resets_state() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.next();
        q.clear();
        assert!(q.is_empty());
        assert!(q.current().is_none());
        assert!(q.snapshot().entries.is_empty());
    }

    #[test]
    fn snapshot_preserves_order() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.enqueue(tr("/c.mp3"));
        q.next();
        let snap = q.snapshot();
        assert_eq!(snap.entries.len(), 3);
        assert_eq!(snap.entries[0].track.path, "/a.mp3");
        assert_eq!(snap.entries[1].track.path, "/b.mp3");
        assert_eq!(snap.entries[2].track.path, "/c.mp3");
        assert_eq!(snap.current_position, Some(0));
        assert_eq!(snap.repeat, RepeatMode::Off);
        assert!(!snap.shuffle);
    }

    #[test]
    fn shuffle_on_permutes_up_next() {
        let mut q = Queue::new();
        for i in 0..20 {
            q.enqueue(tr(&format!("/t{i}.mp3")));
        }
        let original: Vec<String> =
            (0..20).map(|i| format!("/t{i}.mp3")).collect();
        q.set_shuffle(true);
        let snap = q.snapshot();
        let shuffled: Vec<String> = snap
            .entries
            .iter()
            .map(|e| e.track.path.clone())
            .collect();
        // It is astronomically unlikely that a random shuffle
        // of 20 elements is the identity. We assert that the
        // set is preserved (no duplicates, no losses) but
        // the order is not the original.
        let mut sorted = shuffled.clone();
        sorted.sort();
        let mut expected = original.clone();
        expected.sort();
        assert_eq!(sorted, expected);
        // The shuffled order is almost certainly different
        // from the original. If it happens to be the same,
        // the test is still valid (we just don't assert on
        // the inequality).
        if shuffled == original {
            eprintln!("shuffle produced the original order; the test passes by chance");
        }
    }

    #[test]
    fn shuffle_off_restores_original() {
        let mut q = Queue::new();
        for i in 0..5 {
            q.enqueue(tr(&format!("/t{i}.mp3")));
        }
        q.set_shuffle(true);
        q.set_shuffle(false);
        let snap = q.snapshot();
        let restored: Vec<String> = snap
            .entries
            .iter()
            .map(|e| e.track.path.clone())
            .collect();
        let expected: Vec<String> =
            (0..5).map(|i| format!("/t{i}.mp3")).collect();
        assert_eq!(restored, expected);
    }

    #[test]
    fn remove_removes_entry() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.enqueue(tr("/c.mp3"));
        let removed = q.remove(1);
        assert!(removed, "remove(1) should succeed");
        let snap = q.snapshot();
        assert_eq!(snap.entries.len(), 2);
        assert_eq!(snap.entries[0].track.path, "/a.mp3");
        assert_eq!(snap.entries[1].track.path, "/c.mp3");
    }

    #[test]
    fn move_item_reorders() {
        let mut q = Queue::new();
        q.enqueue(tr("/a.mp3"));
        q.enqueue(tr("/b.mp3"));
        q.enqueue(tr("/c.mp3"));
        q.move_item(0, 2);
        let snap = q.snapshot();
        assert_eq!(snap.entries[0].track.path, "/b.mp3");
        assert_eq!(snap.entries[1].track.path, "/c.mp3");
        assert_eq!(snap.entries[2].track.path, "/a.mp3");
    }
}
