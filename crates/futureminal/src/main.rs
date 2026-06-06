//! Futureminal — Main entry point.
//!
//! Usage:
//!   futureminal                    # Launch daemon + UI
//!   futureminal --daemon           # Run headless daemon only
//!   futureminal --dev-mode         # Enable development features

use clap::Parser;
use Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Parser)]
#[command(name = "futureminal")]
#[command(about = "The next-generation terminal: AI-native, Privacy-first, Blockchain-auditable")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Run in daemon mode (headless, no UI).
    #[arg(long)]
    daemon: bool,
    /// Enable development mode (extra logging, hot reload).
    #[arg(long)]
    dev_mode: bool,
    /// Enable blockchain features (requires `--features blockchain`).
    #[arg(long)]
    blockchain: bool,
    /// Path to config file.
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.dev_mode { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(true)
        .with_thread_ids(cli.dev_mode)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!(
        "Starting Futureminal v{} (daemon={}, dev_mode={})",
        env!("CARGO_PKG_VERSION"),
        cli.daemon,
        cli.dev_mode
    );

    if let Some(config_path) = &cli.config {
        std::env::set_var("FUTUREMINAL_CONFIG", config_path.as_os_str());
    }

    let config = futureminal_core::config::Config::load()
        .unwrap_or_else(|e| {
            warn!("Failed to load config: {}. Using defaults.", e);
            futureminal_core::config::Config::default()
        });

    futureminal_core::init();
    futureminal_renderer::init();
    futureminal_ai::init();
    futureminal_ipc::init();
    futureminal_plugin::init();

    #[cfg(feature = "blockchain")]
    {
        if cli.blockchain {
            futureminal_blockchain::init();
            info!("Blockchain features initialized");
        }
    }

    let core_state = futureminal_core::CoreState::new(config);

    if cli.daemon {
        run_daemon(core_state).await?;
    } else {
        run_interactive(core_state).await?;
    }

    Ok(())
}

async fn run_daemon(state: futureminal_core::CoreState) -> anyhow::Result<()> {
    info!("Daemon mode: starting IPC server and PTY manager");

    let pty_manager = futureminal_core::pty::PtyManager::new();
    let (ipc_server, mut ipc_rx) = futureminal_ipc::IpcServer::new();
    let socket_path = futureminal_ipc::default_socket_path();

    // Spawn IPC server
    let ipc_handle = tokio::spawn({
        let server = ipc_server;
        let path = socket_path.clone();
        async move {
            if let Err(e) = server.run(path.to_str().unwrap_or("/tmp/futureminal.sock")).await {
                warn!("IPC server error: {}", e);
            }
        }
    });

    // Spawn config file watcher
    let config_handle = tokio::spawn({
        let state = state.clone();
        async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = state.reload_config().await {
                    warn!("Config reload failed: {}", e);
                }
            }
        }
    });

    // Spawn signal handler
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(());
    });

    // Main daemon loop
    let mut shutdown = false;
    loop {
        tokio::select! {
            Some((request, responder)) = ipc_rx.recv() => {
                if let Err(e) = handle_ipc_request(request, responder, &pty_manager, &state).await {
                    warn!("IPC request error: {}", e);
                }
            }
            _ = &mut shutdown_rx => {
                info!("Received shutdown signal, exiting daemon gracefully");
                shutdown = true;
                break;
            }
            else => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    info!("Terminating all PTY sessions...");
    pty_manager.terminate_all().await;

    // Cleanup socket file
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    ipc_handle.abort();
    config_handle.abort();

    info!("Daemon shutdown complete");
    Ok(())
}

async fn handle_ipc_request(
    request: futureminal_ipc::DaemonRequest,
    responder: tokio::sync::oneshot::Sender<Result<Vec<u8>, futureminal_ipc::IpcError>>,
    pty_manager: &futureminal_core::pty::PtyManager,
    state: &futureminal_core::CoreState,
) -> anyhow::Result<()> {
    use futureminal_ipc::{DaemonRequest, IpcError};
    let result: Result<Vec<u8>, IpcError> = match request {
        DaemonRequest::SpawnSession { shell, cols, rows } => {
            let shell_path = std::path::Path::new(&shell);
            let dims = futureminal_core::pty::PtyDimensions { cols, rows };
            match pty_manager.spawn_session(shell_path, dims).await {
                Ok((id, _)) => Ok(id.into_bytes()),
                Err(e) => Err(IpcError::Transport(e.to_string())),
            }
        }
        DaemonRequest::WriteToSession { session_id, data } => {
            match pty_manager.write_to(&session_id, &data).await {
                Ok(()) => Ok(vec![]),
                Err(e) => Err(IpcError::Transport(e.to_string())),
            }
        }
        DaemonRequest::ResizeSession { session_id, cols, rows } => {
            let dims = futureminal_core::pty::PtyDimensions { cols, rows };
            match pty_manager.resize(&session_id, dims).await {
                Ok(()) => Ok(vec![]),
                Err(e) => Err(IpcError::Transport(e.to_string())),
            }
        }
        DaemonRequest::KillSession { session_id } => {
            match pty_manager.terminate(&session_id).await {
                Ok(()) => Ok(vec![]),
                Err(e) => Err(IpcError::Transport(e.to_string())),
            }
        }
        DaemonRequest::ReloadConfig => {
            match state.reload_config().await {
                Ok(()) => Ok(vec![]),
                Err(e) => Err(IpcError::Transport(e.to_string())),
            }
        }
        DaemonRequest::Shutdown => {
            info!("Shutdown requested via IPC");
            Ok(vec![])
        }
        _ => Ok(vec![]),
    };
    let _ = responder.send(result);
    Ok(())
}

async fn run_interactive(_state: futureminal_core::CoreState) -> anyhow::Result<()> {
    info!("Interactive mode: launching terminal");

    let pty_manager = futureminal_core::pty::PtyManager::new();
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let (session_id, mut events) = pty_manager
        .spawn_session(
            std::path::Path::new(&shell),
            futureminal_core::pty::PtyDimensions::default(),
        )
        .await?;

    info!("Spawned interactive session: {}", session_id);

    // Bridge PTY events to stdout and stdin to PTY
    let pty_manager = std::sync::Arc::new(pty_manager);
    let pty_write = pty_manager.clone();
    let session_id_clone = session_id.clone();

    let stdin_handle = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match stdin.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(_) = pty_write.write_to(&session_id_clone, &buf[..n]).await {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    while let Some(event) = events.recv().await {
        match event {
            futureminal_core::pty::PtyEvent::Output(data) => {
                let _ = tokio::io::stdout().write(&data).await;
                let _ = tokio::io::stdout().flush().await;
            }
            futureminal_core::pty::PtyEvent::Exit(code) => {
                info!("Session {} exited with code {:?}", session_id, code);
                break;
            }
            futureminal_core::pty::PtyEvent::Error(e) => {
                tracing::error!("PTY error: {}", e);
                break;
            }
        }
    }

    stdin_handle.abort();
    pty_manager.terminate(&session_id).await.ok();
    Ok(())
}
