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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_log(id: u64, server: &str, tool: &str) -> RequestLog {
        RequestLog {
            id,
            timestamp: Utc::now(),
            server: server.to_string(),
            tool: tool.to_string(),
            input: serde_json::json!({"arg": "value"}),
            status: RequestStatus::Success,
            latency_ms: 42,
            error: None,
            output: Some(serde_json::json!({"result": "ok"})),
        }
    }

    #[test]
    fn test_request_logger_new() {
        let logger = RequestLogger::new();
        assert!(logger.is_empty());
        assert_eq!(logger.len(), 0);
    }

    #[test]
    fn test_request_logger_default() {
        let logger = RequestLogger::default();
        assert!(logger.is_empty());
    }

    #[test]
    fn test_next_id_increments() {
        let logger = RequestLogger::new();
        assert_eq!(logger.next_id(), 0);
        assert_eq!(logger.next_id(), 1);
        assert_eq!(logger.next_id(), 2);
    }

    #[test]
    fn test_push_and_len() {
        let logger = RequestLogger::new();
        logger.push(sample_log(0, "server1", "tool_a"));
        assert_eq!(logger.len(), 1);
        assert!(!logger.is_empty());

        logger.push(sample_log(1, "server2", "tool_b"));
        assert_eq!(logger.len(), 2);
    }

    #[test]
    fn test_recent_returns_newest_last() {
        let logger = RequestLogger::new();
        logger.push(sample_log(0, "server1", "first"));
        logger.push(sample_log(1, "server2", "second"));
        logger.push(sample_log(2, "server3", "third"));

        let recent = logger.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].tool, "second");
        assert_eq!(recent[1].tool, "third");
    }

    #[test]
    fn test_recent_with_limit_larger_than_entries() {
        let logger = RequestLogger::new();
        logger.push(sample_log(0, "server1", "tool_a"));
        logger.push(sample_log(1, "server2", "tool_b"));

        let recent = logger.recent(100);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_clear() {
        let logger = RequestLogger::new();
        logger.push(sample_log(0, "server1", "tool_a"));
        logger.push(sample_log(1, "server2", "tool_b"));
        assert_eq!(logger.len(), 2);

        logger.clear();
        assert!(logger.is_empty());
        assert_eq!(logger.len(), 0);
    }

    #[test]
    fn test_ring_buffer_evicts_old_entries() {
        let logger = RequestLogger::new();

        // Push more than MAX_LOG_ENTRIES
        for i in 0..(MAX_LOG_ENTRIES + 10) {
            logger.push(sample_log(i as u64, "server", &format!("tool_{}", i)));
        }

        // Should be capped at MAX_LOG_ENTRIES
        assert_eq!(logger.len(), MAX_LOG_ENTRIES);

        // First entry should be evicted (tool_0 through tool_9 evicted)
        let recent = logger.recent(1);
        assert_eq!(recent[0].tool, format!("tool_{}", MAX_LOG_ENTRIES + 9));
    }

    #[test]
    fn test_request_status_serialization() {
        let success = RequestStatus::Success;
        let error = RequestStatus::Error;

        assert_eq!(serde_json::to_string(&success).unwrap(), "\"success\"");
        assert_eq!(serde_json::to_string(&error).unwrap(), "\"error\"");
    }

    #[test]
    fn test_request_log_serialization() {
        let log = sample_log(42, "github", "get_issues");
        let json = serde_json::to_string(&log).unwrap();

        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"server\":\"github\""));
        assert!(json.contains("\"tool\":\"get_issues\""));
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"latency_ms\":42"));
    }

    #[test]
    fn test_request_log_with_error() {
        let log = RequestLog {
            id: 1,
            timestamp: Utc::now(),
            server: "test-server".to_string(),
            tool: "failing_tool".to_string(),
            input: serde_json::json!({}),
            status: RequestStatus::Error,
            latency_ms: 100,
            error: Some("Something went wrong".to_string()),
            output: None,
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("\"status\":\"error\""));
        assert!(json.contains("Something went wrong"));
    }

    #[test]
    fn test_max_log_entries_constant() {
        assert_eq!(MAX_LOG_ENTRIES, 500);
    }
}
