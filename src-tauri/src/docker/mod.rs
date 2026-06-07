use std::error::Error as StdError;
use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Mutex;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use bollard::errors::Error as BollardError;
use bollard::query_parameters::ListContainersOptionsBuilder;
use bollard::{Docker, API_DEFAULT_VERSION};
use serde::Serialize;
use specta::Type;
use thiserror::Error;

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
