pub mod event;
pub mod manager;
pub mod message;
pub mod message_chain;
pub mod metadata;
pub mod sources;
pub mod traits;

pub use event::AstrMessageEvent;
pub use message::{AstrBotMessage, Group, MessageMember, MessageType};
pub use message_chain::{MessageChain, MessageComponent, Plain};
pub use metadata::PlatformMetadata;
pub use traits::Platform;
