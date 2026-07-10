use std::collections::HashSet;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::client::EcsBridgeClient;
use crate::event::AstrMessageEvent;
use crate::message::{AstrBotMessage, Group, MessageMember, MessageType};
use crate::message_chain::MessageChain;
use crate::metadata::PlatformMetadata;
use crate::traits::Platform;

pub struct EcsBridgeAdapter {
    meta: PlatformMetadata,
    ws_url: String,
    event_tx: mpsc::UnboundedSender<AstrMessageEvent>,
    seen_ids: Mutex<HashSet<String>>,
}

impl EcsBridgeAdapter {
    pub fn new(
        id: impl Into<String>,
        ws_url: impl Into<String>,
        event_tx: mpsc::UnboundedSender<AstrMessageEvent>,
    ) -> Self {
        let id = id.into();
        Self {
            meta: PlatformMetadata::new(&id, "ecs_bridge", "微信"),
            ws_url: ws_url.into(),
            event_tx,
            seen_ids: Mutex::new(HashSet::new()),
        }
    }

    fn parse_incoming(&self, json: &Value) -> Option<AstrMessageEvent> {
        let msg_type = json.get("type").and_then(|v| v.as_str())?;
        if msg_type != "message" {
            return None;
        }

        let data = json.get("data")?;
        let session_id = data.get("session_id").and_then(|v| v.as_str())?;
        let sender_id = data.get("sender_id").and_then(|v| v.as_str()).unwrap_or("");
        let sender_name = data
            .get("sender_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let message_id = data
            .get("message_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let text = data.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let is_group = data
            .get("message_type")
            .and_then(|v| v.as_str())
            .map(|t| t == "group")
            .unwrap_or(false);
        let group_id = data
            .get("group_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        // Dedup
        if !message_id.is_empty() {
            if let Ok(mut seen) = self.seen_ids.lock() {
                if !seen.insert(message_id.to_string()) {
                    return None;
                }
                if seen.len() > 10000 {
                    let remove: Vec<String> = seen.iter().take(5000).cloned().collect();
                    for k in remove {
                        seen.remove(&k);
                    }
                }
            }
        }

        let message_type = if is_group {
            MessageType::GroupMessage
        } else {
            MessageType::FriendMessage
        };

        let mut bot_msg = AstrBotMessage::new(
            message_type,
            &self.meta.id,
            session_id,
            message_id,
            MessageMember::new(sender_id),
            text,
        );
        bot_msg.sender.nickname = Some(sender_name.to_string());

        if let Some(gid) = group_id {
            bot_msg.group = Some(Group::new(gid));
        }

        Some(AstrMessageEvent::new(
            bot_msg,
            self.meta.clone(),
            session_id,
        ))
    }

    async fn run_loop(&self) -> Result<(), String> {
        let (client, mut msg_rx) = EcsBridgeClient::connect(&self.ws_url).await?;
        info!("ecs_bridge: connected to {url}", url = self.ws_url);

        // Heartbeat ping every 30s
        let ping_client = client;
        let _ping_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                if ping_client.ping().is_err() {
                    break;
                }
            }
        });

        loop {
            tokio::select! {
                Some(json) = msg_rx.recv() => {
                    if let Some(event) = self.parse_incoming(&json) {
                        info!(
                            "ecs_bridge: [{}] {}: {}",
                            event.get_platform_name(),
                            event.get_sender_name(),
                            event.get_message_str()
                        );
                        if self.event_tx.send(event).is_err() {
                            return Err("event channel closed".to_string());
                        }
                    }
                }
                else => {
                    return Err("message stream ended".to_string());
                }
            }
        }
    }
}

#[async_trait]
impl Platform for EcsBridgeAdapter {
    fn meta(&self) -> PlatformMetadata {
        self.meta.clone()
    }

    async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut backoff: u64 = 1;
        loop {
            match self.run_loop().await {
                Ok(()) => {
                    info!("ecs_bridge: run loop ended cleanly");
                    backoff = 1;
                }
                Err(e) => {
                    warn!("ecs_bridge: run error: {e}, reconnecting in {backoff}s");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    }

    async fn send_message(
        &self,
        session_id: &str,
        message: MessageChain,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let text = message.get_plain_text();
        if text.is_empty() {
            warn!("ecs_bridge: empty message, ignoring");
            return Ok(());
        }

        let outgoing = serde_json::json!({
            "type": "send_message",
            "data": {
                "session_id": session_id,
                "text": text,
            }
        });

        // We need a temporary connection to send
        let (client, _) = EcsBridgeClient::connect(&self.ws_url)
            .await
            .map_err(|e| format!("send: connect failed: {e}"))?;
        client
            .send(outgoing)
            .map_err(|e| format!("send: write failed: {e}"))?;

        info!("ecs_bridge: sent message to {session_id}");
        Ok(())
    }

    fn commit_event(&self, event: AstrMessageEvent) {
        let _ = self.event_tx.send(event);
    }
}
