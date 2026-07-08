use std::collections::HashMap;

use astrbot_utils::error::{AstrBotError, Result};
use tokio::sync::RwLock;
use tracing::info;

use crate::traits::Provider;

pub struct ProviderManager {
    inst_map: RwLock<HashMap<String, Box<dyn Provider>>>,
    default_provider_id: RwLock<Option<String>>,
}

impl ProviderManager {
    pub fn new() -> Self {
        Self {
            inst_map: RwLock::new(HashMap::new()),
            default_provider_id: RwLock::new(None),
        }
    }

    pub async fn register(&self, id: String, provider: Box<dyn Provider>) {
        info!("Registering provider: {id}");
        let mut map = self.inst_map.write().await;
        if map.is_empty() {
            *self.default_provider_id.write().await = Some(id.clone());
        }
        map.insert(id, provider);
    }

    pub async fn get_using_provider(&self) -> Option<tokio::sync::RwLockReadGuard<'_, Box<dyn Provider>>> {
        let id = { self.default_provider_id.read().await.clone() };
        let id = id?;
        tokio::sync::RwLockReadGuard::try_map(self.inst_map.read().await, |map| {
            map.get(&id)
        })
        .ok()
    }

    pub async fn get_provider_by_id(&self, id: &str) -> Option<tokio::sync::RwLockReadGuard<'_, Box<dyn Provider>>> {
        tokio::sync::RwLockReadGuard::try_map(self.inst_map.read().await, |map| {
            map.get(id)
        })
        .ok()
    }

    pub async fn set_default_provider(&self, id: &str) -> Result<()> {
        if self.inst_map.read().await.contains_key(id) {
            *self.default_provider_id.write().await = Some(id.to_string());
            Ok(())
        } else {
            Err(AstrBotError::NotFound(format!(
                "Provider {id} not found"
            )))
        }
    }

    pub async fn list_providers(&self) -> Vec<(String, String)> {
        self.inst_map
            .read()
            .await.keys().map(|id| (id.clone(), "chat_completion".to_string()))
            .collect()
    }

    pub async fn remove(&self, id: &str) {
        let mut map = self.inst_map.write().await;
        map.remove(id);
        let mut default = self.default_provider_id.write().await;
        if default.as_deref() == Some(id) {
            *default = map.keys().next().cloned();
        }
    }

    pub async fn len(&self) -> usize {
        self.inst_map.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inst_map.read().await.len() == 0
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}
