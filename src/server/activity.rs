use super::protocol::{Activity, Cursor, Result};
use std::collections::VecDeque;

pub(super) struct Store {
    id: String,
    position: u64,
    floor: u64,
    bytes: usize,
    limit: usize,
    entries: VecDeque<(Activity, usize)>,
}
impl Store {
    pub fn new(id: String, limit: usize) -> Self {
        Self {
            id,
            position: 0,
            floor: 0,
            bytes: 0,
            limit,
            entries: VecDeque::new(),
        }
    }
    fn cursor(&self, position: u64) -> Cursor {
        Cursor {
            session_id: self.id.clone(),
            position,
        }
    }
    pub fn push(&mut self, event: serde_json::Value) {
        self.position = self
            .position
            .checked_add(1)
            .expect("activity position exhausted");
        let entry = Activity {
            cursor: self.cursor(self.position),
            event,
        };
        let bytes = serde_json::to_vec(&entry).unwrap().len();
        while self.bytes.saturating_add(bytes) > self.limit {
            let Some((removed, size)) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= size;
            self.floor = removed.cursor.position;
        }
        if bytes > self.limit {
            self.floor = self.position;
        } else {
            self.bytes += bytes;
            self.entries.push_back((entry, bytes));
        }
    }
    pub fn poll(&self, cursor: Option<&Cursor>) -> Result {
        if cursor.is_some_and(|c| c.session_id != self.id || c.position > self.position) {
            return Result::Error {
                code: "invalid_cursor".into(),
                message: "cursor is foreign or ahead of this session".into(),
            };
        }
        let position = cursor.map_or(0, |c| c.position);
        if position < self.floor {
            return Result::Gap {
                cursor: self.cursor(self.floor),
                message: "activity was evicted; missing outcomes are unknown".into(),
            };
        }
        Result::Activity {
            entries: self
                .entries
                .iter()
                .filter(|(e, _)| e.cursor.position > position)
                .map(|(e, _)| e.clone())
                .collect(),
            cursor: self.cursor(self.position),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_oversize_and_foreign_cursor() {
        let mut store = Store::new("a".repeat(32), 256);
        for _ in 0..10 {
            store.push(serde_json::json!({"kind":"pending","request_id":1}));
        }
        let Result::Gap { cursor, .. } = store.poll(None) else {
            panic!("expected gap")
        };
        assert!(matches!(store.poll(Some(&cursor)), Result::Activity { .. }));
        store.push(serde_json::json!({"value": "x".repeat(1000)}));
        let Result::Gap { cursor, .. } = store.poll(Some(&cursor)) else {
            panic!("oversize must leave gap")
        };
        let Result::Activity { entries, .. } = store.poll(Some(&cursor)) else {
            panic!()
        };
        assert!(entries.is_empty());
        assert!(matches!(
            store.poll(Some(&Cursor {
                session_id: "b".repeat(32),
                position: 0
            })),
            Result::Error { .. }
        ));
    }
}
