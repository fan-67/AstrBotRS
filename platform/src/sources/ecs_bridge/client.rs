use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

pub enum WsCommand {
    Send(Value),
    Ping,
    Close,
}

pub struct EcsBridgeClient {
    cmd_tx: mpsc::UnboundedSender<WsCommand>,
}

impl EcsBridgeClient {
    pub async fn connect(url: &str) -> Result<(Self, mpsc::UnboundedReceiver<Value>), String> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| format!("WebSocket connect failed: {e}"))?;

        let (mut write, mut read) = ws_stream.split();
        let (cmd_tx, mut cmd_rx): (mpsc::UnboundedSender<WsCommand>, _) = mpsc::unbounded_channel();
        let (msg_tx, msg_rx): (mpsc::UnboundedSender<Value>, _) = mpsc::unbounded_channel();

        // Reader task
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("ecs_bridge: received text: {text}");
                        match serde_json::from_str::<Value>(&text) {
                            Ok(json) => {
                                if msg_tx.send(json).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("ecs_bridge: invalid JSON: {e}");
                            }
                        }
                    }
                    Ok(Message::Pong(_)) => {
                        debug!("ecs_bridge: pong received");
                    }
                    Ok(Message::Close(_)) => {
                        info!("ecs_bridge: connection closed by server");
                        break;
                    }
                    Err(e) => {
                        error!("ecs_bridge: websocket error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
            info!("ecs_bridge: reader task ended");
        });

        // Writer task
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let result = match cmd {
                    WsCommand::Send(json) => {
                        let text = serde_json::to_string(&json).unwrap_or_default();
                        write.send(Message::Text(text.into())).await
                    }
                    WsCommand::Ping => write.send(Message::Ping(vec![])).await,
                    WsCommand::Close => write.send(Message::Close(None)).await,
                };
                if let Err(e) = result {
                    error!("ecs_bridge: write error: {e}");
                    break;
                }
            }
            info!("ecs_bridge: writer task ended");
        });

        Ok((Self { cmd_tx }, msg_rx))
    }

    pub fn send(&self, json: Value) -> Result<(), String> {
        self.cmd_tx
            .send(WsCommand::Send(json))
            .map_err(|e| format!("send failed: {e}"))
    }

    pub fn ping(&self) -> Result<(), String> {
        self.cmd_tx
            .send(WsCommand::Ping)
            .map_err(|e| format!("ping failed: {e}"))
    }

    pub fn close(&self) -> Result<(), String> {
        self.cmd_tx
            .send(WsCommand::Close)
            .map_err(|e| format!("close failed: {e}"))
    }
}
