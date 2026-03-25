use crate::state::AppState;
use anyhow::Result;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
type IpcStream = UnixStream;
#[cfg(windows)]
type IpcStream = std::net::TcpStream;

#[cfg(unix)]
type IpcListener = UnixListener;
#[cfg(windows)]
type IpcListener = std::net::TcpListener;

/// Holds connected client streams and broadcasts state to them
#[derive(Clone)]
pub struct Broadcaster {
    clients: Arc<Mutex<Vec<IpcStream>>>,
    last_state: Arc<Mutex<Option<String>>>,
}

impl Broadcaster {
    fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
            last_state: Arc::new(Mutex::new(None)),
        }
    }

    fn add_client(&self, mut stream: IpcStream) {
        // Send last known state to the new client immediately
        let last = self.last_state.lock().unwrap();
        if let Some(ref state_json) = *last {
            let _ = stream.write_all(state_json.as_bytes());
            let _ = stream.flush();
        }
        drop(last);
        let mut clients = self.clients.lock().unwrap();
        clients.push(stream);
    }

    /// Broadcast state to all connected clients, removing disconnected ones.
    pub fn broadcast(&self, state: &AppState) -> Result<()> {
        let framed = state.to_framed_json()?;

        // Store for new clients
        {
            let mut last = self.last_state.lock().unwrap();
            *last = Some(framed.clone());
        }

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

/// IPC server that accepts client connections
pub struct SocketServer {
    listener: IpcListener,
    broadcaster: Broadcaster,
}

impl SocketServer {
    /// Create a new socket server bound to a Unix socket path
    #[cfg(unix)]
    pub fn new(path: &std::path::Path) -> Result<Self> {
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

    /// Create a new socket server bound to a TCP port on localhost
    #[cfg(windows)]
    pub fn new(port: u16) -> Result<Self> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
        let local_port = listener.local_addr()?.port();
        tracing::info!("Socket server listening on 127.0.0.1:{}", local_port);

        Ok(Self {
            listener,
            broadcaster: Broadcaster::new(),
        })
    }

    /// Get the actual bound port (useful when binding to port 0)
    #[cfg(windows)]
    pub fn local_port(&self) -> u16 {
        self.listener.local_addr().unwrap().port()
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
