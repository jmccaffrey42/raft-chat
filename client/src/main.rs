mod ui;

use shared::{ChatResponse, ChatCommand};
use shared::channel::ChatClientChannel;
use tracing::{info, error};
use eyre::Result;
use tracing_subscriber::layer::SubscriberExt;
use ui::{ChatUI, UIMessage, UIController};
use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, util::SubscriberInitExt};
use std::fs;
use chrono::Utc;

// Struct to hold the client state
struct ChatClientState {
    client: ChatClientChannel,
    ui_controller: UIController,
}

// Function to set up logging
fn setup_logging() -> Result<()> {
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

    info!("Logging initialized.");
    Ok(())
}

// Function to handle server events
async fn handle_server_event(state: &mut ChatClientState, event: ChatResponse) -> Result<bool> {
    match event {
        ChatResponse::MessageReceived(msg) => {
            let _ = state.ui_controller.send_message(UIMessage {
                content: msg.content,
                timestamp: Utc::now(),
            }).await;
        }
        ChatResponse::Joined(user) => {
            let _ = state.ui_controller.send_message(UIMessage {
                content: format!("User {} joined the chat", user),
                timestamp: Utc::now(),
            }).await;
        }
        ChatResponse::Left(user) => {
            let _ = state.ui_controller.send_message(UIMessage {
                content: format!("User {} left the chat", user),
                timestamp: Utc::now(),
            }).await;
        }
        ChatResponse::Error(e) => {
            error!("Server error: {}", e);
            return Ok(false); // Stop the loop on server error
        }
    }
    Ok(true) // Continue the loop
}

// Function to handle user messages/commands
async fn handle_user_message(state: &mut ChatClientState, message: String) -> Result<bool> {
    if message.starts_with('/') {
        let parts: Vec<&str> = message[1..].splitn(2, ' ').collect();
        let command = parts[0];
        let args = parts.get(1).unwrap_or(&"");

        match command {
            "join" => {
                if let Err(e) = state.client.send_command(ChatCommand::Join(args.to_string())).await {
                    error!("Failed to send join command: {}", e);
                    // Optionally notify the UI about the failure
                    let _ = state.ui_controller.send_message(UIMessage {
                        content: format!("Error joining: {}", e),
                        timestamp: Utc::now(),
                    }).await;
                }
            }
            "leave" => {
                if let Err(e) = state.client.send_command(ChatCommand::Leave(args.to_string())).await {
                    error!("Failed to send leave command: {}", e);
                     // Optionally notify the UI about the failure
                    let _ = state.ui_controller.send_message(UIMessage {
                        content: format!("Error leaving: {}", e),
                        timestamp: Utc::now(),
                    }).await;
                }
            }
            "quit" => {
                info!("Quitting chat client via /quit command...");
                let _ = state.ui_controller.send_message(UIMessage {
                    content: "Shutting down...".to_string(),
                    timestamp: Utc::now(),
                }).await;
                return Ok(false); // Signal to stop the loop
            }
            _ => {
                let _ = state.ui_controller.send_message(UIMessage {
                    content: format!("Unknown command: /{}", command),
                    timestamp: Utc::now(),
                }).await;
            }
        }
    } else {
        // Send regular message
        if let Err(e) = state.client.send_message(&message).await {
            error!("Failed to send message: {}", e);
             // Optionally notify the UI about the failure
             let _ = state.ui_controller.send_message(UIMessage {
                content: format!("Error sending message: {}", e),
                timestamp: Utc::now(),
            }).await;
        }
    }
    Ok(true) // Continue the loop
}

// Main event loop logic
async fn run_event_loop(mut client_state: ChatClientState) {
    loop {
        tokio::select! {
            // Handle server messages
            result = client_state.client.receive_event() => {
                match result {
                    Ok(event) => {
                        // Use unwrap_or(false) to default to stopping if handle_server_event fails
                        if !handle_server_event(&mut client_state, event).await.unwrap_or(false) {
                            error!("Server event handler indicated stop or failed.");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error receiving event from server channel: {}", e);
                        // Attempt to inform the UI before breaking
                         let _ = client_state.ui_controller.send_message(UIMessage {
                            content: format!("Connection error: {}", e),
                            timestamp: Utc::now(),
                        }).await;
                        break; // Exit loop on channel receive error
                    }
                }
            }

            // Handle user input from UI
            result = client_state.ui_controller.recv_user_message() => {
                match result {
                     Ok(message) => {
                        // Use unwrap_or(false) to default to stopping if handle_user_message fails
                        if !handle_user_message(&mut client_state, message).await.unwrap_or(false) {
                            info!("User message handler indicated stop (likely /quit) or failed.");
                            break;
                        }
                     }
                     Err(e) => {
                          error!("Error receiving message from UI channel: {}", e);
                          // This might happen if the UI task panics or closes the channel.
                          break; 
                     }
                }
            }
        }
    }
    info!("Event loop task finished.");
    // The loop finishes, the task completes.
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging first
    setup_logging()?;

    info!("Starting chat client...");

    // Install custom panic and error hooks
    color_eyre::install()?;
    
    let client_channel = ChatClientChannel::connect("127.0.0.1:8080")
        .await
        .map_err(|e| eyre::eyre!("failed to connect to chat server: {}", e))?;

    // Create and initialize the UI
    let (mut ui, ui_controller) = ChatUI::new()?;

    // Create the client state
    let client_state = ChatClientState { // No longer mutable here
        client: client_channel,
        ui_controller: ui_controller.clone(), // Clone for the event loop task
    };

    // Spawn the main event loop task using the new function
    let event_loop = tokio::spawn(run_event_loop(client_state)); // Pass ownership

    // Spawn the UI task
    let ui_handle = tokio::task::spawn_blocking(move || {
        let result = ui.run();
        info!("UI task finished.");
        result // Return the result from the UI run
    });

    // Wait for either the event loop or the UI to finish
    tokio::select! {
        _ = event_loop => {
            info!("event_loop ended, sending shutdown signal");
            ui_controller.shutdown().await?;
        }
        _ = ui_handle => {
            info!("UI exited");
        }
    }

    info!("Chat client shutting down.");
    Ok(())
}
