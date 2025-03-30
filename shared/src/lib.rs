use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod channel;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatCommand {
    SendMessage(Message),
    Join(String),
    Leave(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatResponse {
    MessageReceived(Message),
    Joined(String),
    Left(String),
    Error(String),
}

pub type ChatResult<T> = std::result::Result<T, ChatError>;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
