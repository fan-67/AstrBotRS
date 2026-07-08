use std::collections::HashMap;

use astrbot_utils::error::{AstrBotError, Result};
use tracing::info;

use crate::traits::Provider;

pub struct ProviderManager {
    inst_map: HashMap<String, Box<dyn Provider>>,
    default_provider_id: Option<String>,
}

impl ProviderManager {
    pub fn new() -> Self {
        Self {
            inst_map: HashMap::new(),
            default_provider_id: None,
        }
    }

    pub fn register(&mut self, id: String, provider: Box<dyn Provider>) {
        info!("Registering provider: {id}");
        if self.default_provider_id.is_none() {
            self.default_provider_id = Some(id.clone());
        }
        self.inst_map.insert(id, provider);
    }

    pub fn get_using_provider(&self) -> Option<&dyn Provider> {
        self.default_provider_id
            .as_ref()
            .and_then(|id| self.inst_map.get(id))
            .map(|p| p.as_ref())
    }

    pub fn get_provider_by_id(&self, id: &str) -> Option<&dyn Provider> {
        self.inst_map.get(id).map(|p| p.as_ref())
    }

    pub fn set_default_provider(&mut self, id: &str) -> Result<()> {
        if self.inst_map.contains_key(id) {
            self.default_provider_id = Some(id.to_string());
            Ok(())
        } else {
            Err(AstrBotError::NotFound(format!(
                "Provider {id} not found"
            )))
        }
    }

    pub fn list_providers(&self) -> Vec<(&str, &dyn Provider)> {
        self.inst_map
            .iter()
            .map(|(id, p)| (id.as_str(), p.as_ref()))
            .collect()
    }

    pub fn remove(&mut self, id: &str) {
        self.inst_map.remove(id);
        if self.default_provider_id.as_deref() == Some(id) {
            self.default_provider_id = self.inst_map.keys().next().cloned();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inst_map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inst_map.len()
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}
