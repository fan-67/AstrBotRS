use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tracing::Subscriber;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

pub struct LogBroker {
    tx: broadcast::Sender<LogEntry>,
    ring_buffer: Mutex<VecDeque<LogEntry>>,
    max_entries: usize,
}

impl LogBroker {
    pub fn new(max_entries: usize) -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self {
            tx,
            ring_buffer: Mutex::new(VecDeque::with_capacity(max_entries + 1)),
            max_entries,
        }
    }

    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut buf) = self.ring_buffer.lock() {
            if buf.len() >= self.max_entries {
                buf.pop_front();
            }
            buf.push_back(entry.clone());
        }
        let _ = self.tx.send(entry);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.tx.subscribe()
    }

    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        if let Ok(buf) = self.ring_buffer.lock() {
            let len = buf.len();
            buf.iter()
                .rev()
                .take(count.min(len))
                .cloned()
                .collect()
        } else {
            vec![]
        }
    }
}

impl Default for LogBroker {
    fn default() -> Self {
        Self::new(1000)
    }
}

pub struct LogLayer {
    broker: std::sync::Weak<LogBroker>,
}

impl LogLayer {
    pub fn new(broker: std::sync::Weak<LogBroker>) -> Self {
        Self { broker }
    }
}

impl<S: Subscriber> Layer<S> for LogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(broker) = self.broker.upgrade() {
            let timestamp = Utc::now();
            let meta = event.metadata();
            let level = meta.level().to_string();
            let target = meta.target().to_string();

            let mut message = String::new();
            let mut visitor = MessageVisitor(&mut message);
            event.record(&mut visitor);

            broker.push(LogEntry {
                timestamp,
                level,
                target,
                message,
            });
        }
    }
}

struct MessageVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push_str(&format!("{value:?}"));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        }
    }
}

pub fn init_logging(
    log_dir: &str,
    log_file_prefix: &str,
    broker: std::sync::Weak<LogBroker>,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_path = format!("{log_dir}/{log_file_prefix}.jsonl");
    let log_file = std::fs::File::create(&log_path)?;
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(log_file)
        .with_target(true)
        .with_level(true);

    let log_layer = LogLayer::new(broker);

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(log_layer)
        .with(tracing_subscriber::EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
