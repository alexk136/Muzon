// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unix-socket IPC client.
//!
//! Used by `muzonctl` and by the Tauri shell when it attaches
//! to a running headless core. The wire contract is the same
//! as the server: length-prefixed `postcard` framing on a
//! Unix-domain socket.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use super::request::{IpcRequest, IpcResponse};
use super::server::default_socket_path;

const DEFAULT_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Errors produced by the IPC client.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("server returned an invalid response: {0}")]
    InvalidResponse(String),

    #[error("connection timed out after {0:?}")]
    Timeout(std::time::Duration),
}

/// Typed IPC client. Not `Clone` because the underlying
/// `UnixStream` is `!Clone`; callers hold a single client per
/// task and create a new one for each new task.
pub struct IpcClient {
    stream: UnixStream,
    path: String,
}

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcClient")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl IpcClient {
    /// Connect to the headless core at `path`.
    pub async fn connect(path: &Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path).await.map_err(|source| ClientError::Io {
            path: path.to_string_lossy().into_owned(),
            source,
        })?;
        Ok(Self {
            stream,
            path: path.to_string_lossy().into_owned(),
        })
    }

    /// Connect to the default socket path.
    pub async fn connect_default() -> Result<Self, ClientError> {
        let path = default_socket_path();
        Self::connect(&path).await
    }

    /// Send `req` and read the response. The round-trip uses
    /// the same length-prefixed `postcard` framing as the
    /// server.
    pub async fn call(&mut self, req: IpcRequest) -> Result<IpcResponse, ClientError> {
        let payload = postcard::to_stdvec(&req)
            .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;
        let len_bytes = (payload.len() as u32).to_le_bytes();

        // Write the request, with a timeout to avoid a stuck
        // socket blocking the caller forever.
        tokio::time::timeout(DEFAULT_IO_TIMEOUT, async {
            self.stream.write_all(&len_bytes).await?;
            self.stream.write_all(&payload).await?;
            self.stream.flush().await?;
            Ok::<(), std::io::Error>(())
        })
        .await
        .map_err(|_| ClientError::Timeout(DEFAULT_IO_TIMEOUT))?
        .map_err(|source| ClientError::Io {
            path: self.path.clone(),
            source,
        })?;

        // Read the response.
        let mut len_buf = [0u8; 4];
        tokio::time::timeout(DEFAULT_IO_TIMEOUT, self.stream.read_exact(&mut len_buf))
            .await
            .map_err(|_| ClientError::Timeout(DEFAULT_IO_TIMEOUT))?
            .map_err(|source| ClientError::Io {
                path: self.path.clone(),
                source,
            })?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; resp_len];
        tokio::time::timeout(DEFAULT_IO_TIMEOUT, self.stream.read_exact(&mut body))
            .await
            .map_err(|_| ClientError::Timeout(DEFAULT_IO_TIMEOUT))?
            .map_err(|source| ClientError::Io {
                path: self.path.clone(),
                source,
            })?;
        let resp: IpcResponse = postcard::from_bytes(&body)
            .map_err(|e| ClientError::InvalidResponse(e.to_string()))?;
        Ok(resp)
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
            panic!("unexpected: {req:?}")
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn client_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let socket = tmp.path().join("muzon.sock");
        let path = socket.clone();
        let server = tokio::spawn(async move {
            crate::server::serve(&path, echo_handler).await.expect("serve");
        });
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut client = IpcClient::connect(&socket).await.expect("connect");
        let resp = client.call(IpcRequest::Ping { nonce: 7 }).await.expect("call");
        assert_eq!(resp, IpcResponse::Pong { nonce: 7 });

        drop(client);
        server.abort();
    }

    #[tokio::test]
    async fn connect_to_missing_socket_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let socket = tmp.path().join("does_not_exist.sock");
        let result = IpcClient::connect(&socket).await;
        assert!(result.is_err(), "expected connect to fail, got {result:?}");
    }
}
