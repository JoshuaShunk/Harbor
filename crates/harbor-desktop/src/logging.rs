use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// A log entry emitted to the frontend via Tauri events.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// A tracing Layer that forwards gateway-related log events to the Tauri frontend.
pub struct TauriLogLayer {
    app_handle: AppHandle,
}

impl TauriLogLayer {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

/// Visitor that extracts fields from tracing events into a readable message.
struct MessageVisitor {
    message: String,
    fields: Vec<String>,
}

impl MessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            fields: Vec::new(),
        }
    }

    fn into_message(self) -> String {
        if self.fields.is_empty() {
            self.message
        } else if self.message.is_empty() {
            self.fields.join(" ")
        } else {
            format!("{} {}", self.message, self.fields.join(" "))
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            self.fields.push(format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }
}

impl<S: Subscriber> Layer<S> for TauriLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let target = metadata.target();

        // Only capture gateway-related logs
        if !target.starts_with("harbor_core::gateway") && !target.starts_with("harbor_desktop") {
            return;
        }

        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            level: metadata.level().to_string(),
            target: target.to_string(),
            message: visitor.into_message(),
        };

        let _ = self.app_handle.emit("gateway-log", &entry);
    }
}
