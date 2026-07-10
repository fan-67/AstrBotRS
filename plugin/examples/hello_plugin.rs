use astrbot_platform::event::AstrMessageEvent;
use astrbot_platform::message_chain::MessageChain;
use astrbot_plugin::{Plugin, PluginManager};

struct HelloPlugin;

#[async_trait::async_trait]
impl Plugin for HelloPlugin {
    fn name(&self) -> &str {
        "hello_plugin"
    }

    fn description(&self) -> &str {
        "A simple plugin that responds to 'hello' messages"
    }

    async fn on_message(&self, event: &AstrMessageEvent) -> Option<MessageChain> {
        let text = event.get_message_str().to_lowercase();
        if text.contains("hello") {
            Some(MessageChain::text("Hello! How can I help you?"))
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() {
    let mut mgr = PluginManager::new();
    mgr.register(Box::new(HelloPlugin));
    mgr.initialize_all().await;

    println!("Registered {} plugin(s)", mgr.list().len());

    for p in mgr.list() {
        println!(" - {}: {}", p.name(), p.description());
    }
}
