use shared::{ChatCommand, ChatResponse, Message};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, net::{TcpListener, TcpStream}};
use tracing::{info, error};
use eyre::{eyre, Result, WrapErr};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Install custom panic and error hooks
    color_eyre::install()?;
    
    info!("Starting chat server...");

    // Listen for incoming connections
    let listener = TcpListener::bind("0.0.0.0:8080")
        .await
        .wrap_err("Failed to bind to address")?;
        
    info!("Server listening on 0.0.0.0:8080");

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("New connection from {}", addr);
                tokio::spawn(handle_connection(socket));
            }
            Err(e) => {
                error!("Error accepting connection: {}", e);
            }
        }
    }
}

async fn handle_connection(socket: TcpStream) {
    let mut reader = BufReader::new(socket);
    let mut buffer = Vec::new();

    loop {
        // Clear the buffer for the next message
        buffer.clear();

        // Read until a newline character
        match reader.read_until(b'\n', &mut buffer).await {
            Ok(n) if n > 0 => {
                if let Ok(cmd) = serde_json::from_slice::<ChatCommand>(&buffer) {
                    info!("Received cmd: {:?}", cmd);

                    match cmd {
                        ChatCommand::SendMessage(message) => {
                            let response = ChatResponse::MessageReceived(message);
                            let mut response_bytes = serde_json::to_vec(&response).unwrap();
                            response_bytes.push(b'\n');
                            reader.get_mut().write_all(&response_bytes).await.unwrap();
                        }

                        _ => {
                            error!("Unknown command: {:?}", cmd);
                        }
                    }
                }
            }

            Ok(_) => break, // Connection closed

            Err(e) => {
                error!("Error reading from socket: {}", e);
                break;
            }
        }
    }
}
