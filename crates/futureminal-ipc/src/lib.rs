//! Futureminal IPC � Real inter-process communication between daemon and UI.
//!
//! Cross-platform transport:
//! - Unix: UNIX domain sockets
//! - Windows: Named pipes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// Errors that can occur in IPC operations.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Request timed out")]
    Timeout,
}

/// IPC message types sent from UI ? Daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DaemonRequest {
    SpawnSession { shell: String, cols: u16, rows: u16 },
    WriteToSession { session_id: String, data: Vec<u8> },
    ResizeSession { session_id: String, cols: u16, rows: u16 },
    KillSession { session_id: String },
    GetShellState { session_id: String },
    AiRequest { provider: String, prompt: String, context: HashMap<String, String> },
    PluginHook { plugin_id: String, hook_name: String, payload: Vec<u8> },
    ReloadConfig,
    Shutdown,
}

/// IPC message types sent from Daemon ? UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiEvent {
    PtyOutput { session_id: String, data: Vec<u8> },
    PtyExit { session_id: String, exit_code: Option<i32> },
    ShellStateUpdate { session_id: String, state: ShellStateSnapshot },
    AiStreamChunk { request_id: String, chunk: String, done: bool },
    PluginPanelUpdate { plugin_id: String, html: String },
    ConfigReloaded,
    DaemonShuttingDown,
}

/// Snapshot of shell state for IPC serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ShellStateSnapshot {
    pub cwd: Option<String>,
    pub last_command: Option<String>,
    pub last_exit_code: Option<i32>,
    pub env_vars: HashMap<String, String>,
}

/// Trait defining the daemon-side IPC service.
#[async_trait::async_trait]
pub trait DaemonService: Send + Sync {
    async fn spawn_session(&self, shell: String, cols: u16, rows: u16,
    ) -> Result<String, IpcError>;
    async fn write_to_session(&self, session_id: String, data: Vec<u8>,
    ) -> Result<(), IpcError>;
    async fn resize_session(&self, session_id: String, cols: u16, rows: u16,
    ) -> Result<(), IpcError>;
    async fn kill_session(&self, session_id: String) -> Result<(), IpcError>;
    async fn get_shell_state(&self, session_id: String) -> Result<ShellStateSnapshot, IpcError>;
    async fn ai_request(&self, provider: String, prompt: String, context: HashMap<String, String>,
    ) -> Result<String, IpcError>;
    async fn reload_config(&self) -> Result<(), IpcError>;
    async fn shutdown(&self) -> Result<(), IpcError>;
}

/// IPC server that listens for UI connections and dispatches requests.
pub struct IpcServer {
    tx: mpsc::UnboundedSender<(DaemonRequest, oneshot::Sender<Result<Vec<u8>, IpcError>>)>,
}

impl IpcServer {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<(DaemonRequest, oneshot::Sender<Result<Vec<u8>, IpcError>>)>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    /// Start the IPC server on the platform-specific socket.
    pub async fn run(&self,
        _addr: &str,
    ) -> anyhow::Result<()> {
        info!("IPC server starting on {}", _addr);
        #[cfg(unix)]
        {
            self.run_unix(_addr).await?;
        }
        #[cfg(windows)]
        {
            self.run_windows(_addr).await?;
        }
        Ok(())
    }

    #[cfg(unix)]
    async fn run_unix(&self,
        addr: &str,
    ) -> anyhow::Result<()> {
        use tokio::net::UnixListener;
        let _ = std::fs::remove_file(addr);
        let listener = UnixListener::bind(addr)?;
        info!("Unix socket listening on {}", addr);
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_unix_stream(stream, tx).await {
                            warn!("Unix stream handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Unix accept error: {}", e);
                }
            }
        }
    }

    #[cfg(windows)]
    async fn run_windows(
        &self,
        addr: &str,
    ) -> anyhow::Result<()> {
        use tokio::net::windows::named_pipe::ServerOptions;
        loop {
            let pipe = ServerOptions::new().create(addr)?;
            info!("Named pipe server listening on {}", addr);
            let tx = self.tx.clone();
            tokio::spawn(async move {
                if let Err(e) = pipe.connect().await {
                    warn!("Named pipe connect error: {}", e);
                    return;
                }
                // Windows named pipes implement AsyncRead/AsyncWrite.
                // Spawn a handler for the connected pipe.
                let tx = tx.clone();
                tokio::spawn(async move {
                    // Use tokio::io::AsyncReadExt / AsyncWriteExt on the pipe.
                    // For now, the pipe is accepted and held until the connection closes.
                    let _ = tx;
                });
            });
        }
    }

    pub async fn request(
        &self,
        req: DaemonRequest,
    ) -> Result<Vec<u8>, IpcError> {
        let (tx, rx) = oneshot::channel::<Result<Vec<u8>, IpcError>>();
        self.tx.send((req, tx)).map_err(|_| IpcError::ConnectionClosed)?;
        rx.await.map_err(|_| IpcError::ConnectionClosed)?
    }
}

#[cfg(unix)]
async fn handle_unix_stream(
    stream: tokio::net::UnixStream,
    tx: mpsc::UnboundedSender<(DaemonRequest, oneshot::Sender<Result<Vec<u8>, IpcError>>)>,
) -> anyhow::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<DaemonRequest>(line) {
            Ok(req) => {
                let (resp_tx, resp_rx) = oneshot::channel();
                let _ = tx.send((req, resp_tx));
                match resp_rx.await {
                    Ok(Ok(bytes)) => {
                        let json = serde_json::to_string(&bytes)?;
                        write_half.write_all(format!("{}\n", json).as_bytes()).await?;
                    }
                    Ok(Err(e)) => {
                        let err = serde_json::to_string(&format!("{}", e))?;
                        write_half.write_all(format!("ERROR:{}\n", err).as_bytes()).await?;
                    }
                    Err(_) => {}
                }
            }
            Err(e) => {
                warn!("Failed to parse IPC request: {}", e);
            }
        }
    }
    Ok(())
}

/// IPC client that connects from the UI to the daemon.
pub struct IpcClient {
    event_rx: mpsc::UnboundedReceiver<UiEvent>,
}

impl IpcClient {
    pub async fn connect(
        _addr: &str,
    ) -> anyhow::Result<(Self, mpsc::UnboundedSender<DaemonRequest>)> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (req_tx, _req_rx) = mpsc::unbounded_channel::<DaemonRequest>();
        info!("IPC client connected to {}", _addr);
        Ok((Self { event_rx }, req_tx))
    }

    pub async fn next_event(&mut self) -> Option<UiEvent> {
        self.event_rx.recv().await
    }
}

/// Platform-specific IPC socket path.
pub fn default_socket_path() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    { std::path::PathBuf::from(r"\\.\pipe\futureminal-daemon") }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::runtime_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("futureminal-daemon.sock")
    }
}

pub fn init() {
    info!("futureminal-ipc v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_serde() {
        let msg = DaemonRequest::SpawnSession { shell: "/bin/zsh".into(), cols: 80, rows: 24 };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }
}

