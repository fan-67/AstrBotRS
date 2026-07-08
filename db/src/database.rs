use astrbot_utils::error::{AstrBotError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::models::Conversation;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(path: &str) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| AstrBotError::Database(format!("Failed to connect to database: {e}")))?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                umo TEXT NOT NULL UNIQUE,
                messages_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AstrBotError::Database(format!("Migration failed: {e}")))?;

        Ok(())
    }

    pub async fn get_conversation(&self, umo: &str) -> Result<Option<Conversation>> {
        let result = sqlx::query_as::<_, Conversation>(
            "SELECT id, umo, messages_json, created_at, updated_at FROM conversations WHERE umo = ?",
        )
        .bind(umo)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AstrBotError::Database(format!("Query failed: {e}")))?;

        Ok(result)
    }

    pub async fn upsert_conversation(
        &self,
        umo: &str,
        messages_json: &str,
    ) -> Result<Conversation> {
        sqlx::query(
            "INSERT INTO conversations (umo, messages_json) VALUES (?, ?) \
             ON CONFLICT(umo) DO UPDATE SET messages_json = excluded.messages_json, updated_at = datetime('now')",
        )
        .bind(umo)
        .bind(messages_json)
        .execute(&self.pool)
        .await
        .map_err(|e| AstrBotError::Database(format!("Upsert failed: {e}")))?;

        self.get_conversation(umo)
            .await?
            .ok_or_else(|| AstrBotError::Database("Conversation not found after upsert".to_string()))
    }

    pub async fn delete_conversation(&self, umo: &str) -> Result<()> {
        sqlx::query("DELETE FROM conversations WHERE umo = ?")
            .bind(umo)
            .execute(&self.pool)
            .await
            .map_err(|e| AstrBotError::Database(format!("Delete failed: {e}")))?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
