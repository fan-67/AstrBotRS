use crate::message::AstrBotMessage;
use crate::metadata::PlatformMetadata;

#[derive(Debug, Clone)]
pub struct AstrMessageEvent {
    pub message_str: String,
    pub message_obj: AstrBotMessage,
    pub platform_meta: PlatformMetadata,
    pub session_id: String,
    pub role: String,
    pub is_wake: bool,
    pub unified_msg_origin: String,
}

impl AstrMessageEvent {
    pub fn new(
        message_obj: AstrBotMessage,
        platform_meta: PlatformMetadata,
        session_id: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let unified_msg_origin = format!(
            "{}:{}:{}",
            platform_meta.id,
            message_obj.message_type.as_str(),
            session_id
        );
        Self {
            message_str: message_obj.message_str.clone(),
            message_obj,
            platform_meta,
            session_id: session_id.clone(),
            role: "member".to_string(),
            is_wake: false,
            unified_msg_origin,
        }
    }

    pub fn get_platform_name(&self) -> &str {
        &self.platform_meta.name
    }

    pub fn get_platform_id(&self) -> &str {
        &self.platform_meta.id
    }

    pub fn get_message_str(&self) -> &str {
        &self.message_str
    }

    pub fn get_sender_id(&self) -> &str {
        &self.message_obj.sender.user_id
    }

    pub fn get_sender_name(&self) -> &str {
        self.message_obj
            .sender
            .nickname
            .as_deref()
            .unwrap_or("")
    }

    pub fn is_private(&self) -> bool {
        self.message_obj.message_type == crate::message::MessageType::FriendMessage
    }

    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}
