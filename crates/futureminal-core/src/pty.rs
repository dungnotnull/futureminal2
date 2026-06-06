//! PTY (Pseudo-Terminal) management for Futureminal.
//!
//! Spawns shells inside a PTY, handles I/O streaming, resizing, and process
//! lifecycle. Built on top of `portable-pty`.

use portable_pty::{native_pty_system, Child, CommandBuilder, PtyPair, PtySize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::Arc,
};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Size of a PTY in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyDimensions {
    pub cols: u16,
    pub rows: u16,
}

impl Default for PtyDimensions {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

/// Events emitted by a running PTY session.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    Output(Vec<u8>),
    Exit(i32),
    Error(String),
}

/// A single PTY session wrapping a spawned shell.
pub struct PtySession {
    id: String,
    dimensions: PtyDimensions,
    writer: Option<std::sync::Mutex<Box<dyn Write + Send>>>,
    child: Option<std::sync::Mutex<Box<dyn Child + Send>>>,
    /// The master handle is stored separately to support resize.
    master: Option<std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    events_tx: mpsc::UnboundedSender<PtyEvent>,
}

impl PtySession {
    /// Spawn a new PTY session with the given shell.
    pub fn spawn(
        id: String,
        shell: &Path,
        dimensions: PtyDimensions,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<PtyEvent>)> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows: dimensions.rows,
            cols: dimensions.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        cmd.cwd(std::env::current_dir().unwrap_or_else(|_| Path::new("/").into()));

        let child = pair.slave.spawn_command(cmd)?;
        let pid = child.process_id();
        info!("Spawned PTY session {} with PID {:?}", id, pid);

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;
        let master = std::sync::Mutex::new(pair.master);

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let tx = events_tx.clone();
        let session_id = id.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        debug!("PTY {} reader reached EOF", session_id);
                        let _ = tx.send(PtyEvent::Exit(0));
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if tx.send(PtyEvent::Output(data)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let msg = format!("PTY {} read error: {}", session_id, e);
                        error!("{}", msg);
                        let _ = tx.send(PtyEvent::Error(msg));
                        break;
                    }
                }
            }
        });

        Ok((
            Self {
                id,
                dimensions,
                writer: Some(std::sync::Mutex::new(writer)),
                child: Some(std::sync::Mutex::new(child)),
                master: Some(master),
                events_tx,
            },
            events_rx,
        ))
    }

    /// Resize the PTY to new dimensions.
    pub fn resize(&mut self, dimensions: PtyDimensions) -> anyhow::Result<()> {
        self.dimensions = dimensions;
        if let Some(ref master) = self.master {
            master.lock().unwrap().resize(PtySize {
                rows: dimensions.rows,
                cols: dimensions.cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            debug!("Resized PTY {} to {:?}", self.id, dimensions);
        }
        Ok(())
    }

    /// Write raw bytes into the PTY (e.g., user keystrokes).
    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.lock().unwrap().write_all(data)?;
            writer.lock().unwrap().flush()?;
        }
        Ok(())
    }

    /// Write a string into the PTY.
    pub fn write_str(&mut self, text: &str) -> anyhow::Result<()> {
        self.write(text.as_bytes())
    }

    /// Send a signal to the child process.
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(mut child) = self.child.take() {
            info!("Killing PTY session {}", self.id);
            child.lock().unwrap().kill()?;
            child.lock().unwrap().wait()?;
        }
        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn dimensions(&self) -> PtyDimensions {
        self.dimensions
    }
}

/// Manages multiple PTY sessions (one per tab/pane).
pub struct PtyManager {
    sessions: Arc<RwLock<std::collections::HashMap<String, PtySession>>>,
}

impl Default for PtyManager {
    fn default() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn spawn_session(
        &self,
        shell: &Path,
        dimensions: PtyDimensions,
    ) -> anyhow::Result<(String, mpsc::UnboundedReceiver<PtyEvent>)> {
        let id = uuid::Uuid::new_v4().to_string();
        let (session, rx) = PtySession::spawn(id.clone(), shell, dimensions)?;

        let mut sessions = self.sessions.write().await;
        sessions.insert(id.clone(), session);
        Ok((id, rx))
    }

    pub async fn write_to(
        &self,
        session_id: &str,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("PTY session {} not found", session_id))?;
        session.write(data)?;
        Ok(())
    }

    pub async fn resize(
        &self, session_id: &str, dimensions: PtyDimensions) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("PTY session {} not found", session_id))?;
        session.resize(dimensions)
    }

    pub async fn terminate(&self, session_id: &str) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(mut session) = sessions.remove(session_id) {
            session.kill().await?;
        }
        Ok(())
    }

    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    pub async fn terminate_all(&self) {
        let ids: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };
        for id in ids {
            if let Err(e) = self.terminate(&id).await {
                warn!("Failed to terminate session {} during shutdown: {}", id, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions_default() {
        let d = PtyDimensions::default();
        assert_eq!(d.cols, 80);
        assert_eq!(d.rows, 24);
    }

    #[test]
    fn test_pty_event_clone() {
        let e = PtyEvent::Output(vec![1, 2, 3]);
        let cloned = e.clone();
        match cloned {
            PtyEvent::Output(v) => assert_eq!(v, vec![1, 2, 3]),
            _ => panic!("unexpected variant"),
        }
    }
}
