use std::collections::HashMap;
use std::error::Error as StdError;
use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use bollard::query_parameters::{ListContainersOptionsBuilder, LogsOptionsBuilder};
use bollard::{Docker, API_DEFAULT_VERSION};
use futures_util::StreamExt;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use thiserror::Error;

use crate::logs::ansi::{LogBatch, LogBatcher};

mod fake;

pub use fake::FakeDockerBackend;

pub type DockerFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawContainer {
    pub id: String,
    pub names: Vec<String>,
    pub image: String,
    pub state: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Type)]
pub struct DockerContainer {
    pub id: String,
    pub names: Vec<String>,
    pub image: String,
    pub state: String,
    pub status: String,
}

impl From<RawContainer> for DockerContainer {
    fn from(container: RawContainer) -> Self {
        Self {
            id: container.id,
            names: container.names,
            image: container.image,
            state: container.state,
            status: container.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DockerStatus {
    Detected,
    #[default]
    NotDetected,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Type)]
pub struct DockerSnapshot {
    pub status: DockerStatus,
    pub containers: Vec<DockerContainer>,
}

#[derive(Debug, Error)]
pub enum DockerProbeError {
    #[error("failed to list Docker containers: {0}")]
    List(String),
}

#[derive(Debug, Error)]
pub enum DockerBackendError {
    #[error("Docker socket not detected")]
    NotDetected,
    #[error("{0}")]
    Request(String),
}

pub trait DockerBackend: Send + Sync {
    fn list_all(&self) -> DockerFuture<'_, Result<Vec<RawContainer>, DockerBackendError>>;
    fn logs_follow<'a>(
        &'a self,
        _container_id: &'a str,
    ) -> DockerFuture<'a, Result<Vec<Vec<u8>>, DockerBackendError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

pub struct DockerProbe<B> {
    backend: B,
}

impl<B: DockerBackend> DockerProbe<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub async fn snapshot(&self) -> Result<DockerSnapshot, DockerProbeError> {
        match self.backend.list_all().await {
            Ok(containers) => Ok(DockerSnapshot {
                status: DockerStatus::Detected,
                containers: containers.into_iter().map(Into::into).collect(),
            }),
            Err(DockerBackendError::NotDetected) => Ok(not_detected_snapshot()),
            Err(DockerBackendError::Request(error)) => Err(DockerProbeError::List(error)),
        }
    }
}

fn not_detected_snapshot() -> DockerSnapshot {
    DockerSnapshot {
        status: DockerStatus::NotDetected,
        containers: Vec::new(),
    }
}

#[derive(Default)]
pub struct SystemDockerBackend {
    socket_path: Option<String>,
    client: Mutex<Option<Docker>>,
    #[cfg(test)]
    connect_attempts: AtomicUsize,
}

impl SystemDockerBackend {
    #[cfg(unix)]
    pub fn with_unix_socket(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: Some(socket_path.into()),
            client: Mutex::new(None),
            #[cfg(test)]
            connect_attempts: AtomicUsize::new(0),
        }
    }

    fn client(&self) -> Result<Docker, DockerBackendError> {
        let mut cached = self
            .client
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(client) = cached.as_ref() {
            return Ok(client.clone());
        }

        #[cfg(test)]
        self.connect_attempts.fetch_add(1, Ordering::Relaxed);

        let client = match self.socket_path.as_deref() {
            #[cfg(unix)]
            Some(path) => {
                Docker::connect_with_unix(path, 120, API_DEFAULT_VERSION).map_err(map_bollard_error)
            }
            _ => Docker::connect_with_local_defaults().map_err(map_bollard_error),
        }?;
        *cached = Some(client.clone());
        Ok(client)
    }

    fn invalidate_client(&self) {
        *self
            .client
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }

    #[cfg(test)]
    fn connect_attempts(&self) -> usize {
        self.connect_attempts.load(Ordering::Relaxed)
    }
}

impl DockerBackend for SystemDockerBackend {
    fn list_all(&self) -> DockerFuture<'_, Result<Vec<RawContainer>, DockerBackendError>> {
        Box::pin(async move {
            let docker = self.client()?;
            let options = ListContainersOptionsBuilder::default().all(true).build();
            let containers = match docker.list_containers(Some(options)).await {
                Ok(containers) => containers,
                Err(error) => {
                    let error = map_bollard_error(error);
                    if matches!(error, DockerBackendError::NotDetected) {
                        self.invalidate_client();
                    }
                    return Err(error);
                }
            };

            Ok(containers
                .into_iter()
                .map(|container| RawContainer {
                    id: container.id.unwrap_or_default(),
                    names: container.names.unwrap_or_default(),
                    image: container.image.unwrap_or_default(),
                    state: container
                        .state
                        .map(|state| state.to_string())
                        .unwrap_or_default(),
                    status: container.status.unwrap_or_default(),
                })
                .collect())
        })
    }

    fn logs_follow<'a>(
        &'a self,
        container_id: &'a str,
    ) -> DockerFuture<'a, Result<Vec<Vec<u8>>, DockerBackendError>> {
        Box::pin(async move {
            let docker = self.client()?;
            let options = LogsOptionsBuilder::default()
                .stdout(true)
                .stderr(true)
                .follow(true)
                .tail("200")
                .build();
            let mut stream = docker.logs(container_id, Some(options));
            let mut frames = Vec::new();
            while let Some(frame) = stream.next().await {
                let frame = frame.map_err(map_bollard_error)?;
                frames.push(log_output_bytes(frame));
            }
            Ok(frames)
        })
    }
}

fn log_output_bytes(output: LogOutput) -> Vec<u8> {
    output.as_ref().to_vec()
}

static DOCKER_LOG_CANCEL: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn docker_log_cancel() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    DOCKER_LOG_CANCEL.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
#[specta::specta]
pub fn subscribe_docker_logs(
    container_id: String,
    channel: Channel<LogBatch>,
) -> Result<(), String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    docker_log_cancel()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(container_id.clone(), cancelled.clone());

    tauri::async_runtime::spawn(async move {
        stream_system_docker_logs(container_id, channel, cancelled).await;
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn unsubscribe_docker_logs(container_id: String) -> Result<(), String> {
    if let Some(cancelled) = docker_log_cancel()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&container_id)
    {
        cancelled.store(true, AtomicOrdering::Relaxed);
    }
    Ok(())
}

/// Remove our own cancel entry once a stream ends naturally (container stopped,
/// socket dropped). Guarded by pointer identity so a fresh re-subscription's
/// entry is never clobbered by an older stream tearing down.
fn drop_cancel_entry(container_id: &str, cancelled: &Arc<AtomicBool>) {
    let mut map = docker_log_cancel().lock().unwrap_or_else(|e| e.into_inner());
    if map
        .get(container_id)
        .is_some_and(|existing| Arc::ptr_eq(existing, cancelled))
    {
        map.remove(container_id);
    }
}

#[cfg(test)]
async fn stream_logs_from_backend(
    backend: &impl DockerBackend,
    container_id: &str,
    channel: Channel<LogBatch>,
    cancelled: Arc<AtomicBool>,
) {
    let mut batcher = LogBatcher::new(200, 128 * 1024);
    let Ok(frames) = backend.logs_follow(container_id).await else {
        return;
    };

    for frame in frames {
        if cancelled.load(AtomicOrdering::Relaxed) {
            break;
        }
        let batch = batcher.push_bytes(&frame);
        if !batch.lines.is_empty() && channel.send(batch).is_err() {
            break;
        }
    }
}

async fn stream_system_docker_logs(
    container_id: String,
    channel: Channel<LogBatch>,
    cancelled: Arc<AtomicBool>,
) {
    let backend = SystemDockerBackend::default();
    let Ok(docker) = backend.client() else {
        return;
    };
    let options = LogsOptionsBuilder::default()
        .stdout(true)
        .stderr(true)
        .follow(true)
        .tail("200")
        .build();
    let mut stream = docker.logs(&container_id, Some(options));
    let mut batcher = LogBatcher::new(200, 128 * 1024);

    // Race the log stream against a short poll of the cancel flag so an idle
    // container (no new frames) still observes unsubscribe within ~250ms instead
    // of blocking on `stream.next()` until the next frame ever arrives.
    loop {
        tokio::select! {
            maybe_frame = stream.next() => {
                let Some(frame) = maybe_frame else { break };
                if cancelled.load(AtomicOrdering::Relaxed) {
                    break;
                }
                let Ok(frame) = frame.map(log_output_bytes).map_err(map_bollard_error) else {
                    break;
                };
                let batch = batcher.push_bytes(&frame);
                if !batch.lines.is_empty() && channel.send(batch).is_err() {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                if cancelled.load(AtomicOrdering::Relaxed) {
                    break;
                }
            }
        }
    }
    drop_cancel_entry(&container_id, &cancelled);
}

fn map_bollard_error(error: BollardError) -> DockerBackendError {
    if daemon_is_unreachable(&error) {
        DockerBackendError::NotDetected
    } else {
        DockerBackendError::Request(error.to_string())
    }
}

fn daemon_is_unreachable(error: &BollardError) -> bool {
    match error {
        BollardError::SocketNotFoundError(_) | BollardError::RequestTimeoutError => true,
        BollardError::IOError { err } => unreachable_io_kind(err.kind()),
        BollardError::HyperLegacyError { err } if err.is_connect() => true,
        _ => error_chain_has_unreachable_io(error),
    }
}

fn error_chain_has_unreachable_io(error: &(dyn StdError + 'static)) -> bool {
    let mut source = error.source();
    while let Some(current) = source {
        if let Some(io_error) = current.downcast_ref::<std::io::Error>() {
            if unreachable_io_kind(io_error.kind()) {
                return true;
            }
        }
        source = current.source();
    }
    false
}

fn unreachable_io_kind(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::BrokenPipe
            | ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::ipc::{Channel, InvokeResponseBody};

    fn container(id: &str, state: &str) -> RawContainer {
        RawContainer {
            id: id.into(),
            names: vec![format!("/{id}")],
            image: "example:latest".into(),
            state: state.into(),
            status: state.into(),
        }
    }

    #[tokio::test]
    async fn snapshot_includes_running_and_stopped_containers() {
        let backend = FakeDockerBackend::new(vec![Ok(vec![
            container("running", "running"),
            container("stopped", "exited"),
        ])]);
        let probe = DockerProbe::new(backend);

        let snapshot = probe.snapshot().await.unwrap();

        assert_eq!(snapshot.status, DockerStatus::Detected);
        assert_eq!(snapshot.containers.len(), 2);
        assert_eq!(snapshot.containers[0].state, "running");
        assert_eq!(snapshot.containers[1].state, "exited");
    }

    #[tokio::test]
    async fn docker_log_frames_are_sanitized_into_batches() {
        let backend = FakeDockerBackend::with_logs(vec![Ok(vec![
            b"\x1b[31mred\x1b[0m\n".to_vec(),
            b"<script>alert(1)</script>\n".to_vec(),
        ])]);
        let (tx, rx) = std::sync::mpsc::channel();

        stream_logs_from_backend(
            &backend,
            "container-1",
            Channel::new(move |body| {
                tx.send(body).unwrap();
                Ok(())
            }),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await;

        let mut html = String::new();
        for _ in 0..2 {
            let payload = rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
            let InvokeResponseBody::Json(json) = payload else {
                panic!("expected JSON log batch");
            };
            let batch: crate::logs::ansi::LogBatch = serde_json::from_str(&json).unwrap();
            html.push_str(
                &batch
                    .lines
                    .iter()
                    .map(|line| line.html.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        assert!(html.contains("ansi-fg-red"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[tokio::test]
    async fn docker_log_stream_stops_when_cancelled() {
        let backend =
            FakeDockerBackend::with_logs(vec![Ok(vec![b"first\n".to_vec(), b"second\n".to_vec()])]);
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let (tx, rx) = std::sync::mpsc::channel();

        stream_logs_from_backend(
            &backend,
            "container-1",
            Channel::new(move |body| {
                tx.send(body).unwrap();
                Ok(())
            }),
            cancelled,
        )
        .await;

        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn socket_absence_recovers_when_docker_appears() {
        let backend = FakeDockerBackend::new(vec![
            Err(DockerBackendError::NotDetected),
            Ok(vec![container("recovered", "running")]),
        ]);
        let probe = DockerProbe::new(backend);

        let first = probe.snapshot().await.unwrap();
        let second = probe.snapshot().await.unwrap();

        assert_eq!(first.status, DockerStatus::NotDetected);
        assert!(first.containers.is_empty());
        assert_eq!(second.status, DockerStatus::Detected);
        assert_eq!(second.containers[0].id, "recovered");
        assert_eq!(probe.backend.calls(), 2);
    }

    #[tokio::test]
    async fn request_failures_are_reported_without_caching_not_detected() {
        let backend = FakeDockerBackend::new(vec![
            Err(DockerBackendError::Request("daemon unavailable".into())),
            Ok(vec![container("recovered", "running")]),
        ]);
        let probe = DockerProbe::new(backend);

        assert!(matches!(
            probe.snapshot().await,
            Err(DockerProbeError::List(message)) if message == "daemon unavailable"
        ));
        assert_eq!(probe.snapshot().await.unwrap().containers.len(), 1);
        assert_eq!(probe.backend.calls(), 2);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn absent_system_socket_returns_not_detected_without_crashing() {
        let socket = std::env::temp_dir().join(format!(
            "portus-missing-docker-socket-{}",
            std::process::id()
        ));
        let probe = DockerProbe::new(SystemDockerBackend::with_unix_socket(
            socket.to_string_lossy(),
        ));

        let snapshot = probe.snapshot().await.unwrap();

        assert_eq!(snapshot.status, DockerStatus::NotDetected);
        assert!(snapshot.containers.is_empty());
    }

    #[test]
    fn connection_refused_is_treated_as_not_detected() {
        let error = BollardError::IOError {
            err: std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
        };

        assert!(matches!(
            map_bollard_error(error),
            DockerBackendError::NotDetected
        ));
    }

    #[cfg(unix)]
    #[test]
    fn system_backend_reuses_a_successful_client() {
        let socket =
            std::env::temp_dir().join(format!("portus-docker-client-cache-{}", std::process::id()));
        std::fs::File::create(&socket).unwrap();
        let backend = SystemDockerBackend::with_unix_socket(socket.to_string_lossy());

        backend.client().unwrap();
        backend.client().unwrap();

        assert_eq!(backend.connect_attempts(), 1);
        std::fs::remove_file(socket).unwrap();
    }
}
