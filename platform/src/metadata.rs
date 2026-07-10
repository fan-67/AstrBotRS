use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformMetadata {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub support_streaming: bool,
}

impl PlatformMetadata {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            display_name: display_name.into(),
            support_streaming: false,
        }
    }
}
