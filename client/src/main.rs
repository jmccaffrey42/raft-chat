use std::time::{SystemTime, UNIX_EPOCH};

use shared::{ChatCommand, ChatResponse, Message};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, error};
use eyre::{Result, WrapErr, eyre};

#[derive(Debug)]
struct ChatClient {
    writer: OwnedWriteHalf,
    reader: BufReader<OwnedReadHalf>,
}

impl ChatClient {
    async fn connect(addr: &str) -> Result<Self> {
        let connection = TcpStream::connect(addr)
            .await
            .wrap_err_with(|| format!("failed to connect to {}", addr))?;

        let (reader, writer) = connection.into_split();

        Ok(Self { writer, reader: BufReader::new(reader) })
    }
    
    async fn send_message(&mut self, msg_body: &str) -> Result<()> {
        let msg = ChatCommand::SendMessage(Message {
            sender: "client".to_string(),
            content: msg_body.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        });

        let mut msg_bytes = serde_json::to_vec(&msg).unwrap();
        msg_bytes.push(b'\n');

        info!("Sending message: {}", msg_body);

        self.writer
            .write_all(&msg_bytes)
            .await
            .wrap_err("failed to send message")?;
            
        Ok(())
    }

    async fn receive_event(&mut self) -> Result<ChatResponse> {
        let mut buffer = Vec::new();

        match self.reader.read_until(b'\n', &mut buffer).await {
            Ok(n) if n > 0 => {
                if let Ok(event) = serde_json::from_slice(&buffer) {
                    Ok(event)
                } else {
                    Err(eyre!("failed to parse event"))
                }
            }
            Ok(_) => return Err(eyre!("connection closed")),
            Err(e) => return Err(e.into()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    info!("Starting chat client...");

    // Install custom panic and error hooks
    color_eyre::install()?;
    
    let mut client = ChatClient::connect("127.0.0.1:8080")
        .await
        .wrap_err("failed to create chat client")?;

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
                    Err(e) => {
                        error!("Error receiving event: {}", e);
                        break;
                    }
                    _ => {
                        error!("Unknown event");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
