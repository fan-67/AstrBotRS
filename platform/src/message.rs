use serde::{Deserialize, Serialize};

use crate::message_chain::MessageChain;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    FriendMessage,
    GroupMessage,
    ChannelMessage,
    Unknown,
}

impl MessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageType::FriendMessage => "friend",
            MessageType::GroupMessage => "group",
            MessageType::ChannelMessage => "channel",
            MessageType::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "friend" => MessageType::FriendMessage,
            "group" => MessageType::GroupMessage,
            "channel" => MessageType::ChannelMessage,
            _ => MessageType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMember {
    pub user_id: String,
    pub nickname: Option<String>,
}

impl MessageMember {
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            nickname: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub group_id: String,
    pub group_name: Option<String>,
}

impl Group {
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            group_name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstrBotMessage {
    pub message_type: MessageType,
    pub self_id: String,
    pub session_id: String,
    pub message_id: String,
    pub sender: MessageMember,
    pub group: Option<Group>,
    pub message: MessageChain,
    pub message_str: String,
    pub timestamp: i64,
}

impl AstrBotMessage {
    pub fn new(
        message_type: MessageType,
        self_id: impl Into<String>,
        session_id: impl Into<String>,
        message_id: impl Into<String>,
        sender: MessageMember,
        message_str: impl Into<String>,
    ) -> Self {
        Self {
            message_type,
            self_id: self_id.into(),
            session_id: session_id.into(),
            message_id: message_id.into(),
            sender,
            group: None,
            message: MessageChain::new(),
            message_str: message_str.into(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
