// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unix-socket IPC server.
//!
//! Length-prefixed `postcard` framing on the wire: 4-byte
//! little-endian length followed by the payload. The server
//! accepts connections on the path returned by
//! [`default_socket_path`], spawns a task per connection, and
//! dispatches each request to the supplied async handler
//! closure. The handler is `Fn(IpcRequest) -> Future<Output =
//! IpcResponse>` so the dispatch can be async (DB lookups, IPC
//! forwarding) without holding a Mutex across awaits.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, warn};

use super::request::{IpcRequest, IpcResponse};

/// Errors produced by the IPC server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid IPC request: {0}")]
    InvalidRequest(String),

    #[error("server handler returned an error: {0}")]
    Handler(String),
}

/// Compute the default Unix socket path. Honours
/// `XDG_RUNTIME_DIR/muzon.sock`; falls back to
/// `/tmp/muzon-$UID.sock` if `XDG_RUNTIME_DIR` is unset.
pub fn default_socket_path() -> PathBuf {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(runtime).join("muzon.sock");
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return p;
    }
    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/tmp/muzon-{uid}.sock"))
}

/// Ensure the socket file is removed before binding (so a
/// stale socket from a previous crashed core does not block
/// the new bind). Best-effort: a `NotFound` error is ignored.
pub fn ensure_socket_absent(path: &Path) -> Result<(), ServerError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ServerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Set the socket file permissions to `0600` (owner read/write
/// only). This is the §3.9 confinement: only the same user can
/// talk to the core. Best-effort on platforms that do not
/// support Unix permissions (no-op on Windows).
pub fn set_socket_perms(path: &Path) -> Result<(), ServerError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|source| ServerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

/// Bind the Unix socket at `socket_path` and serve requests
/// forever. Each accepted connection is handled in a spawned
/// tokio task. The handler is wrapped in `Arc<H>` so each task
/// can hold a `'static` reference to it.
pub async fn serve<H, Fut>(socket_path: &Path, handler: H) -> Result<(), ServerError>
where
    H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = IpcResponse> + Send + 'static,
{
    ensure_socket_absent(socket_path)?;
    let listener = UnixListener::bind(socket_path).map_err(|source| ServerError::Io {
        path: socket_path.to_path_buf(),
        source,
    })?;
    set_socket_perms(socket_path)?;
    debug!("ipc-server: bound to {}", socket_path.display());

    let handler = Arc::new(handler);
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let handler = handler.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_connection(stream, handler).await {
                        warn!("ipc-server: connection ended with error: {err}");
                    }
                });
            }
            Err(source) => {
                warn!("ipc-server: accept failed: {source}");
            }
        }
    }
}

async fn handle_connection<H, Fut>(
    mut stream: UnixStream,
    handler: Arc<H>,
) -> Result<(), ServerError>
where
    H: Fn(IpcRequest) -> Fut,
    Fut: Future<Output = IpcResponse>,
{
    loop {
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_n) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(source) => {
                return Err(ServerError::Io {
                    path: PathBuf::from("<socket>"),
                    source,
                });
            }
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await.map_err(|source| ServerError::Io {
            path: PathBuf::from("<socket>"),
            source,
        })?;
        let req: IpcRequest = postcard::from_bytes(&payload)
            .map_err(|e| ServerError::InvalidRequest(e.to_string()))?;
        let resp = (handler)(req).await;
        let bytes = postcard::to_stdvec(&resp)
            .map_err(|e| ServerError::Handler(e.to_string()))?;
        let len_bytes = (bytes.len() as u32).to_le_bytes();
        stream.write_all(&len_bytes).await.map_err(|source| ServerError::Io {
            path: PathBuf::from("<socket>"),
            source,
        })?;
        stream.write_all(&bytes).await.map_err(|source| ServerError::Io {
            path: PathBuf::from("<socket>"),
            source,
        })?;
        stream.flush().await.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::IpcRequest;
    use crate::request::IpcResponse;
    use std::time::Duration;

    async fn echo_handler(req: IpcRequest) -> IpcResponse {
        if let IpcRequest::Ping { nonce } = req {
            IpcResponse::Pong { nonce }
        } else {
            panic!("unexpected request: {req:?}")
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn request_response_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let socket = tmp.path().join("muzon.sock");
        let path = socket.clone();
        let server = tokio::spawn(async move {
            serve(&path, echo_handler).await.expect("serve");
        });
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut stream = UnixStream::connect(&socket).await.expect("connect");
        let req = IpcRequest::Ping { nonce: 42 };
        let bytes = postcard::to_stdvec(&req).expect("serialise");
        let len = (bytes.len() as u32).to_le_bytes();
        stream.write_all(&len).await.expect("write len");
        stream.write_all(&bytes).await.expect("write body");
        stream.flush().await.ok();

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.expect("read len");
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; resp_len];
        stream.read_exact(&mut body).await.expect("read body");
        let resp: IpcResponse = postcard::from_bytes(&body).expect("deserialise");
        assert_eq!(resp, IpcResponse::Pong { nonce: 42 });

        drop(stream);
        server.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn length_prefix_framing_handles_many_requests() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let socket = tmp.path().join("muzon.sock");
        let path = socket.clone();
        let server = tokio::spawn(async move {
            serve(&path, echo_handler).await.expect("serve");
        });
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut stream = UnixStream::connect(&socket).await.expect("connect");
        for i in 0..1000u64 {
            let req = IpcRequest::Ping { nonce: i };
            let bytes = postcard::to_stdvec(&req).expect("serialise");
            let len = (bytes.len() as u32).to_le_bytes();
            stream.write_all(&len).await.expect("write len");
            stream.write_all(&bytes).await.expect("write body");
        }
        stream.flush().await.ok();
        for i in 0..1000u64 {
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.expect("read len");
            let resp_len = u32::from_le_bytes(len_buf) as usize;
            let mut body = vec![0u8; resp_len];
            stream.read_exact(&mut body).await.expect("read body");
            let resp: IpcResponse = postcard::from_bytes(&body).expect("deserialise");
            assert_eq!(resp, IpcResponse::Pong { nonce: i });
        }
        drop(stream);
        server.abort();
    }

    #[test]
    fn default_socket_path_uses_xdg() {
        let saved = std::env::var("XDG_RUNTIME_DIR").ok();
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        let path = default_socket_path();
        std::env::remove_var("XDG_RUNTIME_DIR");
        if let Some(s) = saved {
            std::env::set_var("XDG_RUNTIME_DIR", s);
        }
        assert_eq!(path, dir.path().join("muzon.sock"));
    }

    #[test]
    fn default_socket_path_fallback_to_tmp() {
        let saved = std::env::var("XDG_RUNTIME_DIR").ok();
        std::env::remove_var("XDG_RUNTIME_DIR");
        let path = default_socket_path();
        if let Some(s) = saved {
            std::env::set_var("XDG_RUNTIME_DIR", s);
        }
        assert!(path.starts_with("/tmp/"), "fallback must be /tmp/, got {path:?}");
        assert!(
            path.to_string_lossy().contains("muzon-"),
            "fallback must be /tmp/muzon-$UID.sock, got {path:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn socket_perms_0600() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let socket = tmp.path().join("muzon.sock");
        std::fs::write(&socket, b"").expect("create");
        set_socket_perms(&socket).expect("set perms");
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&socket)
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "socket perms must be 0600, got {mode:o}");
    }
}
