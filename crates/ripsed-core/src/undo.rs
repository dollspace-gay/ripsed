use serde::{Deserialize, Serialize};

/// An entry in the undo log, storing enough information to reverse an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEntry {
    /// The full original text before the operation.
    pub original_text: String,
}

/// A record in the persistent undo log file (.ripsed/undo.jsonl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRecord {
    pub timestamp: String,
    pub file_path: String,
    pub entry: UndoEntry,
}

/// Manages the undo log.
pub struct UndoLog {
    records: Vec<UndoRecord>,
    max_entries: usize,
}

impl UndoLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            records: Vec::new(),
            max_entries,
        }
    }

    /// Load undo log from JSONL content.
    pub fn from_jsonl(content: &str, max_entries: usize) -> Self {
        let records: Vec<UndoRecord> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Self {
            records,
            max_entries,
        }
    }

    /// Serialize the log to JSONL format.
    pub fn to_jsonl(&self) -> String {
        self.records
            .iter()
            .filter_map(|r| serde_json::to_string(r).ok())
            .collect::<Vec<_>>()
            .join("\n")
            + if self.records.is_empty() { "" } else { "\n" }
    }

    /// Append a new undo record.
    pub fn push(&mut self, record: UndoRecord) {
        self.records.push(record);
        self.prune();
    }

    /// Remove the last N records and return them (for undo).
    pub fn pop(&mut self, count: usize) -> Vec<UndoRecord> {
        let drain_start = self.records.len().saturating_sub(count);
        self.records.drain(drain_start..).rev().collect()
    }

    /// Number of entries in the log.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Get recent entries for display.
    pub fn recent(&self, count: usize) -> &[UndoRecord] {
        let start = self.records.len().saturating_sub(count);
        &self.records[start..]
    }

    fn prune(&mut self) {
        if self.records.len() > self.max_entries {
            let excess = self.records.len() - self.max_entries;
            self.records.drain(..excess);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop() {
        let mut log = UndoLog::new(100);
        log.push(UndoRecord {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            file_path: "test.txt".to_string(),
            entry: UndoEntry {
                original_text: "hello".to_string(),
            },
        });
        assert_eq!(log.len(), 1);
        let popped = log.pop(1);
        assert_eq!(popped.len(), 1);
        assert_eq!(popped[0].file_path, "test.txt");
        assert!(log.is_empty());
    }

    #[test]
    fn test_pruning() {
        let mut log = UndoLog::new(2);
        for i in 0..5 {
            log.push(UndoRecord {
                timestamp: format!("2026-01-0{i}T00:00:00Z"),
                file_path: format!("file{i}.txt"),
                entry: UndoEntry {
                    original_text: format!("content{i}"),
                },
            });
        }
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_jsonl_roundtrip() {
        let mut log = UndoLog::new(100);
        log.push(UndoRecord {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            file_path: "test.txt".to_string(),
            entry: UndoEntry {
                original_text: "original".to_string(),
            },
        });
        let jsonl = log.to_jsonl();
        let loaded = UndoLog::from_jsonl(&jsonl, 100);
        assert_eq!(loaded.len(), 1);
    }
}
