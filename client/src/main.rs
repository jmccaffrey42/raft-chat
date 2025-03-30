use shared::ChatResponse;
use shared::channel::ChatClientChannel;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, error};
use eyre::Result;

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

    // Create a buffer reader for stdin
    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut input = String::new();

    loop {
        tokio::select! {
            // Handle user input
            result = stdin.read_line(&mut input) => {
                match result {
                    Ok(_) => {
                        let message = input.trim();
                        if !message.is_empty() {
                            if let Err(e) = client.send_message(message).await {
                                error!("Failed to send message: {}", e);
                                break;
                            }
                        }
                        input.clear();
                    }
                    Err(e) => {
                        error!("Error reading from stdin: {}", e);
                        break;
                    }
                }
            }

            // Handle server messages
            result = client.receive_event() => {
                match result {
                    Ok(ChatResponse::MessageReceived(msg)) => {
                        info!("Received message: {}", msg.content);
                    }
                    Ok(ChatResponse::Joined(user)) => {
                        info!("Joined chat as {}", user);
                    }
                    Ok(ChatResponse::Left(user)) => {
                        info!("Left chat as {}", user);
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
        }
    }

    Ok(())
}
