use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Conversation {
    pub id: i64,
    pub umo: String,
    pub messages_json: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
