use crate::event::AstrMessageEvent;
use crate::message::AstrBotMessage;
use crate::metadata::PlatformMetadata;

pub struct WeixinOCMessageEvent {
    pub inner: AstrMessageEvent,
}

impl WeixinOCMessageEvent {
    pub fn new(message: AstrBotMessage, platform_meta: PlatformMetadata) -> Self {
        let session_id = message.session_id.clone();
        Self {
            inner: AstrMessageEvent::new(message, platform_meta, session_id),
        }
    }
}
