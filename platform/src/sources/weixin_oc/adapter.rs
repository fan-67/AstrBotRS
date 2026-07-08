use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::client::WeixinOCClient;
use crate::event::AstrMessageEvent;
use crate::message::{AstrBotMessage, MessageMember, MessageType};
use crate::message_chain::MessageChain;
use crate::metadata::PlatformMetadata;
use crate::traits::Platform;

struct WeixinOCState {
    token: Option<String>,
}

pub struct WeixinOCAdapter {
    meta: PlatformMetadata,
    state: Arc<Mutex<WeixinOCState>>,
    http: HttpClient,
    base_url: String,
    cdn_base_url: String,
    event_tx: mpsc::UnboundedSender<AstrMessageEvent>,
}

impl WeixinOCAdapter {
    pub fn new(
        id: impl Into<String>,
        base_url: impl Into<String>,
        cdn_base_url: impl Into<String>,
        token: Option<String>,
        event_tx: mpsc::UnboundedSender<AstrMessageEvent>,
    ) -> Self {
        let id = id.into();
        Self {
            meta: PlatformMetadata::new(&id, "weixin_oc", "个人微信"),
            state: Arc::new(Mutex::new(WeixinOCState { token })),
            http: HttpClient::new(),
            base_url: base_url.into(),
            cdn_base_url: cdn_base_url.into(),
            event_tx,
        }
    }

    fn client(&self) -> WeixinOCClient {
        let token = self.state.lock().unwrap().token.clone();
        WeixinOCClient::with_http(
            self.http.clone(),
            &self.base_url,
            &self.cdn_base_url,
            120_000,
            token,
        )
    }

    fn parse_message_item(item: &Value) -> Option<(String, String)> {
        let item_type = item.get("type").and_then(|v| v.as_i64()).unwrap_or(0);
        match item_type {
            1 => {
                let text = item
                    .pointer("/text_item/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Some(("text".to_string(), text.to_string()))
            }
            2 => Some(("image".to_string(), "[图片]".to_string())),
            3 => {
                let text = item
                    .pointer("/voice_item/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("[语音]");
                Some(("voice".to_string(), text.to_string()))
            }
            4 => Some(("file".to_string(), "[文件]".to_string())),
            5 => Some(("video".to_string(), "[视频]".to_string())),
            _ => None,
        }
    }

    fn parse_sync_response(&self, data: &Value) -> Vec<AstrBotMessage> {
        let mut messages = Vec::new();
        let msg_list = data
            .pointer("/msg_page/message_list")
            .and_then(|v| v.as_array());

        let Some(msg_list) = msg_list else {
            return messages;
        };

        for msg in msg_list {
            let from_user_id = msg
                .pointer("/from_user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let from_nickname = msg
                .pointer("/from_nickname")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let item_list = msg
                .pointer("/item_list")
                .and_then(|v| v.as_array());

            let Some(item_list) = item_list else {
                continue;
            };

            for item in item_list {
                if let Some((_kind, text)) = Self::parse_message_item(item) {
                    let session_id = from_user_id.to_string();
                    let message_id = uuid::Uuid::new_v4().to_string();

                    let mut bot_msg = AstrBotMessage::new(
                        MessageType::FriendMessage,
                        &self.meta.id,
                        session_id,
                        message_id,
                        MessageMember::new(from_user_id),
                        text,
                    );
                    bot_msg.sender.nickname = Some(from_nickname.to_string());
                    messages.push(bot_msg);
                }
            }
        }

        messages
    }

    async fn login(&self) -> Result<(), String> {
        info!("weixin_oc: starting QR code login");

        let client = self.client();

        let resp = client
            .request_json(
                "GET",
                "ilink/bot/get_bot_qrcode",
                Some(&std::collections::HashMap::from([(
                    "bot_type".to_string(),
                    "3".to_string(),
                )])),
                None,
                false,
                Some(15_000),
            )
            .await?;

        let qrcode = resp
            .get("qrcode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing qrcode".to_string())?;
        let qrcode_url = resp
            .get("qrcode_img_content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing qrcode_img_content".to_string())?;

        info!("weixin_oc: QR code URL: {qrcode_url}");
        info!("weixin_oc: 请使用手机微信扫码登录");

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let status_resp = client
                .request_json(
                    "GET",
                    "ilink/bot/get_qrcode_status",
                    Some(&std::collections::HashMap::from([(
                        "qrcode".to_string(),
                        qrcode.to_string(),
                    )])),
                    None,
                    false,
                    Some(35_000),
                )
                .await?;

            let status = status_resp
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("wait");

            match status {
                "confirmed" => {
                    let bot_token = status_resp
                        .get("bot_token")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| "missing bot_token".to_string())?;
                    info!("weixin_oc: login confirmed!");
                    if let Ok(mut state) = self.state.lock() {
                        state.token = Some(bot_token.to_string());
                    }
                    return Ok(());
                }
                "expired" => {
                    warn!("weixin_oc: QR code expired");
                    return Err("qr expired".to_string());
                }
                _ => {
                    debug!("weixin_oc: waiting for QR scan...");
                }
            }
        }
    }

    async fn sync_loop(&self) -> Result<(), String> {
        let mut sync_buf = String::new();

        loop {
            let client = self.client();
            let mut params = std::collections::HashMap::new();
            if !sync_buf.is_empty() {
                params.insert("sync_buf".to_string(), sync_buf.clone());
            }

            match client
                .request_json(
                    "GET",
                    "ilink/bot/syncv2",
                    Some(&params),
                    None,
                    true,
                    Some(35_000),
                )
                .await
            {
                Ok(data) => {
                    if let Some(new_buf) = data.get("sync_buf").and_then(|v| v.as_str()) {
                        sync_buf = new_buf.to_string();
                    }

                    // Check for session timeout
                    let errcode = data.get("errcode").and_then(|v| v.as_i64()).unwrap_or(0);
                    if errcode == -14 {
                        return Err("session timeout".to_string());
                    }

                    let messages = self.parse_sync_response(&data);
                    for msg in messages {
                        let session_id = msg.session_id.clone();
                        let event = AstrMessageEvent::new(msg, self.meta.clone(), session_id);
                        if self.event_tx.send(event).is_err() {
                            return Err("event channel closed".to_string());
                        }
                    }
                }
                Err(e) => {
                    if e.contains("-14") || e.contains("session") {
                        return Err(format!("session expired: {e}"));
                    }
                    error!("weixin_oc: sync error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[async_trait]
impl Platform for WeixinOCAdapter {
    fn meta(&self) -> PlatformMetadata {
        self.meta.clone()
    }

    async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            if self.state.lock().unwrap().token.is_none() {
                match self.login().await {
                    Ok(()) => info!("weixin_oc: login successful"),
                    Err(e) => {
                        error!("weixin_oc: login failed: {e}, retrying in 10s");
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                        continue;
                    }
                }
            }

            info!("weixin_oc: starting sync loop");
            match self.sync_loop().await {
                Ok(()) => {
                    info!("weixin_oc: sync loop ended cleanly");
                }
                Err(e) => {
                    warn!("weixin_oc: sync loop ended: {e}, re-logging in...");
                    if let Ok(mut state) = self.state.lock() {
                        state.token = None;
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    async fn send_message(
        &self,
        session_id: &str,
        message: MessageChain,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let text = message.get_plain_text();
        if text.is_empty() {
            return Ok(());
        }

        let client = self.client();
        let payload = serde_json::json!({
            "base_info": {
                "channel_version": "astrbot"
            },
            "msg": {
                "from_user_id": "",
                "to_user_id": session_id,
                "client_id": uuid::Uuid::new_v4().to_string(),
                "message_type": 2,
                "message_state": 2,
                "item_list": [
                    {
                        "type": 1,
                        "text_item": {
                            "text": text
                        }
                    }
                ]
            }
        });

        client
            .request_json(
                "POST",
                "ilink/bot/sendmessage",
                None,
                Some(payload),
                true,
                None,
            )
            .await
            .map_err(|e| format!("send message failed: {e}"))?;

        info!("weixin_oc: sent message to {session_id}");
        Ok(())
    }

    fn commit_event(&self, event: AstrMessageEvent) {
        let _ = self.event_tx.send(event);
    }

    async fn terminate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("weixin_oc: terminating");
        Ok(())
    }
}
