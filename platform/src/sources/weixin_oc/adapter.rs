use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client as HttpClient;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::client::WeixinOCClient;
use super::crypto;
use crate::event::AstrMessageEvent;
use crate::message::{AstrBotMessage, Group, MessageMember, MessageType};
use crate::message_chain::MessageChain;
use crate::message_chain::MessageComponent;
use crate::metadata::PlatformMetadata;
use crate::traits::Platform;

struct WeixinOCState {
    token: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedMessage {
    message_id: String,
    sender_id: String,
    text: String,
    timestamp: i64,
}

struct RecentMessageCache {
    sessions: HashMap<String, VecDeque<CachedMessage>>,
    max_cache_size: usize,
    max_sessions: usize,
}

impl RecentMessageCache {
    fn new(max_cache_size: usize, max_sessions: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            max_cache_size,
            max_sessions,
        }
    }

    fn add(&mut self, session_id: String, msg: CachedMessage) {
        let entry = self.sessions.entry(session_id).or_default();
        if entry.len() >= self.max_cache_size {
            entry.pop_front();
        }
        entry.push_back(msg);
        self.prune_sessions();
    }

    fn find_reply(&self, session_id: &str, ref_msg_id: &str) -> Option<&CachedMessage> {
        self.sessions
            .get(session_id)?
            .iter()
            .find(|m| m.message_id == ref_msg_id)
    }

    fn prune_sessions(&mut self) {
        if self.sessions.len() > self.max_sessions {
            let mut pairs: Vec<_> = self.sessions.drain().collect();
            pairs.sort_by_key(|(_, msgs)| msgs.back().map(|m| m.timestamp).unwrap_or(0));
            pairs.truncate(self.max_sessions);
            self.sessions = pairs.into_iter().collect();
        }
    }
}

pub struct WeixinOCAdapter {
    meta: PlatformMetadata,
    state: Arc<Mutex<WeixinOCState>>,
    http: HttpClient,
    base_url: String,
    cdn_base_url: String,
    event_tx: mpsc::UnboundedSender<AstrMessageEvent>,
    seen_message_ids: Mutex<std::collections::HashSet<String>>,
    cached_messages: Arc<Mutex<RecentMessageCache>>,
    context_tokens: Mutex<HashMap<String, String>>,
    sync_buf_path: String,
    sync_buf: Mutex<String>,
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
        let sync_buf_path = format!("data/weixin_oc_sync_buf_{}.dat", id);
        let initial_sync_buf = std::fs::read_to_string(&sync_buf_path).unwrap_or_default();
        Self {
            meta: PlatformMetadata::new(&id, "weixin_oc", "个人微信"),
            state: Arc::new(Mutex::new(WeixinOCState { token })),
            http: HttpClient::new(),
            base_url: base_url.into(),
            cdn_base_url: cdn_base_url.into(),
            event_tx,
            seen_message_ids: Mutex::new(std::collections::HashSet::new()),
            cached_messages: Arc::new(Mutex::new(RecentMessageCache::new(100, 500))),
            context_tokens: Mutex::new(HashMap::new()),
            sync_buf_path,
            sync_buf: Mutex::new(initial_sync_buf),
        }
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
            .get("msgs")
            .and_then(|v| v.as_array());

        let Some(msg_list) = msg_list else {
            return messages;
        };

        for msg in msg_list {
            let server_msg_id = msg
                .get("msg_id")
                .or_else(|| msg.get("message_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let mut dedup = self.seen_message_ids.lock().unwrap();
            if !dedup.insert(server_msg_id.clone()) {
                continue;
            }
            if dedup.len() > 10000 {
                // Remove oldest half to avoid replay flood
                let remove_count = dedup.len() / 2;
                let keys: Vec<String> = dedup.iter().take(remove_count).cloned().collect();
                for k in keys { dedup.remove(&k); }
            }
            drop(dedup);

            let from_user_id = msg
                .pointer("/from_user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let from_nickname = msg
                .pointer("/from_nickname")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Save context_token for this session so send_message can include it
            if let Some(token) = msg.get("context_token").and_then(|v| v.as_str()) {
                if !token.is_empty() && !from_user_id.is_empty() {
                    self.context_tokens.lock().unwrap()
                        .insert(from_user_id.to_string(), token.to_string());
                }
            }

            let group_id = msg
                .get("from_group_id")
                .or_else(|| msg.get("group_id"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());

            let ref_msg_id = msg.get("ref_msg").and_then(|v| {
                v.as_str()
                    .or_else(|| v.get("msg_id").and_then(|id| id.as_str()))
            }).filter(|s| !s.is_empty());

            let item_list = msg
                .pointer("/item_list")
                .and_then(|v| v.as_array());

            let Some(item_list) = item_list else {
                continue;
            };

            let all_texts: Vec<String> = item_list
                .iter()
                .filter_map(Self::parse_message_item)
                .map(|(_, text)| text)
                .collect();

            let combined_text = all_texts.join(" ");
            let session_key = group_id.unwrap_or(from_user_id);

            let reply_context = ref_msg_id.and_then(|ref_id| {
                self.cached_messages
                    .lock()
                    .ok()
                    .and_then(|cache| cache.find_reply(session_key, ref_id).map(|m| m.text.clone()))
            });

            if !combined_text.is_empty()
                && let Ok(mut cache) = self.cached_messages.lock() {
                    cache.add(
                        session_key.to_string(),
                        CachedMessage {
                            message_id: server_msg_id.clone(),
                            sender_id: from_user_id.to_string(),
                            text: combined_text.clone(),
                            timestamp: chrono::Utc::now().timestamp(),
                        },
                    );
                }

            let message_type = if group_id.is_some() {
                MessageType::GroupMessage
            } else {
                MessageType::FriendMessage
            };

            let final_text = if let Some(ref ctx) = reply_context {
                format!("[回复: {}] {}", ctx, combined_text)
            } else {
                combined_text
            };

            let mut bot_msg = AstrBotMessage::new(
                message_type,
                &self.meta.id,
                session_key.to_string(),
                server_msg_id.clone(),
                MessageMember::new(from_user_id),
                final_text,
            );
            bot_msg.sender.nickname = Some(from_nickname.to_string());

            if let Some(gid) = group_id {
                bot_msg.group = Some(Group::new(gid));
            }

            messages.push(bot_msg);
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

    async fn sync_loop(&self) -> Result<(), String> {
        let mut sync_buf = self.sync_buf.lock().unwrap().clone();

        loop {
            let client = self.client();

            match client
                .request_json(
                    "POST",
                    "ilink/bot/getupdates",
                    None,
                    Some(serde_json::json!({
                        "base_info": {
                            "channel_version": "astrbot"
                        },
                        "get_updates_buf": sync_buf
                    })),
                    true,
                    Some(35_000),
                )
                .await
            {
                Ok(data) => {
                    let errcode = data.get("errcode").and_then(|v| v.as_i64()).unwrap_or(0);
                    if errcode == -14 {
                        return Err("session timeout".to_string());
                    }
                    if let Some(new_buf) = data.get("get_updates_buf").and_then(|v| v.as_str()) {
                        sync_buf = new_buf.to_string();
                        if let Err(e) = std::fs::write(&self.sync_buf_path, &sync_buf) {
                            error!("weixin_oc: failed to persist sync_buf: {e}");
                        }
                        *self.sync_buf.lock().unwrap() = sync_buf.clone();
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
                    if e.contains("-14") {
                        return Err(format!("session expired: {e}"));
                    }
                    error!("weixin_oc: sync error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn prepare_media_item(
        &self,
        client: &WeixinOCClient,
        user_id: &str,
        file_path: &str,
        upload_media_type: i64,
        item_type: i64,
        file_name: String,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let data = std::fs::read(file_path)
            .map_err(|e| format!("read media file {file_path} failed: {e}"))?;
        let raw_size = data.len();
        let raw_md5 = format!("{:x}", md5::compute(&data));
        let file_key = uuid::Uuid::new_v4().to_string().replace("-", "");
        let aes_key_bytes = uuid::Uuid::new_v4().into_bytes();
        let aes_key_hex = hex::encode(aes_key_bytes);
        let ciphertext_size = crypto::aes_padded_size(raw_size);

        let getupload_payload = serde_json::json!({
            "filekey": file_key,
            "media_type": upload_media_type,
            "to_user_id": user_id,
            "rawsize": raw_size,
            "rawfilemd5": raw_md5,
            "filesize": ciphertext_size,
            "no_need_thumb": true,
            "aeskey": aes_key_hex,
            "base_info": {
                "channel_version": "astrbot"
            }
        });

        let resp = client
            .request_json(
                "POST",
                "ilink/bot/getuploadurl",
                None,
                Some(getupload_payload),
                true,
                Some(120_000),
            )
            .await
            .map_err(|e| format!("getuploadurl failed: {e}"))?;

        let upload_param = resp
            .get("upload_param")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing upload_param".to_string())?;
        let upload_full_url = resp
            .get("upload_full_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let encrypt_query_param = client
            .upload_to_cdn(
                upload_full_url,
                upload_param,
                &file_key,
                &aes_key_hex,
                &data,
            )
            .await
            .map_err(|e| format!("cdn upload failed: {e}"))?;

        let aes_key_b64 =
            base64::engine::general_purpose::STANDARD.encode(aes_key_hex.as_bytes());

        let media_payload = serde_json::json!({
            "encrypt_query_param": encrypt_query_param,
            "aes_key": aes_key_b64,
            "encrypt_type": 1,
        });

        let item = match item_type {
            2 => serde_json::json!({
                "type": 2,
                "image_item": {
                    "media": media_payload,
                    "mid_size": ciphertext_size,
                }
            }),
            3 => serde_json::json!({
                "type": 3,
                "voice_item": {
                    "media": media_payload,
                    "voice_size": ciphertext_size,
                }
            }),
            4 => serde_json::json!({
                "type": 4,
                "file_item": {
                    "media": media_payload,
                    "file_name": file_name,
                    "len": raw_size.to_string(),
                }
            }),
            _ => return Err(format!("unsupported item type: {item_type}").into()),
        };

        Ok(item)
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
                Ok(()) => info!("weixin_oc: sync loop ended cleanly"),
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
        let client = self.client();
        let mut item_list: Vec<Value> = Vec::new();
        let mut pending_text = String::new();
        let mut temp_files: Vec<String> = Vec::new();

        for component in &message.chain {
            match component {
                MessageComponent::Plain(p) => {
                    pending_text.push_str(&p.text);
                }
                MessageComponent::Image(img) => {
                    if let Some(ref path) = img.path {
                        if !pending_text.is_empty() {
                            item_list.push(serde_json::json!({
                                "type": 1,
                                "text_item": { "text": pending_text }
                            }));
                            pending_text.clear();
                        }
                        temp_files.push(path.clone());
                        let media_item = self
                            .prepare_media_item(
                                &client,
                                session_id,
                                path,
                                1,
                                2,
                                String::new(),
                            )
                            .await?;
                        item_list.push(media_item);
                    } else {
                        warn!("weixin_oc: image without path, skipping");
                    }
                }
                MessageComponent::File(file) => {
                    if !pending_text.is_empty() {
                        item_list.push(serde_json::json!({
                            "type": 1,
                            "text_item": { "text": pending_text }
                        }));
                        pending_text.clear();
                    }
                    let file_name = if file.name.is_empty() {
                        std::path::Path::new(&file.file)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".to_string())
                    } else {
                        file.name.clone()
                    };
                    temp_files.push(file.file.clone());
                    let media_item = self
                        .prepare_media_item(
                            &client,
                            session_id,
                            &file.file,
                            3,
                            4,
                            file_name,
                        )
                        .await?;
                    item_list.push(media_item);
                }
                MessageComponent::Record(record) => {
                    if !pending_text.is_empty() {
                        item_list.push(serde_json::json!({
                            "type": 1,
                            "text_item": { "text": pending_text }
                        }));
                        pending_text.clear();
                    }
                    temp_files.push(record.file.clone());
                    let media_item = self
                        .prepare_media_item(
                            &client,
                            session_id,
                            &record.file,
                            1,
                            3,
                            String::new(),
                        )
                        .await?;
                    item_list.push(media_item);
                }
                _ => {}
            }
        }

        if !pending_text.is_empty() {
            item_list.push(serde_json::json!({
                "type": 1,
                "text_item": { "text": pending_text }
            }));
        }

        if item_list.is_empty() {
            warn!("weixin_oc: no items to send, ignoring");
            return Ok(());
        }

        let mut msg_payload = serde_json::json!({
            "base_info": { "channel_version": "astrbot" },
            "msg": {
                "from_user_id": "",
                "to_user_id": session_id,
                "client_id": uuid::Uuid::new_v4().to_string(),
                "message_type": 2,
                "message_state": 2,
                "item_list": item_list,
            }
        });

        if let Ok(tokens) = self.context_tokens.lock() {
            if let Some(token) = tokens.get(session_id) {
                msg_payload["msg"]["context_token"] = serde_json::Value::String(token.clone());
            }
        }

        client
            .request_json(
                "POST",
                "ilink/bot/sendmessage",
                None,
                Some(msg_payload),
                true,
                Some(30_000),
            )
            .await
            .map_err(|e| format!("send message failed: {e}"))?;

        // Clean up temp media files
        for f in &temp_files {
            if std::path::Path::new(f).exists() {
                let _ = std::fs::remove_file(f);
            }
        }

        info!("weixin_oc: sent message to {session_id}");
        Ok(())
    }

    fn commit_event(&self, event: AstrMessageEvent) {
        let _ = self.event_tx.send(event);
    }

    async fn start_typing(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let context_token = self.context_tokens.lock().unwrap().get(session_id).cloned();
        let Some(context_token) = context_token else {
            debug!("weixin_oc: no context_token for {session_id}, skipping typing");
            return Ok(());
        };

        let client = self.client();
        let resp = client.get_typing_config(session_id, &context_token).await
            .map_err(|e| format!("get_typing_config failed: {e}"))?;

        let ticket = resp.get("typing_ticket").and_then(|v| v.as_str())
            .ok_or_else(|| "missing typing_ticket".to_string())?;

        client.send_typing_state(session_id, ticket, false).await
            .map_err(|e| format!("send_typing_state failed: {e}"))?;

        info!("weixin_oc: typing started for {session_id}");
        Ok(())
    }

    async fn stop_typing(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let context_token = self.context_tokens.lock().unwrap().get(session_id).cloned();
        let Some(context_token) = context_token else {
            return Ok(());
        };

        let client = self.client();
        let resp = client.get_typing_config(session_id, &context_token).await
            .map_err(|e| format!("get_typing_config failed: {e}"))?;

        if let Some(ticket) = resp.get("typing_ticket").and_then(|v| v.as_str()) {
            let _ = client.send_typing_state(session_id, ticket, true).await;
        }

        Ok(())
    }

    async fn terminate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("weixin_oc: terminating");
        Ok(())
    }
}
