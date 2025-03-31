mod ui;

use shared::ChatResponse;
use shared::channel::ChatClientChannel;
use tracing::{info, error};
use eyre::Result;
use ui::{ChatUI, UIMessage};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    info!("Starting chat client...");

    // Install custom panic and error hooks
    color_eyre::install()?;
    
    let mut client = ChatClientChannel::connect("127.0.0.1:8080")
        .await
        .map_err(|e| eyre::eyre!("failed to create chat client: {}", e))?;

    // Create and initialize the UI
    let mut ui = ChatUI::new()?;
    let message_tx = ui.message_tx();
    let mut user_message_rx = ui.user_message_tx().subscribe();

    // Spawn a task to handle both server messages and client messages
    let message_tx_clone = message_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle server messages
                result = client.receive_event() => {
                    match result {
                        Ok(ChatResponse::MessageReceived(msg)) => {
                            let _ = message_tx_clone.send(UIMessage {
                                content: msg.content,
                                timestamp: chrono::Utc::now(),
                            }).await;
                        }
                        Ok(ChatResponse::Joined(user)) => {
                            let _ = message_tx_clone.send(UIMessage {
                                content: format!("User {} joined the chat", user),
                                timestamp: chrono::Utc::now(),
                            }).await;
                        }
                        Ok(ChatResponse::Left(user)) => {
                            let _ = message_tx_clone.send(UIMessage {
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

                Ok(message) = user_message_rx.recv() => {
                    if let Err(e) = client.send_message(&message).await {
                        error!("Failed to queue message: {}", e);
                    }
                }
            }
        }
    });

    // Run the UI
    ui.run().await?;

    Ok(())
}
