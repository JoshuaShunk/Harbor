use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Maximum number of entries held in the ring buffer.
pub const MAX_LOG_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    Success,
    Error,
}

/// A single tool call record captured by the gateway.
#[derive(Debug, Clone, Serialize)]
pub struct RequestLog {
    /// Monotonically increasing id for ordering and deduplication.
    pub id: u64,
    /// UTC timestamp when the call was received.
    pub timestamp: DateTime<Utc>,
    /// Name of the MCP server that handled the call.
    pub server: String,
    /// Name of the tool that was invoked.
    pub tool: String,
    /// Arguments passed to the tool (may be truncated for large payloads).
    pub input: serde_json::Value,
    /// Success or Error.
    pub status: RequestStatus,
    /// Round-trip latency in milliseconds (includes server processing time).
    pub latency_ms: u64,
    /// Error message if status is Error.
    pub error: Option<String>,
    /// Tool result payload (may be truncated for large payloads).
    pub output: Option<serde_json::Value>,
}

/// In-memory ring buffer of recent gateway tool call records.
///
/// Capped at `MAX_LOG_ENTRIES`. Older entries are evicted when the buffer is full.
/// Thread-safe — cheaply cloneable via `Arc<RequestLogger>`.
pub struct RequestLogger {
    entries: Mutex<VecDeque<RequestLog>>,
    counter: AtomicU64,
}

impl Default for RequestLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestLogger {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES + 1)),
            counter: AtomicU64::new(0),
        }
    }

    /// Generate the next unique entry id.
    pub fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Push a completed log entry into the ring buffer.
    pub fn push(&self, log: RequestLog) {
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= MAX_LOG_ENTRIES {
                entries.pop_front();
            }
            entries.push_back(log);
        }
    }

    /// Return up to `limit` most recent entries, newest-last.
    pub fn recent(&self, limit: usize) -> Vec<RequestLog> {
        if let Ok(entries) = self.entries.lock() {
            let skip = entries.len().saturating_sub(limit);
            entries.iter().skip(skip).cloned().collect()
        } else {
            vec![]
        }
    }

    /// Total entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// Clear all entries.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}
