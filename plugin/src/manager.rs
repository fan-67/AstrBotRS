use astrbot_platform::event::AstrMessageEvent;
use astrbot_platform::message_chain::MessageChain;
use tracing::info;

use crate::traits::Plugin;

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        info!("Registering plugin: {}", plugin.name());
        self.plugins.push(plugin);
    }

    pub async fn dispatch_message(&self, event: &AstrMessageEvent) -> Option<MessageChain> {
        for plugin in &self.plugins {
            if let Some(reply) = plugin.on_message(event).await {
                return Some(reply);
            }
        }
        None
    }

    pub async fn initialize_all(&self) {
        for plugin in &self.plugins {
            if let Err(e) = plugin.initialize().await {
                tracing::error!("Plugin {} init failed: {e}", plugin.name());
            }
        }
    }

    pub fn list(&self) -> Vec<&dyn Plugin> {
        self.plugins.iter().map(|p| p.as_ref()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
