use crossterm::{
    cursor::{Hide, Show, MoveTo},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType, size},
    tty::IsTty,
};
use eyre::Result;
use std::{
    io::{self, Stdout, Write}, mem, sync::Arc, time::Duration, sync::Mutex
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::error;

const MAX_MESSAGES: usize = 1000;

#[derive(Debug)]
pub struct UIMessage {
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct UIController {
    message_tx: mpsc::Sender<UIMessage>,
    user_message_tx: broadcast::Sender<String>,
    user_message_rx: broadcast::Receiver<String>,
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl Clone for UIController {
    fn clone(&self) -> Self {
        Self {
            message_tx: self.message_tx.clone(),
            user_message_tx: self.user_message_tx.clone(),
            user_message_rx: self.user_message_tx.subscribe(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }
}

impl UIController {
    pub async fn send_message(&self, message: UIMessage) -> Result<()> {
        self.message_tx.send(message).await
            .map_err(|e| eyre::eyre!("Failed to send message: {}", e))
    }

    pub async fn recv_user_message(&mut self) -> Result<String> {
        let message = self.user_message_rx.recv().await
            .map_err(|e| eyre::eyre!("Failed to receive user message: {}", e))?;

        Ok(message)
    }

    pub async fn shutdown(&self) -> Result<()> {
        let mut lock = self.shutdown_tx.lock()
            .map_err(|_| eyre::eyre!("Failed to acquire lock"))?;
        
        if let Some(tx) = lock.take() {
            tx.send(()).map_err(|_| eyre::eyre!("Failed to send shutdown signal"))
        } else {
            Err(eyre::eyre!("Shutdown signal already sent"))
        }
    }
}

pub struct ChatUI {
    messages: Vec<UIMessage>,
    input_buffer: String,
    stdout: Stdout,
    message_rx: mpsc::Receiver<UIMessage>,
    shutdown_rx: oneshot::Receiver<()>,
    user_message_tx: broadcast::Sender<String>,
}

impl ChatUI {
    pub fn new() -> Result<(Self, UIController)> {
        // Check if we're running in a terminal
        if !io::stdout().is_tty() {
            return Err(eyre::eyre!("Not running in a terminal"));
        }

        // Enable raw mode and alternate screen
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        // Create channels for message passing
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (message_tx, message_rx) = mpsc::channel(100);
        let (user_message_tx, _) = broadcast::channel(100);

        Ok((Self {
            messages: Vec::with_capacity(MAX_MESSAGES),
            input_buffer: String::new(),
            stdout,
            message_rx,
            shutdown_rx,
            user_message_tx: user_message_tx.clone(),
        },
        UIController {
            message_tx,
            user_message_tx: user_message_tx.clone(),
            user_message_rx: user_message_tx.subscribe(),
            shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
        }))
    }

    pub fn run(&mut self) -> Result<()> {
        // Hide cursor and clear screen
        execute!(self.stdout, Hide)?;
        self.clear_screen()?;

        loop {
            // Draw the UI
            self.draw()?;

            // Handle events
            if event::poll(Duration::from_millis(100))? {
            // if event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break;
                        }
                        KeyCode::Char(c) => {
                            self.input_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            self.input_buffer.pop();
                        }
                        KeyCode::Enter => {
                            if !self.input_buffer.is_empty() {
                                let message = mem::take(&mut self.input_buffer);
                                if let Err(e) = self.user_message_tx.send(message.clone()) {
                                    error!("Failed to send message: {}", e);
                                }
                            }
                        }
                        KeyCode::Esc => break,
                        _ => {}
                    }
                }
            } 

            // check if the shutdown signal has been sent
            if let Ok(_) = self.shutdown_rx.try_recv() {
                break;
            }

            // Check for new messages
            while let Ok(message) = self.message_rx.try_recv() {
                self.push_message(message);
            }
        }

        // Cleanup
        disable_raw_mode()?;
        execute!(
            self.stdout,
            LeaveAlternateScreen,
            DisableMouseCapture,
            Show
        )?;

        Ok(())
    }

    fn push_message(&mut self, message: UIMessage) {
        self.messages.push(message);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.remove(0);
        }
    }
    
    fn clear_screen(&mut self) -> Result<()> {
        execute!(self.stdout, Clear(ClearType::All))?;
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        self.clear_screen()?;

        // Get terminal size
        let (width, height) = size()?;
        let height = height as usize;
        let width = width as usize;

        // Draw messages from bottom up
        let mut y = height - 2; // Start one line above the input line
        for message in self.messages.iter().rev() {
            if y == 0 {
                break;
            }
            let timestamp = message.timestamp.format("%H:%M:%S").to_string();
            let line = format!("[{}] {}", timestamp, message.content);
            
            // Truncate line if it's too long
            let line = if line.len() > width as usize {
                &line[..width as usize]
            } else {
                &line
            };

            execute!(
                self.stdout,
                MoveTo(0, y as u16),
                Print(line)
            )?;
            y -= 1;
        }

        // Draw input bar
        let input_line = format!("> {}", self.input_buffer);
        let input_line = if input_line.len() > width as usize {
            &input_line[..width as usize]
        } else {
            &input_line
        };

        execute!(
            self.stdout,
            MoveTo(0, (height - 1) as u16),
            Print(input_line)
        )?;

        // Show cursor at the end of input (at the last character position)
        execute!(
            self.stdout,
            MoveTo(input_line.len() as u16, (height - 1) as u16),
            Show
        )?;

        self.stdout.flush()?;
        Ok(())
    }
} 