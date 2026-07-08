use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plain {
    pub text: String,
}

impl Plain {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub url: Option<String>,
    pub path: Option<String>,
    pub base64: Option<String>,
}

impl Image {
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            url: Some(url.into()),
            path: None,
            base64: None,
        }
    }

    pub fn from_file(path: impl Into<String>) -> Self {
        Self {
            url: None,
            path: Some(path.into()),
            base64: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct At {
    pub user_id: String,
    pub nickname: Option<String>,
}

impl At {
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            nickname: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub message_id: String,
    pub sender_id: String,
    pub message_str: Option<String>,
}

impl Reply {
    pub fn new(message_id: impl Into<String>, sender_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            sender_id: sender_id.into(),
            message_str: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub name: String,
    pub file: String,
}

impl File {
    pub fn new(name: impl Into<String>, file: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            file: file.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub file: String,
    pub url: Option<String>,
}

impl Record {
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageComponent {
    Plain(Plain),
    Image(Image),
    File(File),
    Record(Record),
    At(At),
    Reply(Reply),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageChain {
    pub chain: Vec<MessageComponent>,
}

impl MessageChain {
    pub fn new() -> Self {
        Self { chain: vec![] }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            chain: vec![MessageComponent::Plain(Plain::new(text))],
        }
    }

    pub fn push(&mut self, component: MessageComponent) {
        self.chain.push(component);
    }

    pub fn get_plain_text(&self) -> String {
        self.chain
            .iter()
            .filter_map(|c| match c {
                MessageComponent::Plain(p) => Some(p.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }
}

impl From<String> for MessageChain {
    fn from(text: String) -> Self {
        Self::text(text)
    }
}

impl From<&str> for MessageChain {
    fn from(text: &str) -> Self {
        Self::text(text)
    }
}
