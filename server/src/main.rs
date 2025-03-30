use shared::{channel::ChatClientChannel, ChatCommand, ChatResponse};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, error};
use eyre::{Result, WrapErr};

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
    let mut client = ChatClientChannel::from_stream(socket).unwrap();

    loop {

        match client.receive_command().await {
            Ok(cmd) => {
                match cmd {
                    ChatCommand::SendMessage(message) => {
                        let response = ChatResponse::MessageReceived(message);
                        let mut response_bytes = serde_json::to_vec(&response).unwrap();
                        client.send_bytes(&mut response_bytes).await.unwrap();
                    }

                    ChatCommand::Join(_) => todo!(),

                    ChatCommand::Leave(_) => todo!(),
                }
            }

            Err(e) => {
                error!("Error reading from socket: {}", e);
                break;
            }
        }
    }
}
