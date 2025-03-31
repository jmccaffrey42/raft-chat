mod ui;

use shared::{ChatResponse, ChatCommand};
use shared::channel::ChatClientChannel;
use tokio::pin;
use tokio::sync::oneshot;
use tracing::{info, error};
use eyre::Result;
use tracing_subscriber::layer::SubscriberExt;
use ui::{ChatUI, UIMessage};
use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, Layer, util::SubscriberInitExt};
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    // Create logs directory if it doesn't exist
    let log_dir = PathBuf::from("logs");
    fs::create_dir_all(&log_dir)?;

    // Configure file appender
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir,
        "chat-client.log",
    );

    // Create a subscriber that writes to both file and stdout
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::Layer::new()
        .with_writer(non_blocking)
        .with_ansi(false);
    
    tracing_subscriber::registry()
        .with(file_layer)
        .init();

    info!("Starting chat client...");

    // Install custom panic and error hooks
    color_eyre::install()?;
    
    let mut client = ChatClientChannel::connect("127.0.0.1:8080")
        .await
        .map_err(|e| eyre::eyre!("failed to create chat client: {}", e))?;

    // Create and initialize the UI
    let (mut ui, ui_controller) = ChatUI::new()?;

    let ui_controller_clone = ui_controller.clone();
    // Spawn the main event loop task
    let mut event_loop = tokio::spawn(async move {
        let mut ui_controller = ui_controller_clone;
        loop {
            tokio::select! {
                // Handle server messages
                result = client.receive_event() => {
                    match result {
                        Ok(ChatResponse::MessageReceived(msg)) => {
                            let _ = ui_controller.send_message(UIMessage {
                                content: msg.content,
                                timestamp: chrono::Utc::now(),
                            }).await;
                        }
                        Ok(ChatResponse::Joined(user)) => {
                            let _ = ui_controller.send_message(UIMessage {
                                content: format!("User {} joined the chat", user),
                                timestamp: chrono::Utc::now(),
                            }).await;
                        }
                        Ok(ChatResponse::Left(user)) => {
                            let _ = ui_controller.send_message(UIMessage {
                                content: format!("User {} left the chat", user),
                                timestamp: chrono::Utc::now(),
                            }).await;
                        }
                        Ok(ChatResponse::Error(e)) => {
                            error!("Server error: {}", e);
                            break;
                        }
                        Err(e) => {
                            error!("Error receiving event: {}", e);
                            break;
                        }
                    }
                }

                Ok(message) = ui_controller.recv_user_message() => {
                    // Handle slash commands
                    if message.starts_with('/') {
                        let parts: Vec<&str> = message[1..].splitn(2, ' ').collect();
                        let command = parts[0];
                        let args = parts.get(1).unwrap_or(&"");

                        match command {
                            "join" => {
                                if let Err(e) = client.send_command(ChatCommand::Join(args.to_string())).await {
                                    error!("Failed to send join command: {}", e);
                                }
                            }
                            "leave" => {
                                if let Err(e) = client.send_command(ChatCommand::Leave(args.to_string())).await {
                                    error!("Failed to send leave command: {}", e);
                                }
                            }
                            "quit" => {
                                info!("Quitting chat client...");
                                let _ = ui_controller.send_message(UIMessage {
                                    content: "Shutting down...".to_string(),
                                    timestamp: chrono::Utc::now(),
                                }).await;
                                break;
                            }
                            _ => {
                                let _ = ui_controller.send_message(UIMessage {
                                    content: format!("Unknown command: {}", command),
                                    timestamp: chrono::Utc::now(),
                                }).await;
                            }
                        }
                    } else {
                        if let Err(e) = client.send_message(&message).await {
                            error!("Failed to queue message: {}", e);
                        }
                    }
                }
            }
        }
        info!("Event loop completed!!!");
    });

    let ui_handle = tokio::task::spawn_blocking(move || {
        let result = ui.run();
        info!("UI thread completed");
        result
    });

    tokio::select! {
        _ = &mut event_loop => {
            info!("event_loop ended, sending shutdown signal");
            ui_controller.shutdown().await?;
        }
        _ = ui_handle => {
            info!("UI exited");
        }
    }

    Ok(())
}
