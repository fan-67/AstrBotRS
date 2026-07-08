use std::collections::HashSet;
use std::sync::Arc;

use astrbot_platform::AstrMessageEvent;
use tokio::sync::{Mutex, mpsc};
use tracing::info;

use crate::pipeline::Pipeline;

pub struct EventBus {
    rx: mpsc::UnboundedReceiver<AstrMessageEvent>,
    pipeline: Pipeline,
    pending: Arc<Mutex<HashSet<String>>>,
}

impl EventBus {
    pub fn new(rx: mpsc::UnboundedReceiver<AstrMessageEvent>, pipeline: Pipeline) -> Self {
        Self {
            rx,
            pipeline,
            pending: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn dispatch(mut self) {
        while let Some(event) = self.rx.recv().await {
            let message_id = event.message_obj.message_id.clone();
            info!(
                "[{}] {}: {}",
                event.get_platform_name(),
                event.get_sender_name(),
                event.get_message_str()
            );

            {
                let mut pending = self.pending.lock().await;
                if !pending.insert(message_id.clone()) {
                    continue;
                }
            }

            let pipeline = self.pipeline.clone();
            let pending = self.pending.clone();
            let mid = message_id.clone();
            tokio::spawn(async move {
                pipeline.execute(event).await;
                pending.lock().await.remove(&mid);
            });
        }
        info!("EventBus: dispatch loop ended");
    }
}
