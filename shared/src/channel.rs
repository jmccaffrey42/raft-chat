use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json;

use crate::{ChatCommand, ChatResponse, Message, ChatError, ChatResult};

#[derive(Debug)]
pub struct ChatClientChannel {
    writer: OwnedWriteHalf,
    reader: BufReader<OwnedReadHalf>,
}

impl ChatClientChannel {
    pub async fn connect(addr: &str) -> ChatResult<Self> {
        let connection = TcpStream::connect(addr)
            .await
            .map_err(|e| ChatError::Network(format!("failed to connect to {}: {}", addr, e)))?;

        Self::from_stream(connection)
    }
    
    pub fn from_stream(socket: TcpStream) -> ChatResult<Self> {
        let (reader, writer) = socket.into_split();
        Ok(Self { writer, reader: BufReader::new(reader) })
    }

    pub async fn send_bytes(&mut self, data: &mut Vec<u8>) -> ChatResult<()> {
        if !data.ends_with(&[b'\n']) {  
            data.push(b'\n');
        }

        self.writer
            .write_all(data)
            .await
            .map_err(|e| ChatError::Network(format!("failed to send bytes: {}", e)))?;

        Ok(())
    }

    pub async fn send_message(&mut self, msg_body: &str) -> ChatResult<()> {
        let msg = ChatCommand::SendMessage(Message {
            sender: "client".to_string(),
            content: msg_body.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        });

        let mut msg_bytes = serde_json::to_vec(&msg)
            .map_err(|e| ChatError::Protocol(format!("failed to serialize message: {}", e)))?;

        self.send_bytes(&mut msg_bytes).await
    }

    pub async fn receive_event(&mut self) -> ChatResult<ChatResponse> {
        let mut buffer = Vec::new();

        match self.reader.read_until(b'\n', &mut buffer).await {
            Ok(n) if n > 0 => {
                serde_json::from_slice(&buffer)
                    .map_err(|e| ChatError::Protocol(format!("failed to parse event: {}", e)))
            }
            Ok(_) => Err(ChatError::Network("connection closed".to_string())),
            Err(e) => Err(ChatError::Network(format!("failed to read from connection: {}", e))),
        }
    }

    pub async fn receive_command(&mut self) -> ChatResult<ChatCommand> {
        let mut buffer = Vec::new();

        match self.reader.read_until(b'\n', &mut buffer).await {
            Ok(n) if n > 0 => {
                serde_json::from_slice(&buffer)
                    .map_err(|e| ChatError::Protocol(format!("failed to parse command: {}", e)))
            }
            Ok(_) => Err(ChatError::Network("connection closed".to_string())),
            Err(e) => Err(ChatError::Network(format!("failed to read from connection: {}", e))),
        }
    }
} 