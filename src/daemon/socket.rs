use crate::state::AppState;
use anyhow::Result;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Holds connected client streams and broadcasts state to them
#[derive(Clone)]
pub struct Broadcaster {
    clients: Arc<Mutex<Vec<UnixStream>>>,
}

impl Broadcaster {
    fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_client(&self, stream: UnixStream) {
        if let Ok(mut clients) = self.clients.lock() {
            clients.push(stream);
        }
    }

    /// Broadcast state to all connected clients, removing disconnected ones.
    pub fn broadcast(&self, state: &AppState) -> Result<()> {
        let framed = state.to_framed_json()?;
        let bytes = framed.as_bytes();

        let mut clients = self.clients.lock().unwrap();
        clients.retain_mut(|stream| {
            stream.write_all(bytes).is_ok() && stream.flush().is_ok()
        });

        Ok(())
    }

    /// Number of connected clients
    pub fn client_count(&self) -> usize {
        self.clients.lock().unwrap().len()
    }
}

/// Unix socket server that accepts client connections
pub struct SocketServer {
    listener: UnixListener,
    broadcaster: Broadcaster,
}

impl SocketServer {
    /// Create a new socket server at the given path
    pub fn new(path: &Path) -> Result<Self> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(false)?;
        tracing::info!("Socket server listening on {}", path.display());

        Ok(Self {
            listener,
            broadcaster: Broadcaster::new(),
        })
    }

    /// Get a clone of the broadcaster for publishing state
    pub fn broadcaster(&self) -> Broadcaster {
        self.broadcaster.clone()
    }

    /// Accept one incoming connection (blocking). For testing.
    pub fn accept_loop_once(&self) -> Result<()> {
        let (stream, _) = self.listener.accept()?;
        tracing::info!("Client connected");
        self.broadcaster.add_client(stream);
        Ok(())
    }

    /// Run the accept loop in a blocking fashion (call from a dedicated thread).
    pub fn accept_loop(&self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    tracing::info!("Client connected to daemon socket");
                    self.broadcaster.add_client(stream);
                }
                Err(e) => {
                    tracing::error!("Socket accept error: {}", e);
                    break;
                }
            }
        }
    }
}
